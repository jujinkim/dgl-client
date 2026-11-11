import importlib.util
from pathlib import Path
import struct
import json
import hashlib
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import package_rust_linux as package

class LinuxPackageTests(unittest.TestCase):
    def test_stage_includes_entry_icon_notices_and_metadata_without_tmux(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / "dgl-desktop"
            header = bytearray(64)
            header[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", header, 18, 62)
            binary.write_bytes(header)
            appdir = root / "DaGaLi.AppDir"
            build = package.stage(binary, appdir, "0.1.2", with_libraries=False)
            self.assertEqual(build["arch"], package.TARGET)
            self.assertEqual(build["runtimeValidation"], "not-performed")
            self.assertTrue((appdir / "AppRun").stat().st_mode & 0o111)
            self.assertTrue((appdir / ".DirIcon").is_file())
            self.assertTrue((appdir / "usr/share/doc/DaGaLi/NOTICE").is_file())
            self.assertTrue((appdir / "usr/share/doc/DaGaLi/AppImage-runtime-LICENSE").is_file())
            self.assertTrue((appdir / "usr/share/applications/kim.jujin.dglc.desktop").is_file())
            self.assertFalse((appdir / "usr/bin/tmux").exists())
            binary.write_bytes(b"MZ" + bytes(62))
            with self.assertRaises(ValueError):
                package.verify_executable(binary)

    def test_corrupt_cached_tools_are_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as temp:
            cache = Path(temp)
            digest = package.TOOLS["appimagetool"][1]
            (cache / f"appimagetool-{digest[:16]}").write_bytes(b"corrupt")
            with patch.object(package.urllib.request, "urlopen", side_effect=AssertionError("no network")):
                with self.assertRaises(ValueError):
                    package.tool("appimagetool", cache)

    def test_system_driver_and_glibc_libraries_are_not_bundled(self):
        for name in ["libc.so.6", "libm.so.6", "ld-linux-x86-64.so.2", "libGLX.so.0", "libvulkan.so.1", "libstdc++.so.6", "libgcc_s.so.1"]:
            self.assertIsNotNone(package.SYSTEM.match(name))
        for name in ["libfontconfig.so.1", "libxcb.so.1", "libfreetype.so.6", "libcrypto.so.3"]:
            self.assertIsNone(package.SYSTEM.match(name))

    def test_linux_version_verifier_rejects_changed_package_bytes(self):
        import verify_app_versions as versions
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.2"\n')
            output = root / "artifacts/rust-candidate/linux"
            output.mkdir(parents=True)
            metadata = output / "build.json"
            metadata.write_text(json.dumps({"version": "0.1.2", "arch": package.TARGET}))
            image = output / "DaGaLi-0.1.2-x86_64.AppImage"
            image.write_bytes(b"synthetic package checksum fixture")
            (output / "SHA256SUMS").write_text("".join(
                f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in [image, metadata]))
            with patch.object(versions, "ROOT", root), patch.object(sys, "argv", ["verify_app_versions.py", "--linux"]), patch.object(versions.subprocess, "check_output", return_value=b'{"packages":[{"name":"dgl-desktop","version":"0.1.2"}]}'):
                versions.main()
                image.write_bytes(b"changed package")
                with self.assertRaisesRegex(AssertionError, "Linux package checksum"):
                    versions.main()
