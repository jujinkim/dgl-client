#!/usr/bin/env python3
"""Check displayed/generated versions against Cargo's workspace version and built packages."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def workspace_version(root=ROOT):
    section = (root / "Cargo.toml").read_text().split("[workspace.package]", 1)[1].split("\n[", 1)[0]
    return re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$', section, re.M)[1]


def android_version_code(root=ROOT):
    return re.search(r'\bversionCode\s*=\s*(\d+)', (root / "androidApp/build.gradle.kts").read_text())[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--android", action="store_true", help="Check Kotlin and the Debug APK (default when no platform is selected)")
    parser.add_argument("--mac", action="store_true", help="Check the packaged macOS app")
    parser.add_argument("--windows", action="store_true", help="Check the native Windows package metadata")
    args = parser.parse_args()
    version = workspace_version()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    for package in metadata["packages"]:
        assert package["version"] == version, package["name"]
    checked = ["Cargo"]
    if args.android or not (args.mac or args.windows):
        kotlin = (ROOT / "app/build/generated/appVersion/kim/jujin/dglc/AppVersion.kt").read_text()
        assert f'const val APP_VERSION = "{version}"' in kotlin, "Kotlin display version"
        sdk = Path(os.environ.get("ANDROID_HOME", str(Path.home() / "Library/Android/sdk")))
        apk = ROOT / "androidApp/build/outputs/apk/debug/androidApp-debug.apk"
        badging = subprocess.check_output([str(sdk / "build-tools/36.0.0/aapt"), "dump", "badging", str(apk)], text=True)
        assert re.search(r"versionCode='([^']+)' versionName='([^']+)'", badging).groups() == (android_version_code(), version), "APK version"
        checked.append("Kotlin/Android")
    if args.mac:
        output = ROOT / "artifacts/rust-candidate/mac"
        info = plistlib.loads((output / "DaGaLi.app/Contents/Info.plist").read_bytes())
        assert info["CFBundleShortVersionString"] == info["CFBundleVersion"] == version, "macOS bundle version"
        assert json.loads((output / "build.json").read_text())["version"] == version, "macOS build metadata"
        checked.append("macOS")
    if args.windows:
        output = ROOT / "artifacts/rust-candidate/windows"
        assert json.loads((output / "build.json").read_text())["version"] == version, "Windows build metadata"
        with zipfile.ZipFile(output / "dgl-rust-windows-x64.zip") as archive:
            assert json.loads(archive.read("dgl-rust-windows-x64/build.json"))["version"] == version, "Windows ZIP metadata"
        checked.append("Windows")
    print(f"App/package versions match Cargo.toml: {version} ({', '.join(checked)})")


if __name__ == "__main__":
    main()
