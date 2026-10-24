import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_rust_windows as windows


def pe_fixture(machine=0x8664):
    data = bytearray(128)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 64)
    data[64:68] = b"PE\0\0"
    struct.pack_into("<H", data, 68, machine)
    struct.pack_into("<H", data, 88, 0x20b)
    return data


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
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "app.exe"
            binary.write_bytes(pe_fixture())
            output = root / "output"
            output.mkdir()
            (output / "old-secret.txt").write_text("must not be packaged")

            def notices(package, destination, target=None):
                self.assertEqual(target, windows.TARGET)
                destination.mkdir()
                (destination / "DEPENDENCIES.json").write_text("[]\n")

            with patch.object(windows, "collect", side_effect=notices):
                archive = windows.package(binary, output, "0.1.0")
            with zipfile.ZipFile(archive) as zipped:
                prefix = windows.NAME + "/"
                self.assertTrue(all(n.startswith(prefix) for n in zipped.namelist()))
                self.assertFalse(any("old-secret" in n for n in zipped.namelist()))
                self.assertEqual(zipped.read(prefix + "dgl-desktop.exe"), binary.read_bytes())
                manifest = json.loads(zipped.read(prefix + "build.json"))
                self.assertEqual(manifest["signing"], "unsigned")
                self.assertEqual(manifest["runtimeValidation"], "not-performed")
                self.assertEqual(manifest["binarySha256"], hashlib.sha256(binary.read_bytes()).hexdigest())
                self.assertIn(prefix + "third-party-rust/DEPENDENCIES.json", zipped.namelist())
                sums = zipped.read(prefix + "SHA256SUMS").decode().splitlines()
                self.assertEqual(len(sums), len(zipped.namelist()) - 1)
                for line in sums:
                    digest, name = line.split("  ", 1)
                    self.assertEqual(digest, hashlib.sha256(zipped.read(prefix + name)).hexdigest())
            self.assertEqual((output / "SHA256SUMS").read_text().split()[0], hashlib.sha256(archive.read_bytes()).hexdigest())

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


if __name__ == "__main__":
    unittest.main()
