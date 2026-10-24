#!/usr/bin/env python3
"""API36 candidate acceptance; invoked inside scripts/integration.py's isolated SSH fixture."""
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
adb = str(Path(os.environ["ANDROID_HOME"]) / "platform-tools/adb")
base = [adb, "-s", os.environ.get("ANDROID_SERIAL", "emulator-5554")]

def run(args, **kw):
    return subprocess.run(base + args, check=True, capture_output=True, timeout=kw.pop("timeout", 120), **kw)

def main():
    assert run(["shell", "getprop", "ro.build.version.sdk"]).stdout.strip() == b"36"
    port = os.environ["DGL_TEST_PORT"]
    out = ROOT / "artifacts/rust-candidate/android"
    out.mkdir(parents=True, exist_ok=True)
    run(["install", "-r", "androidApp/build/outputs/apk/debug/androidApp-debug.apk"])
    run(["install", "-r", "androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk"])
    run(["reverse", f"tcp:{port}", f"tcp:{port}"])
    old = run(["shell", "settings", "get", "secure", "show_ime_with_hard_keyboard"]).stdout.decode().strip()
    old_font = run(["shell", "settings", "get", "system", "font_scale"]).stdout.decode().strip()
    fixture = {"port": int(port), "user": os.environ["DGL_TEST_USER"], "fingerprint": os.environ["DGL_TEST_FINGERPRINT"],
               "key": Path(os.environ["DGL_TEST_KEY_FILE"]).read_text(), "rsa": Path(os.environ["DGL_TEST_RSA_FILE"]).read_text(),
               "encrypted": Path(os.environ["DGL_TEST_ENCRYPTED_FILE"]).read_text(), "passphrase": os.environ["DGL_TEST_PASSPHRASE"]}
    try:
        for name in ["rust-light", "rust-dark", "rust-editor", "ui-failure"]:
            (out / f"{name}.png").unlink(missing_ok=True)
            run(["shell", "run-as", "kim.jujin.rovessh", "rm", "-f", f"cache/{name}.png"])
        run(["shell", "settings", "put", "system", "font_scale", "1.3"])
        run(["shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", "1"])
        run(["shell", "run-as", "kim.jujin.rovessh", "sh", "-c", '"mkdir -p cache && cat > cache/integration.json"'], input=json.dumps(fixture).encode())
        classes = "kim.jujin.rovessh.AndroidPlatformTest,kim.jujin.rovessh.AndroidAppFlowTest,kim.jujin.rovessh.AndroidVolumeKeyTest"
        result = run(["shell", "am", "instrument", "-w", "-r", "-e", "dgl.fixture", "true", "-e", "class", classes,
                      "kim.jujin.rovessh.test/androidx.test.runner.AndroidJUnitRunner"], timeout=240)
        output = result.stdout.decode()
        (out / "api36-instrumentation.txt").write_text(output)
        print(output)
        for name in ["rust-light", "rust-dark", "rust-editor", "ui-failure"]:
            image = subprocess.run(base + ["exec-out", "run-as", "kim.jujin.rovessh", "cat", f"cache/{name}.png"], capture_output=True)
            if image.returncode == 0 and image.stdout.startswith(b"\x89PNG"):
                (out / f"{name}.png").write_bytes(image.stdout)
        if "FAILURES" in output or "INSTRUMENTATION_FAILED" in output or "OK (7 tests)" not in output:
            raise SystemExit(1)
    finally:
        run(["shell", "run-as", "kim.jujin.rovessh", "rm", "-f", "cache/integration.json"])
        run(["reverse", "--remove", f"tcp:{port}"])
        run(["shell", "settings", "delete", "secure", "show_ime_with_hard_keyboard"] if old == "null" else ["shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", old])
        run(["shell", "settings", "delete", "system", "font_scale"] if old_font == "null" else ["shell", "settings", "put", "system", "font_scale", old_font])
        run(["shell", "am", "force-stop", "kim.jujin.rovessh"])

if __name__ == "__main__":
    main()
