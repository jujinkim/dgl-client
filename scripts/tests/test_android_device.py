import importlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import android_device as device

ROOT = Path(__file__).resolve().parents[2]


class AndroidDeviceTest(unittest.TestCase):
    def test_adb_base_uses_sdk_platform_tools_and_serial(self):
        self.assertEqual(device.adb_base("emulator-5554", sdk="/sdk"), [str(Path("/sdk/platform-tools/adb")), "-s", "emulator-5554"])
        with patch.dict(os.environ, {"ANDROID_HOME": "/home-sdk"}):
            self.assertEqual(device.adb_base("emulator-5580")[0], str(Path("/home-sdk/platform-tools/adb")))

    def test_restore_setting_deletes_unset_values(self):
        self.assertEqual(device.restore_setting("secure", "show_ime_with_hard_keyboard", "null"),
                         ["shell", "settings", "delete", "secure", "show_ime_with_hard_keyboard"])
        self.assertEqual(device.restore_setting("system", "font_scale", "1.15"),
                         ["shell", "settings", "put", "system", "font_scale", "1.15"])
        self.assertEqual(device.restore_setting("global", "window_animation_scale", "0"),
                         ["shell", "settings", "put", "global", "window_animation_scale", "0"])

    def test_ssh_fixture_reads_files_and_keeps_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            files = {}
            for name in ("KEY", "RSA", "ENCRYPTED"):
                path = Path(temporary) / name
                path.write_text(name.lower() + "-é\n", encoding="utf-8")
                files[f"DGL_TEST_{name}_FILE"] = str(path)
            env = dict(files, DGL_TEST_PORT="2222", DGL_TEST_USER="tester", DGL_TEST_FINGERPRINT="SHA256:x", DGL_TEST_PASSPHRASE="secret")
            self.assertEqual(device.ssh_fixture(env), {"port": 2222, "user": "tester", "fingerprint": "SHA256:x", "key": "key-é\n",
                                                       "rsa": "rsa-é\n", "encrypted": "encrypted-é\n", "passphrase": "secret"})

    def test_fixture_commands_stay_inside_app_cache(self):
        self.assertEqual(device.WRITE_FIXTURE, ["shell", "run-as", "kim.jujin.dglc", "sh", "-c", '"mkdir -p cache && cat > cache/integration.json"'])
        self.assertEqual(device.REMOVE_FIXTURE, ["shell", "run-as", "kim.jujin.dglc", "rm", "-f", "cache/integration.json"])
        self.assertEqual(device.RUNNER, "kim.jujin.dglc.test/androidx.test.runner.AndroidJUnitRunner")
        self.assertEqual(device.APKS[0], "androidApp/build/outputs/apk/debug/androidApp-debug.apk")

    def test_pull_cache_png_requires_success_and_png_header(self):
        cases = [(0, b"\x89PNG\r\n", b"\x89PNG\r\n"), (0, b"cat: no such file", None), (1, b"\x89PNG", None)]
        for returncode, stdout, expected in cases:
            with self.subTest(returncode=returncode, stdout=stdout), \
                 patch.object(device.subprocess, "run", return_value=subprocess.CompletedProcess([], returncode, stdout, b"")) as run:
                self.assertEqual(device.pull_cache_png(["adb", "-s", "e"], "ui-failure", timeout=10), expected)
                run.assert_called_once_with(["adb", "-s", "e", "exec-out", "run-as", "kim.jujin.dglc", "cat", "cache/ui-failure.png"],
                                            capture_output=True, timeout=10)


class WorkspaceScriptTest(unittest.TestCase):
    def test_settings_and_display_are_restored_after_failure(self):
        calls = []
        replies = {("shell", "getprop", "ro.build.version.sdk"): "36", ("shell", "wm", "size"): "Physical size: 1080x2400",
                   ("shell", "wm", "density"): "Physical density: 420\nOverride density: 300",
                   ("shell", "settings", "get", "system", "font_scale"): "1.15"}

        def fake(command, **kwargs):
            args = tuple(command[3:])
            calls.append(args)
            if args[:3] == ("shell", "am", "instrument"):
                raise subprocess.CalledProcessError(1, command)
            return subprocess.CompletedProcess(command, 0, replies.get(args, "null").encode(), b"")

        with patch.dict(os.environ, {"ANDROID_HOME": "/sdk", "ANDROID_SERIAL": "emulator-5580"}):
            sys.modules.pop("workspace_android", None)
            workspace = importlib.import_module("workspace_android")
        with tempfile.TemporaryDirectory() as temporary, patch.object(workspace, "OUT", Path(temporary)), \
             patch.object(workspace.subprocess, "run", side_effect=fake), patch.object(sys, "argv", ["workspace_android.py"]):
            with self.assertRaises(subprocess.CalledProcessError):
                workspace.main()
        self.assertIn(("install", "-r", str(ROOT / device.APKS[0])), calls)
        self.assertIn(("install", "-r", str(ROOT / device.APKS[1])), calls)
        restored = calls[calls.index(("shell", "am", "force-stop", "kim.jujin.dglc")):]
        self.assertEqual(restored[1:], [
            ("shell", "wm", "size", "reset"), ("shell", "wm", "density", "300"),
            ("shell", "settings", "delete", "secure", "show_ime_with_hard_keyboard"),
            ("shell", "settings", "put", "system", "font_scale", "1.15"),
            ("shell", "settings", "delete", "global", "window_animation_scale"),
            ("shell", "settings", "delete", "global", "transition_animation_scale")])


if __name__ == "__main__":
    unittest.main()
