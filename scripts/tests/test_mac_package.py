import json
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_rust_mac as mac


class MacPackageTest(unittest.TestCase):
    def test_bundle_and_build_metadata_follow_cargo_version_and_target_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ("desktop/resources/dgl.icns", "desktop/NOTICE", "desktop/LICENSE-APACHE-GPUI", "licenses/simple-icons-LICENSE.md",
                         "custom-target/release/dgl-desktop"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(name.encode())
            metadata = {"target_directory": str(root / "custom-target"), "packages": [{"name": "dgl-desktop", "version": "9.8.7"}]}
            with patch.object(mac, "ROOT", root), patch.object(sys, "argv", ["package_rust_mac.py", "--release"]), \
                 patch.object(mac.subprocess, "run"), patch.object(mac.subprocess, "check_output", return_value=json.dumps(metadata)), \
                 patch.object(mac, "host_target", return_value="aarch64-apple-darwin"), patch.object(mac, "collect") as notices:
                mac.main()
            notices.assert_called_once_with("dgl-desktop", root / "artifacts/rust-candidate/mac/DaGaLi.app/Contents/Resources/third-party-rust",
                                            target="aarch64-apple-darwin")
            output = root / "artifacts/rust-candidate/mac"
            info = plistlib.loads((output / "DaGaLi.app/Contents/Info.plist").read_bytes())
            self.assertEqual(info["CFBundleShortVersionString"], "9.8.7")
            self.assertEqual(info["CFBundleVersion"], "9.8.7")
            self.assertEqual(json.loads((output / "build.json").read_text())["version"], "9.8.7")
            self.assertEqual((output / "DaGaLi.app/Contents/MacOS/dgl-desktop").read_bytes(), b"custom-target/release/dgl-desktop")
