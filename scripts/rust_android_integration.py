#!/usr/bin/env python3
"""API36 candidate acceptance; invoked inside scripts/integration.py's isolated SSH fixture."""
import json
import os
from pathlib import Path
import subprocess
from android_device import APKS, REMOVE_FIXTURE, RUNNER, WRITE_FIXTURE, adb_base, pull_cache_png, restore_setting, ssh_fixture
from verify_android_results import expected_for_selection, verify

ROOT = Path(__file__).resolve().parents[1]
base = adb_base(os.environ.get("ANDROID_SERIAL", "emulator-5554"))

def run(args, **kw):
    return subprocess.run(base + args, check=True, capture_output=True, timeout=kw.pop("timeout", 120), **kw)

def main():
    assert run(["shell", "getprop", "ro.build.version.sdk"]).stdout.strip() == b"36"
    port = os.environ["DGL_TEST_PORT"]
    out = ROOT / "artifacts/rust-candidate/android"
    out.mkdir(parents=True, exist_ok=True)
    for apk in APKS:
        run(["install", "-r", apk])
    run(["reverse", f"tcp:{port}", f"tcp:{port}"])
    old = run(["shell", "settings", "get", "secure", "show_ime_with_hard_keyboard"]).stdout.decode().strip()
    old_font = run(["shell", "settings", "get", "system", "font_scale"]).stdout.decode().strip()
    fixture = ssh_fixture()
    try:
        for name in ["rust-light", "rust-dark", "rust-editor", "rust-tree", "resize-cache", "ui-failure", "volume-ui-failure"]:
            (out / f"{name}.png").unlink(missing_ok=True)
            run(["shell", "run-as", "kim.jujin.dglc", "rm", "-f", f"cache/{name}.png"])
        run(["shell", "settings", "put", "system", "font_scale", "1.3"])
        run(["shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", "1"])
        run(WRITE_FIXTURE, input=json.dumps(fixture).encode())
        classes = os.environ.get("DGL_ANDROID_TEST_CLASSES", "kim.jujin.dglc.AndroidPlatformTest,kim.jujin.dglc.AndroidAppFlowTest,kim.jujin.dglc.AndroidVolumeKeyTest,kim.jujin.dglc.AndroidHierarchyTest,kim.jujin.dglc.AndroidMobileInputTest")
        expected = expected_for_selection(classes)
        result = run(["shell", "am", "instrument", "-w", "-r", "-e", "dgl.fixture", "true", "-e", "class", classes, RUNNER], timeout=240)
        output = result.stdout.decode()
        (out / "api36-instrumentation.txt").write_text(output)
        print(output)
        for name in ["rust-light", "rust-dark", "rust-editor", "rust-tree", "resize-cache", "ui-failure", "volume-ui-failure"]:
            image = pull_cache_png(base, name)
            if image is not None:
                (out / f"{name}.png").write_bytes(image)
        verify(output, expected)
    finally:
        run(REMOVE_FIXTURE)
        run(["reverse", "--remove", f"tcp:{port}"])
        run(restore_setting("secure", "show_ime_with_hard_keyboard", old))
        run(restore_setting("system", "font_scale", old_font))
        run(["shell", "am", "force-stop", "kim.jujin.dglc"])

if __name__ == "__main__":
    main()
