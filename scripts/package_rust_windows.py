#!/usr/bin/env python3
"""Build the unsigned Windows x64 portable ZIP, plus an optional Microsoft Store MSIX (--msix).

Does not run, install, or sign the app. The Store signs the MSIX when it is submitted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
from string import Template
import struct
import subprocess
import tempfile
from xml.etree import ElementTree
import zipfile

from collect_rust_notices import collect

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-pc-windows-msvc"
NAME = "dgl-rust-windows-x64"
MSIX = ROOT / "desktop/resources/windows/msix"
APPX = "{http://schemas.microsoft.com/appx/manifest/foundation/windows10}"
# ZIP-only guidance and hashes; the MSIX has its own block map and Store signature.
ZIP_ONLY = {"README.txt", "build.json", "SHA256SUMS"}


def verify_executable(path):
    """Reject missing, truncated, non-PE, or wrong-architecture build output."""
    with path.open("rb") as binary:
        header = binary.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise ValueError("Expected a Windows PE executable")
        offset = struct.unpack_from("<I", header, 60)[0]
        binary.seek(offset)
        pe = binary.read(26)
        if len(pe) != 26 or pe[:4] != b"PE\0\0" or struct.unpack_from("<H", pe, 4)[0] != 0x8664:
            raise ValueError("Expected an x64 Windows PE executable")
        if struct.unpack_from("<H", pe, 24)[0] != 0x20b:
            raise ValueError("Expected PE32+ optional header")


def msix_version(version):
    """Map Cargo's x.y.z to MSIX x.y.z.0; the Store reserves the revision field."""
    parts = version.split(".")
    if len(parts) != 3 or not all(part.isdigit() and int(part) <= 65535 for part in parts):
        raise ValueError(f"MSIX needs a numeric x.y.z version with fields up to 65535: {version}")
    return f"{version}.0"


def msix_manifest(version):
    """Fill the checked-in manifest and reject malformed XML before makeappx runs."""
    text = Template((MSIX / "AppxManifest.xml").read_text(encoding="utf-8")).substitute(
        version=msix_version(version))
    return text, ElementTree.fromstring(text.encode("utf-8")).find(APPX + "Identity")


def pack_msix(content, destination):
    subprocess.run(["makeappx", "pack", "/o", "/d", str(content), "/p", str(destination)], check=True)


