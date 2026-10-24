#!/usr/bin/env python3
"""Build the Rust Android engine for all four ABIs and generate matching UniFFI bindings."""
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {"arm64-v8a": ("aarch64-linux-android", "aarch64-linux-android"),
           "armeabi-v7a": ("armv7-linux-androideabi", "armv7a-linux-androideabi"),
           "x86": ("i686-linux-android", "i686-linux-android"),
           "x86_64": ("x86_64-linux-android", "x86_64-linux-android")}

def run(args, **kwargs):
    if os.uname().sysname == "Darwin":
        kwargs.setdefault("env", dict(os.environ, DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"))
    subprocess.run(args, cwd=ROOT, check=True, **kwargs)

def main():
    sdk = Path(os.environ.get("ANDROID_HOME", str(Path.home() / "Library/Android/sdk")))
    ndk = Path(os.environ.get("ANDROID_NDK_HOME", str(sdk / "ndk/29.0.14206865")))
    host = "darwin-x86_64" if os.uname().sysname == "Darwin" else "linux-x86_64"
    toolchain = ndk / "toolchains/llvm/prebuilt" / host / "bin"
    if not toolchain.is_dir():
        raise SystemExit(f"NDK toolchain missing: {toolchain}")
    run(["cargo", "build", "--locked", "-p", "dgl-uniffi", "--features", "cli"])
    library = ROOT / "target/debug" / ("libdgl_uniffi.dylib" if host.startswith("darwin") else "libdgl_uniffi.so")
    generated = ROOT / "coreBindings/src/main/kotlin"
    run(["target/debug/uniffi-bindgen", "generate", "--library", str(library), "--language", "kotlin",
         "--out-dir", str(generated), "--no-format"])
    # Bindgen emits a flat file even for a nested Kotlin package.
    destination = generated / "uniffi/dgl_core"
    destination.mkdir(parents=True, exist_ok=True)
    for source in generated.glob("*.kt"):
        shutil.move(source, destination / source.name)
    for abi, (target, clang) in TARGETS.items():
        env = os.environ.copy()
        env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(toolchain / f"{clang}34-clang")
        env[f"CC_{target.replace('-', '_')}"] = str(toolchain / f"{clang}34-clang")
        env[f"AR_{target.replace('-', '_')}"] = str(toolchain / "llvm-ar")
        env["CARGO_TARGET_DIR"] = str(ROOT / "target/android")
        run(["cargo", "build", "--locked", "--release", "-p", "dgl-uniffi", "--target", target], env=env)
        output = ROOT / "coreBindings/src/main/jniLibs" / abi
        output.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "target/android" / target / "release/libdgl_uniffi.so", output)

if __name__ == "__main__":
    main()
