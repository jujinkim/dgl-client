#!/usr/bin/env python3
# Build an Ubuntu 24.04 x86_64 Release AppImage; never starts the GUI or installs tmux.
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tempfile
import urllib.request
from collect_rust_notices import collect
ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
TOOLS = {
    "appimagetool": ("https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
                     "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"),
    "runtime": ("https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
                "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"),
}
# OS and graphics driver libraries must come from the supported host.
SYSTEM = re.compile(r"^(ld-linux|lib(c|m|pthread|dl|rt|resolv|util|stdc\+\+|gcc_s|nss_[^.]+)\.so|lib(GL|EGL|GLX|GLdispatch|vulkan|drm)[^.]*\.so)")
APP_RUN = '''#!/bin/sh
set -eu
APPDIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
export APPDIR
export LD_LIBRARY_PATH="$APPDIR/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$APPDIR/usr/bin/dgl-desktop" "$@"
'''
DESKTOP = '''[Desktop Entry]
Type=Application
Name=DaGaLi
Comment=Persistent SSH and local tmux workspaces
Exec=dgl-desktop
Icon=kim.jujin.dglc
Terminal=false
Categories=System;TerminalEmulator;
StartupWMClass=kim.jujin.dglc
'''


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tool(name, cache):
    url, digest = TOOLS[name]
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / f"{name}-{digest[:16]}"
    if not path.exists():
        with tempfile.NamedTemporaryFile(dir=cache, delete=False) as out:
            temporary = Path(out.name)
            try:
                with urllib.request.urlopen(url, timeout=120) as source:
                    shutil.copyfileobj(source, out)
                out.close()
                if sha256(temporary) != digest:
                    raise ValueError(f"{name} SHA-256 mismatch")
                temporary.replace(path)
            finally:
                temporary.unlink(missing_ok=True)
    if sha256(path) != digest:
        raise ValueError(f"{name} SHA-256 mismatch; remove corrupt cache file")
    path.chmod(0o755)
    return path


