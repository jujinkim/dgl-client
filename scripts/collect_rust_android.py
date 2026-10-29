#!/usr/bin/env python3
"""Verify the already-built Rust Android app without copying its APK."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile
from verify_app_versions import workspace_version, android_version_code

ROOT = Path(__file__).resolve().parents[1]
ABIS = {"arm64-v8a", "armeabi-v7a", "x86", "x86_64"}

def verify_rust_payload(archive):
    """Reject a previous-engine APK even if its package ID and version match."""
    names = archive.namelist()
    native = {name.split("/")[1] for name in names
              if re.fullmatch(r"lib/[^/]+/libdgl_uniffi\.so", name)}
    packaged = {name.split("/")[1] for name in names if re.fullmatch(r"lib/[^/]+/[^/]+\.so", name)}
    if native != ABIS or packaged != ABIS:
        raise ValueError(f"Expected four Rust ABIs; Rust: {sorted(native)}, packaged: {sorted(packaged)}")
    dex = b"".join(archive.read(name) for name in names if re.fullmatch(r"classes\d*\.dex", name))
    # Reject both relocated reference classes and any previous Kotlin namespace.
    for retired in (b"Lkim/jujin/dglc/ClientModel;", b"Lkim/jujin/dglc/ssh/SshjTransport;",
                    b"Lkim/jujin/dglc/core/ControlParser;", b"Lkim/jujin/dglc/core/TmuxWire;",
                    b"Lkim/jujin/rovessh/",
                    b"Lnet/schmizz/sshj/", b"Lorg/bouncycastle/"):
        if retired in dex:
            raise ValueError("Retired engine/dependency in production APK: " + retired.decode())
    for required in (b"Lkim/jujin/dglc/RustClientConnection;", b"Lkim/jujin/dglc/RustKeyTools;",
                     b"Luniffi/dgl_core/NativeConnection;"):
        if required not in dex:
            raise ValueError("Rust implementation missing: " + required.decode())
    archive.getinfo("assets/third-party-rust/DEPENDENCIES.json")
    return {abi: hashlib.sha256(archive.read(f"lib/{abi}/libdgl_uniffi.so")).hexdigest()
            for abi in sorted(ABIS)}

def main():
    sdk = Path(os.environ.get("ANDROID_HOME", str(Path.home() / "Library/Android/sdk")))
    build_tools = sdk / "build-tools/36.0.0"
    source = ROOT / "androidApp/build/outputs/apk/debug/androidApp-debug.apk"
    manifest = subprocess.check_output([str(build_tools / "aapt"), "dump", "badging", str(source)], text=True)
    package = re.search(r"package: name='([^']+)' versionCode='([^']+)' versionName='([^']+)'", manifest)
    if not package or package.groups() != ("kim.jujin.dglc", android_version_code(), workspace_version()):
        raise SystemExit("Unexpected candidate identity/version")
    if "sdkVersion:'34'" not in manifest or "targetSdkVersion:'36'" not in manifest:
        raise SystemExit("Unexpected SDK requirements")
    with zipfile.ZipFile(source) as archive:
        libraries = verify_rust_payload(archive)
        archive.getinfo("assets/terminal/xterm.js")
    subprocess.run([str(build_tools / "apksigner"), "verify", "--verbose", str(source)], check=True)
    output = ROOT / "artifacts/rust-candidate/android"
    output.mkdir(parents=True, exist_ok=True)
    report = {"engine": "rust", "applicationId": package[1], "versionCode": package[2], "versionName": package[3],
              "profile": "debug", "minSdk": 34, "targetSdk": 36, "abis": sorted(ABIS), "librarySha256": libraries,
              "apkPath": source.relative_to(ROOT).as_posix(),
              "apkSha256": hashlib.sha256(source.read_bytes()).hexdigest(),
              "cargoLockSha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()}
    (output / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Validated: " + str(source))

if __name__ == "__main__":
    main()