def package(binary, output, version, msix=False):
    """Publish the ZIP (and MSIX when requested) atomically; a ZIP-only run drops any stale MSIX."""
    verify_executable(binary)
    manifest, identity = msix_manifest(version) if msix else (None, None)
    output.mkdir(parents=True, exist_ok=True)
    # Build in a fresh directory so old DLLs/files cannot leak into a new ZIP.
    with tempfile.TemporaryDirectory(prefix="dgl-windows-", dir=output) as temp:
        stage = Path(temp) / NAME
        stage.mkdir()
        shutil.copy2(binary, stage / "dgl-desktop.exe")
        for name in ["NOTICE", "LICENSE-APACHE-GPUI"]:
            shutil.copy2(ROOT / "desktop" / name, stage / name)
        shutil.copy2(ROOT / "licenses/simple-icons-LICENSE.md", stage / "simple-icons-LICENSE.md")
        collect("dgl-desktop", stage / "third-party-rust", target=TARGET)
        (stage / "README.txt").write_text(
            "DaGaLi - Windows x64\n\n"
            "Extract the entire ZIP, then run dgl-desktop.exe.\n"
            "Unsigned development candidate; Windows runtime acceptance is pending.\n"
            "Requires Windows 10/11 x64 and a Direct3D 11 capable graphics driver.\n"
            "Settings: %LOCALAPPDATA%\\dgl-rust-candidate-v1\\settings.json\n"
            "Credentials: current-user DPAPI, in the credentials subdirectory.\n"
            "DGL_CANDIDATE_DIR overrides both locations. No legacy data migration.\n"
            "Terminal copy/paste: Ctrl+Shift+C / Ctrl+Shift+V. Ctrl+C goes to SSH.\n"
            "Optional OS notifications register a per-user Start Menu shortcut and COM activator.\n"
            "App notification ID: kim.jujin.dglc. No administrator privileges required.\n"
            "After moving this folder, launch the app directly to refresh notification registration.\n"
            "Native Windows toast display/click validation is pending; see docs/WINDOWS.md section 7.\n"
            "See docs/WINDOWS.md in the source repository for build/test steps.\n",
            encoding="utf-8",
        )
        build = {
            "version": version, "profile": "release", "arch": TARGET,
            "signing": "unsigned", "runtimeValidation": "not-performed",
            "crt": "static", "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        }
        if msix:
            build.update(msixVersion=identity.get("Version"), msixIdentity=identity.get("Name"),
                         msixPublisher=identity.get("Publisher"))
        (stage / "build.json").write_text(json.dumps(build, indent=2) + "\n", encoding="utf-8")
        files = sorted(p for p in stage.rglob("*") if p.is_file())
        (stage / "SHA256SUMS").write_text("".join(
            f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(stage).as_posix()}\n" for p in files
        ), encoding="utf-8")
        archive = Path(temp) / f"{NAME}.zip"
        # Registry archives may give license files pre-1980 timestamps. Clamp
        # only ZIP metadata; keep their contents and SHA-256 hashes unchanged.
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as zipped:
            for file in sorted(p for p in stage.rglob("*") if p.is_file()):
                zipped.write(file, f"{NAME}/{file.relative_to(stage).as_posix()}")
        packed = Path(temp) / f"{NAME}.msix"
        if msix:
            content = Path(temp) / "msix"
            # shutil.copy drops the pre-1980 registry timestamps, which MSIX's ZIP container rejects.
            shutil.copytree(stage, content, copy_function=shutil.copy, ignore=lambda directory, names: [
                n for n in names if n in ZIP_ONLY and Path(directory) == stage])
            shutil.copytree(MSIX / "Assets", content / "Assets", copy_function=shutil.copy)
            (content / "AppxManifest.xml").write_text(manifest, encoding="utf-8")
            pack_msix(content, packed)
        # Publish only after every file and requested package have been generated.
        archive.replace(output / archive.name)
        if msix:
            packed.replace(output / packed.name)
        else:
            (output / packed.name).unlink(missing_ok=True)
        shutil.copy2(stage / "build.json", output / "build.json")
    packages = [output / f"{NAME}.zip"] + ([output / f"{NAME}.msix"] if msix else [])
    (output / "SHA256SUMS").write_text("".join(
        f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in packages
    ), encoding="utf-8")
    return packages


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--msix", action="store_true", help="Also build the unsigned Store MSIX (needs makeappx)")
    args = parser.parse_args()
    if platform.system() != "Windows":
        raise SystemExit("Run scripts/build_windows.ps1 on native Windows; GPUI release shaders require fxc.exe.")
    # Restrict static CRT flags to this executable build. UniFFI's cdylib tests
    # retain their default CRT configuration, as do the Mac/Android targets.
    env = dict(os.environ)
    flags = env.get("CARGO_ENCODED_RUSTFLAGS")
    if flags is None:
        flags = "\x1f".join(env.get("RUSTFLAGS", "").split())
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(filter(None, [flags, "-C", "target-feature=+crt-static"]))
    subprocess.run(["cargo", "build", "--locked", "--release", "--target", TARGET, "-p", "dgl-desktop"],
                   cwd=ROOT, env=env, check=True)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "dgl-desktop")
    binary = Path(metadata["target_directory"]) / TARGET / "release/dgl-desktop.exe"
    for path in package(binary, ROOT / "artifacts/rust-candidate/windows", version, msix=args.msix):
        print(path)


if __name__ == "__main__":
    main()
