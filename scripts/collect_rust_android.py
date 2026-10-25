#!/usr/bin/env python3
"""Verify and collect the already-built Rust Android app."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import zipfile

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
    for retired in (b"Lkim/jujin/rovessh/ClientModel;", b"Lkim/jujin/rovessh/ssh/SshjTransport;",
                    b"Lnet/schmizz/sshj/", b"Lorg/bouncycastle/"):
        if retired in dex:
            raise ValueError("Retired engine/dependency in production APK: " + retired.decode())
    for required in (b"Lkim/jujin/dgl/RustClientConnection;", b"Lkim/jujin/dgl/RustKeyTools;",
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
    if not package or package.groups() != ("kim.jujin.dglc", "1", "0.1.0"):
        raise SystemExit("Unexpected candidate identity/version")
    if "sdkVersion:'34'" not in manifest or "targetSdkVersion:'36'" not in manifest:
        raise SystemExit("Unexpected SDK requirements")
    with zipfile.ZipFile(source) as archive:
        libraries = verify_rust_payload(archive)
        archive.getinfo("assets/terminal/xterm.js")
    subprocess.run([str(build_tools / "apksigner"), "verify", "--verbose", str(source)], check=True)
    output = ROOT / "artifacts/rust-candidate/android"
    output.mkdir(parents=True, exist_ok=True)
    target = output / "dgl-rust-debug.apk"
    shutil.copy2(source, target)
    report = {"engine": "rust", "applicationId": package[1], "versionCode": package[2], "versionName": package[3],
              "profile": "debug", "minSdk": 34, "targetSdk": 36, "abis": sorted(ABIS), "librarySha256": libraries,
              "apkSha256": hashlib.sha256(target.read_bytes()).hexdigest(),
              "cargoLockSha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()}
    (output / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(target)

if __name__ == "__main__":
    main()
