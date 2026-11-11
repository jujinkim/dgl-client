#!/usr/bin/env python3
"""Check displayed/generated versions against Cargo's workspace version and built packages."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
from xml.etree import ElementTree
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def workspace_version(root=ROOT):
    section = (root / "Cargo.toml").read_text().split("[workspace.package]", 1)[1].split("\n[", 1)[0]
    return re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$', section, re.M)[1]


def android_version_code(root=ROOT):
    version = workspace_version(root)
    major, minor, patch = map(int, version.split('.'))
    if not (0 <= major <= 9 and 0 <= minor <= 99 and 0 <= patch <= 99):
        raise ValueError("Android version requires major 0..9, minor 0..99 and patch 0..99")
    code = major * 10000 + minor * 100 + patch
    if code <= 0:
        raise ValueError("Android versionCode must be positive")
    return str(code)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--android", action="store_true", help="Check Kotlin and the Debug APK (default when no platform is selected)")
    parser.add_argument("--mac", action="store_true", help="Check the packaged macOS app")
    parser.add_argument("--windows", action="store_true", help="Check the native Windows package metadata")
    parser.add_argument("--linux", action="store_true", help="Check AppImage filename, metadata and checksum")
    args = parser.parse_args()
    version = workspace_version()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    for package in metadata["packages"]:
        assert package["version"] == version, package["name"]
    checked = ["Cargo"]
    if args.android or not (args.mac or args.windows or args.linux):
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
        msix = output / "dgl-rust-windows-x64.msix"
        if msix.is_file():  # Optional; ZIP-only builds remove any earlier MSIX.
            with zipfile.ZipFile(msix) as package:
                identity = ElementTree.fromstring(package.read("AppxManifest.xml")).find(
                    "{http://schemas.microsoft.com/appx/manifest/foundation/windows10}Identity")
                assert identity.get("Version") == f"{version}.0", "Windows MSIX version"
        checked.append("Windows ZIP/MSIX" if msix.is_file() else "Windows ZIP")
    if args.linux:
        output = ROOT / "artifacts/rust-candidate/linux"
        build = json.loads((output / "build.json").read_text())
        assert build["version"] == version and build["arch"] == "x86_64-unknown-linux-gnu", "Linux build metadata"
        image = output / f"DaGaLi-{version}-x86_64.AppImage"
        checksums = dict(line.split("  ", 1)[::-1] for line in (output / "SHA256SUMS").read_text().splitlines())
        for path in [image, output / "build.json"]:
            assert hashlib.sha256(path.read_bytes()).hexdigest() == checksums[path.name], "Linux package checksum"
        checked.append("Linux AppImage")
    print(f"App/package versions match Cargo.toml: {version} ({', '.join(checked)})")


if __name__ == "__main__":
    main()
