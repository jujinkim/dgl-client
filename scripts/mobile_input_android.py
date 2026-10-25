#!/usr/bin/env python3
"""Bounded API36 mobile UX checks and screenshots on a dedicated emulator."""
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts/mobile-input"
SERIAL = os.environ.get("ANDROID_SERIAL", "emulator-5554")
ADB = [str(Path(os.environ["ANDROID_HOME"]) / "platform-tools/adb"), "-s", SERIAL]


def run(*args, **kwargs):
    return subprocess.run(ADB + list(args), check=True, capture_output=True, timeout=kwargs.pop("timeout", 120), **kwargs).stdout


def main():
    assert SERIAL.startswith("emulator-"), "Use a dedicated emulator"
    assert run("shell", "getprop", "ro.build.version.sdk").strip() == b"36"
    OUT.mkdir(parents=True, exist_ok=True)
    run("install", "-r", str(ROOT / "androidApp/build/outputs/apk/debug/androidApp-debug.apk"))
    run("install", "-r", str(ROOT / "androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk"))
    old_ime = run("shell", "settings", "get", "secure", "show_ime_with_hard_keyboard").decode().strip()
    animations = {key: run("shell", "settings", "get", "global", key).decode().strip()
                  for key in ("window_animation_scale", "transition_animation_scale")}
    size = run("shell", "wm", "size").decode()
    old_size = next((line.split(": ", 1)[1] for line in size.splitlines() if line.startswith("Override size:")), "reset")
    density = run("shell", "wm", "density").decode().splitlines()[-1].split(": ")[1]
    names = ["mobile-light-portrait", "mobile-dark-landscape", "mobile-composer-portrait", "mobile-composer-landscape", "mobile-failure"]
    try:
        for name in names:
            (OUT / f"{name}.png").unlink(missing_ok=True)
            run("shell", "run-as", "kim.jujin.dglc", "rm", "-f", f"cache/{name}.png")
        run("shell", "wm", "size", f"{round(320 * int(density) / 160)}x{round(760 * int(density) / 160)}")
        run("shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", "1")
        # Capture settled layouts, not the system's rotating screenshot overlay.
        for key in animations:
            run("shell", "settings", "put", "global", key, "0")
        result = run("shell", "am", "instrument", "-w", "-r", "-e", "class",
                     "kim.jujin.rovessh.AndroidMobileInputTest,kim.jujin.rovessh.AndroidPlatformTest",
                     "kim.jujin.dglc.test/androidx.test.runner.AndroidJUnitRunner", timeout=180).decode()
        (OUT / "instrumentation.txt").write_text(result)
        print(result)
        if "OK (3 tests)" not in result or "FAILURES" in result or "INSTRUMENTATION_FAILED" in result:
            raise SystemExit(1)
    finally:
        for name in names:
            image = subprocess.run(ADB + ["exec-out", "run-as", "kim.jujin.dglc", "cat", f"cache/{name}.png"], capture_output=True, timeout=10)
            if image.returncode == 0 and image.stdout.startswith(b"\x89PNG"):
                (OUT / f"{name}.png").write_bytes(image.stdout)
        run("shell", "wm", "size", old_size)
        for key, value in animations.items():
            run("shell", "settings", "delete", "global", key) if value == "null" else run("shell", "settings", "put", "global", key, value)
        if old_ime == "null":
            run("shell", "settings", "delete", "secure", "show_ime_with_hard_keyboard")
        else:
            run("shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", old_ime)
        run("shell", "am", "force-stop", "kim.jujin.dglc")


if __name__ == "__main__":
    main()
