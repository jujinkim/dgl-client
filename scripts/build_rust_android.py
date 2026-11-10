#!/usr/bin/env python3
"""Build the Rust Android engine for all four ABIs and generate matching UniFFI bindings."""
import os
import platform
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {"arm64-v8a": ("aarch64-linux-android", "aarch64-linux-android"),
           "armeabi-v7a": ("armv7-linux-androideabi", "armv7a-linux-androideabi"),
           "x86": ("i686-linux-android", "i686-linux-android"),
           "x86_64": ("x86_64-linux-android", "x86_64-linux-android")}
NDK_VERSION = "29.0.14206865"


def host_tools():
    system = platform.system()
    if system == "Windows":
        return "windows-x86_64", "dgl_uniffi.dll", "uniffi-bindgen.exe", ".cmd", ".exe"
    if system == "Darwin":
        return "darwin-x86_64", "libdgl_uniffi.dylib", "uniffi-bindgen", "", ""
    if system == "Linux":
        return "linux-x86_64", "libdgl_uniffi.so", "uniffi-bindgen", "", ""
    raise SystemExit(f"Unsupported Android build host: {system}")


def default_sdk():
    system = platform.system()
    if system == "Windows":
        return Path(os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData/Local"))) / "Android/Sdk"
    return Path.home() / ("Library/Android/sdk" if system == "Darwin" else "Android/Sdk")

def run(args, **kwargs):
    if platform.system() == "Darwin":
        env = dict(kwargs.get("env", os.environ))
        env.setdefault("DEVELOPER_DIR", "/Applications/Xcode.app/Contents/Developer")
        kwargs["env"] = env
    subprocess.run(args, cwd=ROOT, check=True, **kwargs)

def main():
    sdk = Path(os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT") or default_sdk())
    ndk = Path(os.environ.get("ANDROID_NDK_HOME") or sdk / "ndk" / NDK_VERSION)
    host, library_name, bindgen_name, compiler_suffix, executable_suffix = host_tools()
    toolchain = ndk / "toolchains/llvm/prebuilt" / host / "bin"
    if not toolchain.is_dir():
        raise SystemExit(f"NDK toolchain missing: {toolchain}. Install NDK {NDK_VERSION} in Android Studio's SDK Manager.")
    host_env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target"))
    run(["cargo", "build", "--locked", "-p", "dgl-uniffi", "--features", "cli"], env=host_env)
    library = ROOT / "target/debug" / library_name
    generated = ROOT / "coreBindings/src/main/kotlin"
    run([str(ROOT / "target/debug" / bindgen_name), "generate", "--library", str(library), "--language", "kotlin",
         "--out-dir", str(generated), "--no-format"])
    # Bindgen emits a flat file even for a nested Kotlin package.
    destination = generated / "uniffi/dgl_core"
    destination.mkdir(parents=True, exist_ok=True)
    for source in generated.glob("*.kt"):
        shutil.move(source, destination / source.name)
    for abi, (target, clang) in TARGETS.items():
        env = os.environ.copy()
        env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(toolchain / f"{clang}34-clang{compiler_suffix}")
        env[f"CC_{target.replace('-', '_')}"] = str(toolchain / f"{clang}34-clang{compiler_suffix}")
        env[f"AR_{target.replace('-', '_')}"] = str(toolchain / f"llvm-ar{executable_suffix}")
        env["CARGO_TARGET_DIR"] = str(ROOT / "target/android")
        run(["cargo", "build", "--locked", "--release", "-p", "dgl-uniffi", "--target", target], env=env)
        output = ROOT / "coreBindings/src/main/jniLibs" / abi
        output.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "target/android" / target / "release/libdgl_uniffi.so", output)

if __name__ == "__main__":
    main()
