#!/usr/bin/env python3
"""Bounded API36 workspace and compact-input regression checks; restore display settings."""
import argparse
import os
from pathlib import Path
import subprocess

from verify_android_results import expected_for_selection, verify

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts/android-workspace"
SERIAL = os.environ["ANDROID_SERIAL"]
ADB = [str(Path(os.environ["ANDROID_HOME"]) / "platform-tools/adb"), "-s", SERIAL]


def run(*args, timeout=120):
    return subprocess.run(ADB + list(args), check=True, capture_output=True, timeout=timeout).stdout.decode().strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--section", action="append", choices=("workspace", "mobile-input", "compact"))
    args = parser.parse_args()
    assert SERIAL.startswith("emulator-"), "Use a dedicated emulator"
    assert run("shell", "getprop", "ro.build.version.sdk") == "36"
    OUT.mkdir(parents=True, exist_ok=True)
    run("install", "-r", str(ROOT / "androidApp/build/outputs/apk/debug/androidApp-debug.apk"))
    run("install", "-r", str(ROOT / "androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk"))
    original = {}
    for key in ("size", "density"):
        output = run("shell", "wm", key)
        original[key] = next((line.split(": ", 1)[1] for line in output.splitlines() if line.startswith("Override")), "reset")
    settings = {("secure", "show_ime_with_hard_keyboard"): "1", ("system", "font_scale"): "1.0",
                ("global", "window_animation_scale"): "0", ("global", "transition_animation_scale"): "0"}
    old_settings = {key: run("shell", "settings", "get", *key) for key in settings}
    try:
        for key, value in settings.items():
            run("shell", "settings", "put", *key, value)
        run("shell", "wm", "density", "160")
        run("shell", "wm", "size", "839x1000")
        selections = [("workspace", "kim.jujin.dglc.AndroidWorkspaceTest"),
                      ("mobile-input", "kim.jujin.dglc.AndroidMobileInputTest"),
                      ("compact", "kim.jujin.dglc.AndroidVolumeKeyTest,kim.jujin.dglc.AndroidHierarchyTest")]
        for name, classes in selections:
            if args.section and name not in args.section:
                continue
            if name != "workspace":
                run("shell", "am", "force-stop", "kim.jujin.dglc")
                run("shell", "wm", "size", "320x760" if name == "mobile-input" else "411x914")
            result = run("shell", "am", "instrument", "-w", "-r", "-e", "class", classes,
                         "kim.jujin.dglc.test/androidx.test.runner.AndroidJUnitRunner", timeout=240)
            (OUT / f"{name}.txt").write_text(result + "\n")
            print(result)
            verify(result, expected_for_selection(classes))
    finally:
        run("shell", "am", "force-stop", "kim.jujin.dglc")
        for key, value in original.items():
            run("shell", "wm", key, value)
        for key, value in old_settings.items():
            if value == "null":
                run("shell", "settings", "delete", *key)
            else:
                run("shell", "settings", "put", *key, value)


if __name__ == "__main__":
    main()
