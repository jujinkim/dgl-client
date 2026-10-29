#!/usr/bin/env python3
"""Shared adb argument builders for the dedicated-emulator Android instrumentation scripts."""
import os
from pathlib import Path
import subprocess

PACKAGE = "kim.jujin.dglc"
RUNNER = PACKAGE + ".test/androidx.test.runner.AndroidJUnitRunner"
# Repository-relative; the single Debug APK and its instrumentation APK.
APKS = ("androidApp/build/outputs/apk/debug/androidApp-debug.apk",
        "androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk")
# Disposable SSH credentials go over stdin into app-private cache, never as adb arguments.
WRITE_FIXTURE = ["shell", "run-as", PACKAGE, "sh", "-c", '"mkdir -p cache && cat > cache/integration.json"']
REMOVE_FIXTURE = ["shell", "run-as", PACKAGE, "rm", "-f", "cache/integration.json"]


def adb_base(serial, sdk=None):
    return [str(Path(sdk if sdk is not None else os.environ["ANDROID_HOME"]) / "platform-tools/adb"), "-s", serial]


def restore_setting(namespace, key, value):
    """adb args putting back a value read by `settings get`; "null" means it was unset."""
    if value == "null":
        return ["shell", "settings", "delete", namespace, key]
    return ["shell", "settings", "put", namespace, key, value]


def ssh_fixture(environ=None):
    """Fixture JSON fields exported by scripts/integration.py's isolated SSH server."""
    env = os.environ if environ is None else environ
    read = lambda name: Path(env[name]).read_text(encoding="utf-8")
    return {"port": int(env["DGL_TEST_PORT"]), "user": env["DGL_TEST_USER"], "fingerprint": env["DGL_TEST_FINGERPRINT"],
            "key": read("DGL_TEST_KEY_FILE"), "rsa": read("DGL_TEST_RSA_FILE"),
            "encrypted": read("DGL_TEST_ENCRYPTED_FILE"), "passphrase": env["DGL_TEST_PASSPHRASE"]}


def pull_cache_png(base, name, timeout=None):
    """Return cache/<name>.png from the app sandbox, or None when absent or not a PNG."""
    image = subprocess.run(base + ["exec-out", "run-as", PACKAGE, "cat", f"cache/{name}.png"], capture_output=True, timeout=timeout)
    return image.stdout if image.returncode == 0 and image.stdout.startswith(b"\x89PNG") else None
