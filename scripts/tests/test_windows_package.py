import hashlib
import json
import os
from pathlib import Path
import re
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
from xml.etree import ElementTree
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_rust_windows as windows
from verify_app_versions import workspace_version


def pe_fixture(machine=0x8664):
    data = bytearray(128)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 64)
    data[64:68] = b"PE\0\0"
    struct.pack_into("<H", data, 68, machine)
    struct.pack_into("<H", data, 88, 0x20b)
    return data


def fake_makeappx(content, destination):
    """Stand in for makeappx: MSIX is a ZIP container with the payload at its root."""
    with zipfile.ZipFile(destination, "w") as package:
        for file in sorted(p for p in content.rglob("*") if p.is_file()):
            package.write(file, file.relative_to(content).as_posix())


def png_size(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    return struct.unpack(">II", data[16:24])


class WindowsPackageTest(unittest.TestCase):
    def test_rejects_wrong_architecture_and_invalid_executables(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "app.exe"
            for data in (b"", b"MZ", b"not a Windows binary", pe_fixture(0xaa64), pe_fixture(0x14c)):
                binary.write_bytes(data)
                with self.assertRaises(ValueError):
                    windows.verify_executable(binary)
            binary.write_bytes(pe_fixture())
            windows.verify_executable(binary)

    def test_archive_inventory_and_hashes_match_and_exclude_stale_files(self):
        for msix in (False, True):
            with self.subTest(msix=msix):
                self.check_packages(msix)

    def check_packages(self, msix):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "app.exe"
            binary.write_bytes(pe_fixture())
            output = root / "output"
            output.mkdir()
            (output / "old-secret.txt").write_text("must not be packaged")
            (output / f"{windows.NAME}.msix").write_bytes(b"stale MSIX from an earlier build")

            def notices(package, destination, target=None):
                self.assertEqual(target, windows.TARGET)
                destination.mkdir()
                (destination / "DEPENDENCIES.json").write_text("[]\n")
                # Cargo registry files commonly have an epoch timestamp.
                license_file = destination / "LICENSE"
                license_file.write_bytes(b"dependency license\n")
                os.utime(license_file, (315532800 - 86400, 315532800 - 86400))

            makeappx = fake_makeappx if msix else AssertionError("ZIP-only builds must not run makeappx")
            with patch.object(windows, "collect", side_effect=notices), \
                    patch.object(windows, "pack_msix", side_effect=makeappx):
                packages = windows.package(binary, output, workspace_version(), msix=msix)
            archive = output / f"{windows.NAME}.zip"
            self.assertEqual(packages, [archive, output / f"{windows.NAME}.msix"] if msix else [archive])
            with zipfile.ZipFile(archive) as zipped:
                prefix = windows.NAME + "/"
                self.assertTrue(all(n.startswith(prefix) for n in zipped.namelist()))
                self.assertFalse(any("old-secret" in n for n in zipped.namelist()))
                self.assertEqual(zipped.read(prefix + "dgl-desktop.exe"), binary.read_bytes())
                manifest = json.loads(zipped.read(prefix + "build.json"))
                self.assertEqual(manifest["version"], workspace_version())
                self.assertEqual(manifest["signing"], "unsigned")
                self.assertEqual(manifest["runtimeValidation"], "not-performed")
                self.assertEqual(manifest["binarySha256"], hashlib.sha256(binary.read_bytes()).hexdigest())
                self.assertEqual(manifest.get("msixVersion"), workspace_version() + ".0" if msix else None)
                self.assertIn(prefix + "third-party-rust/DEPENDENCIES.json", zipped.namelist())
                self.assertEqual(zipped.read(prefix + "third-party-rust/LICENSE"), b"dependency license\n")
                self.assertEqual(zipped.getinfo(prefix + "third-party-rust/LICENSE").date_time[:3], (1980, 1, 1))
                sums = zipped.read(prefix + "SHA256SUMS").decode().splitlines()
                self.assertEqual(len(sums), len(zipped.namelist()) - 1)
                for line in sums:
                    digest, name = line.split("  ", 1)
                    self.assertEqual(digest, hashlib.sha256(zipped.read(prefix + name)).hexdigest())
            if msix:
                with zipfile.ZipFile(packages[1]) as package:
                    names = set(package.namelist())
                    self.assertEqual(package.read("dgl-desktop.exe"), binary.read_bytes())
                    self.assertIn("third-party-rust/LICENSE", names)
                    self.assertTrue({"Assets/Square44x44Logo.png", "Assets/Square150x150Logo.png",
                                     "Assets/StoreLogo.png"} <= names)
                    self.assertFalse(names & windows.ZIP_ONLY)
                    self.assertFalse(any("old-secret" in n for n in names))
                    identity = ElementTree.fromstring(package.read("AppxManifest.xml")).find(windows.APPX + "Identity")
                    self.assertEqual(identity.get("Version"), workspace_version() + ".0")
            else:
                self.assertFalse((output / f"{windows.NAME}.msix").exists(), "stale MSIX must not outlive a ZIP-only build")
            sums = dict(reversed(line.split("  ", 1)) for line in (output / "SHA256SUMS").read_text().splitlines())
            self.assertEqual(sums, {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in packages})

    def test_failed_notice_collection_preserves_previous_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "app.exe"
            binary.write_bytes(pe_fixture())
            archive = root / f"{windows.NAME}.zip"
            archive.write_bytes(b"previous completed artifact")
            with patch.object(windows, "collect", side_effect=RuntimeError("collection failed")):
                with self.assertRaises(RuntimeError):
                    windows.package(binary, root, "0.1.0")
            self.assertEqual(archive.read_bytes(), b"previous completed artifact")

    def test_failed_msix_pack_preserves_previous_packages(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "app.exe"
            binary.write_bytes(pe_fixture())
            previous = {name: f"previous {name}".encode() for name in
                        (f"{windows.NAME}.zip", f"{windows.NAME}.msix", "SHA256SUMS")}
            for name, data in previous.items():
                (root / name).write_bytes(data)

            def notices(package, destination, target=None):
                destination.mkdir()

            with patch.object(windows, "collect", side_effect=notices), \
                    patch.object(windows, "pack_msix", side_effect=OSError("makeappx failed")):
                with self.assertRaises(OSError):
                    windows.package(binary, root, "0.1.0", msix=True)
            for name, data in previous.items():
                self.assertEqual((root / name).read_bytes(), data)

    def test_msix_version_reserves_store_revision(self):
        self.assertEqual(windows.msix_version("0.1.0"), "0.1.0.0")
        self.assertEqual(windows.msix_version(workspace_version()), workspace_version() + ".0")
        for version in ("0.1", "0.1.0.0", "0.1.0-beta", "65536.0.0", ""):
            with self.assertRaises(ValueError):
                windows.msix_version(version)

    def test_msix_manifest_matches_payload_and_toast_activator(self):
        text, identity = windows.msix_manifest("1.2.3")
        self.assertNotIn("$", text)
        self.assertEqual(identity.get("Version"), "1.2.3.0")
        self.assertEqual(identity.get("ProcessorArchitecture"), "x64")
        root = ElementTree.fromstring(text.encode())
        application = root.find(f"{windows.APPX}Applications/{windows.APPX}Application")
        self.assertEqual(application.get("Executable"), "dgl-desktop.exe")
        logos = {element.text for element in root.iter(windows.APPX + "Logo")}
        logos |= {value for element in root.iter() for key, value in element.attrib.items() if key.endswith("Logo")}
        # Scale-100 sizes, as generated by scripts/generate_app_icons.py (MSIX_LOGOS).
        expected = {"Assets\\Square44x44Logo.png": (44, 44), "Assets\\Square150x150Logo.png": (150, 150),
                    "Assets\\StoreLogo.png": (50, 50)}
        self.assertEqual(logos, set(expected))
        for logo, size in expected.items():
            self.assertEqual(png_size(windows.MSIX / logo.replace("\\", "/")), size)
        com = "{http://schemas.microsoft.com/appx/manifest/com/windows10}"
        desktop = "{http://schemas.microsoft.com/appx/manifest/desktop/windows10}"
        server = next(root.iter(com + "ExeServer"))
        self.assertEqual((server.get("Executable"), server.get("Arguments")), ("dgl-desktop.exe", "--agent-toast"))
        clsids = {next(root.iter(com + "Class")).get("Id"),
                  next(root.iter(desktop + "ToastNotificationActivation")).get("ToastActivatorCLSID")}
        self.assertEqual(len(clsids), 1)
        rust = (windows.ROOT / "desktop/src/notifications/windows.rs").read_text()
        packaged = re.search(r"PACKAGE_ACTIVATOR: GUID = GUID::from_u128\(0x([0-9a-f_]+)\)", rust)[1]
        unpackaged = re.search(r'ACTIVATOR_TEXT: &str = "\{([0-9A-F-]+)\}"', rust)[1]
        clsid = clsids.pop()
        self.assertEqual(clsid.replace("-", "").lower(), packaged.replace("_", ""))
        self.assertNotEqual(clsid, unpackaged)
        self.assertIn('"\\"{}\\" --agent-toast"', rust)


if __name__ == "__main__":
    unittest.main()
