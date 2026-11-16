from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_app_versions import android_version_code


class AndroidVersionCodeTest(unittest.TestCase):
    def code(self, version):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')
            return int(android_version_code(root))

    def test_fixed_width_components(self):
        for version, expected in (("0.1.1", 101), ("0.10.0", 1000), ("1.2.3", 10203), ("9.99.99", 99999)):
            with self.subTest(version=version):
                self.assertEqual(self.code(version), expected)

    def test_overflow_and_zero_are_rejected(self):
        for version in ("10.0.0", "0.100.0", "0.0.100", "0.0.0"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.code(version)

if __name__ == "__main__":
    unittest.main()
