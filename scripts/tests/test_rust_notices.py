import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_rust_notices as notices


def metadata(root):
    crate = root / "registry/shared-1.0.0"
    crate.mkdir(parents=True)
    (crate / "Cargo.toml").write_text("[package]\nname = \"shared\"\n")
    (crate / "LICENSE-MIT").write_text("MIT text\n")
    return {"packages": [
        {"id": "app", "name": "dgl-uniffi", "version": "0.1.0", "source": None, "manifest_path": str(root / "app/Cargo.toml")},
        {"id": "shared", "name": "shared", "version": "1.0.0", "source": "registry+https://github.com/rust-lang/crates.io-index",
         "manifest_path": str(crate / "Cargo.toml"), "license": "MIT", "repository": None, "license_file": None}],
        "resolve": {"nodes": [{"id": "app", "dependencies": ["shared"]}, {"id": "shared", "dependencies": []}]}}


class RustNoticeTest(unittest.TestCase):
    def collect(self, package, target=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "out/third-party-rust"
            stale = output / "removed-crate-0.1.0"
            stale.mkdir(parents=True)
            (output / "DEPENDENCIES.json").write_text("[]\n")
            with patch.object(notices.subprocess, "check_output", return_value=json.dumps(metadata(root))) as cargo:
                notices.collect(package, output, target)
            inventory = json.loads((output / "DEPENDENCIES.json").read_text())
            return cargo.call_args.args[0], inventory, stale.exists(), (output / "shared-1.0.0/LICENSE-MIT").read_text()

    def test_android_bindings_default_to_the_four_packaged_abis(self):
        command, inventory, stale, license_text = self.collect("dgl-uniffi")
        filtered = [command[i + 1] for i, arg in enumerate(command) if arg == "--filter-platform"]
        self.assertEqual(filtered, ["aarch64-linux-android", "armv7-linux-androideabi", "i686-linux-android", "x86_64-linux-android"])
        self.assertEqual([item["name"] for item in inventory], ["shared"])
        self.assertEqual(license_text, "MIT text\n")
        self.assertFalse(stale, "a previous collection must not leave crates outside the filtered graph")

    def test_explicit_target_overrides_default(self):
        command, *_ = self.collect("dgl-uniffi", "x86_64-pc-windows-msvc")
        self.assertEqual(command.count("--filter-platform"), 1)
        self.assertIn("x86_64-pc-windows-msvc", command)

    def test_unfiltered_collection_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(notices.subprocess, "check_output") as cargo:
            with self.assertRaises(ValueError):
                notices.collect("dgl-desktop", Path(temporary) / "out")
            cargo.assert_not_called()

    def test_host_target_reads_rustc_host_line(self):
        with patch.object(notices.subprocess, "check_output", return_value="rustc 1.90.0\nhost: aarch64-apple-darwin\nrelease: 1.90.0\n"):
            self.assertEqual(notices.host_target(), "aarch64-apple-darwin")


if __name__ == "__main__":
    unittest.main()