def verify_executable(path):
    with path.open('rb') as binary:
        header = binary.read(64)
    if len(header) < 64 or header[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", header, 18)[0] != 62:
        raise ValueError("Expected an x86_64 Linux ELF executable")


def bundle_libraries(binary, appdir):
    libraries = appdir / "usr/lib"
    libraries.mkdir(parents=True)
    dynamic = ["libxkbcommon.so.0", "libxkbcommon-x11.so.0", "libwayland-client.so.0", "libwayland-cursor.so.0"]
    cache = subprocess.check_output(["ldconfig", "-p"], text=True)
    roots = [str(binary)]
    for name in dynamic:
        match = re.search(r"^\s*" + re.escape(name) + r"\s+.*=> (/\S+)", cache, re.MULTILINE)
        if not match:
            raise ValueError(f"Missing dynamically loaded frontend library: {name}")
        roots.append(match.group(1))
    found = {}
    for source in roots:
        output = subprocess.check_output(["ldd", source], text=True)
        if "not found" in output:
            raise ValueError("Unresolved dynamic library: " + output)
        for line in output.splitlines():
            match = re.search(r"(\S+) => (/\S+) ", line)
            if match:
                found[match.group(1)] = match.group(2)
    found.update({Path(source).name: source for source in roots[1:]})
    inventory = []
    for name, source in sorted(found.items()):
        if SYSTEM.match(name):
            continue
        shutil.copy2(source, libraries / name)
        inventory.append({"name": name, "sha256": sha256(libraries / name), "source": source})
    notices = appdir / "usr/share/doc/DaGaLi/system-libraries"
    notices.mkdir(parents=True)
    for entry in inventory:
        owner = None
        for candidate in dict.fromkeys([entry["source"], str(Path(entry["source"]).resolve()), entry["source"].replace("/lib/", "/usr/lib/", 1)]):
            result = subprocess.run(["dpkg-query", "-S", candidate], text=True, capture_output=True)
            if result.returncode == 0 and result.stdout.strip():
                owner = result.stdout.splitlines()[0].split(": ", 1)[0]
                break
        if owner is None:
            raise ValueError(f"Cannot identify distribution license owner for {entry['source']}")
        package = owner.split(":", 1)[0]
        copyright_file = Path("/usr/share/doc") / package / "copyright"
        if not copyright_file.is_file():
            raise ValueError(f"Missing distribution copyright for {entry['name']} ({owner})")
        shutil.copy2(copyright_file, notices / f"{package}-copyright")
        entry["package"] = owner
    (notices / "libraries.json").write_text(json.dumps(inventory, indent=2) + "\n")


def stage(binary, appdir, version, with_libraries=True):
    verify_executable(binary)
    (appdir / "usr/bin").mkdir(parents=True)
    shutil.copy2(binary, appdir / "usr/bin/dgl-desktop")
    (appdir / "usr/bin/dgl-desktop").chmod(0o755)
    (appdir / "AppRun").write_text(APP_RUN)
    (appdir / "AppRun").chmod(0o755)
    (appdir / "kim.jujin.dglc.desktop").write_text(DESKTOP)
    icons = appdir / "usr/share/icons/hicolor/1024x1024/apps"
    icons.mkdir(parents=True)
    for target in [icons / "kim.jujin.dglc.png", appdir / "kim.jujin.dglc.png", appdir / ".DirIcon"]:
        shutil.copy2(ROOT / "assets/app-icon/icon.png", target)
    applications = appdir / "usr/share/applications"
    applications.mkdir(parents=True)
    shutil.copy2(appdir / "kim.jujin.dglc.desktop", applications)
    docs = appdir / "usr/share/doc/DaGaLi"
    docs.mkdir(parents=True)
    for source in [ROOT / "desktop/NOTICE", ROOT / "desktop/LICENSE-APACHE-GPUI", ROOT / "licenses/simple-icons-LICENSE.md", ROOT / "docs/LINUX.md"]:
        shutil.copy2(source, docs)
    runtime_notices = ROOT / "desktop/resources/linux"
    provenance = json.loads((runtime_notices / "AppImage-runtime-source.json").read_text())
    if provenance["runtimeSha256"] != TOOLS["runtime"][1]:
        raise ValueError("AppImage runtime source evidence does not match the pinned runtime")
    if sha256(runtime_notices / "AppImage-runtime-LICENSE") != provenance["licenseSha256"]:
        raise ValueError("AppImage runtime license SHA-256 mismatch")
    for source in runtime_notices.iterdir():
        if source.is_file(): shutil.copy2(source, docs)
    if with_libraries:
        bundle_libraries(binary, appdir)
    build = {"version": version, "profile": "release", "arch": TARGET, "minimumOs": "Ubuntu 24.04",
             "binarySha256": sha256(binary), "runtimeValidation": "not-performed", "tools": TOOLS}
    (docs / "build.json").write_text(json.dumps(build, indent=2) + "\n")
    return build


def main():
    parser = argparse.ArgumentParser(description="Build the Ubuntu 24.04 x86_64 AppImage")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/rust-candidate/linux")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("Build natively on Ubuntu 24.04 x86_64 (see docs/LINUX.md)")
    os_release = dict(line.split("=", 1) for line in Path("/etc/os-release").read_text().splitlines() if "=" in line)
    os_release = {key: value.strip('"\'') for key, value in os_release.items()}
    if os_release.get("ID") != "ubuntu" or os_release.get("VERSION_ID") != "24.04":
        parser.error("The distribution build baseline is Ubuntu 24.04; newer OS builds can break compatibility")
    subprocess.run(["cargo", "build", "--locked", "--release", "-p", "dgl-desktop", "--target", TARGET], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "dgl-desktop")
    binary = Path(metadata["target_directory"]) / TARGET / "release/dgl-desktop"
    args.output.mkdir(parents=True, exist_ok=True)
    cache = ROOT / ".tools/appimage"
    appimagetool, runtime = tool("appimagetool", cache), tool("runtime", cache)
    with tempfile.TemporaryDirectory(prefix="dgl-linux-", dir=args.output) as temp:
        appdir = Path(temp) / "DaGaLi.AppDir"
        build = stage(binary, appdir, version)
        collect("dgl-desktop", appdir / "usr/share/doc/DaGaLi/third-party-rust", target=TARGET)
        subprocess.run(["desktop-file-validate", str(appdir / "kim.jujin.dglc.desktop")], check=True)
        image = Path(temp) / f"DaGaLi-{version}-x86_64.AppImage"
        subprocess.run([str(appimagetool), "--appimage-extract-and-run", "--runtime-file", str(runtime), str(appdir), str(image)],
                       env={**os.environ, "ARCH": "x86_64", "VERSION": version}, check=True)
        image.chmod(0o755)
        subprocess.run([str(image.resolve()), "--appimage-extract"], cwd=temp, check=True, stdout=subprocess.DEVNULL)
        extracted = Path(temp) / "squashfs-root"
        verify_executable(extracted / "usr/bin/dgl-desktop")
        if sha256(extracted / "usr/bin/dgl-desktop") != build["binarySha256"]:
            raise ValueError("Packaged binary does not match Release build")
        image.replace(args.output / image.name)
        (args.output / "build.json").write_text(json.dumps(build, indent=2) + "\n")
        (args.output / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.name}\n" for p in [args.output / image.name, args.output / "build.json"]))
    print(args.output / image.name)


if __name__ == "__main__":
    main()
