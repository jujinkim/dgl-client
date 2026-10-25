#!/usr/bin/env python3
"""Create a separate local GPUI candidate app; never installs or changes the default app."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
from collect_rust_notices import collect

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    env = dict(os.environ, DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer")
    subprocess.run(["cargo", "build", "--locked", "-p", "dgl-desktop"] + (["--release"] if args.release else []), cwd=ROOT, env=env, check=True)
    output = ROOT / "artifacts/rust-candidate/mac"
    app = output / "dgl Rust Candidate.app/Contents"
    (app / "MacOS").mkdir(parents=True, exist_ok=True)
    (app / "Resources").mkdir(exist_ok=True)
    binary = app / "MacOS/dgl-desktop"
    shutil.copy2(ROOT / "target" / ("release" if args.release else "debug") / "dgl-desktop", binary)
    (app / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": "kim.jujin.dglc", "CFBundleName": "dgl Rust Candidate",
        "CFBundleExecutable": "dgl-desktop", "CFBundlePackageType": "APPL", "CFBundleVersion": "1", "CFBundleShortVersionString": "0.1.0",
        "NSHighResolutionCapable": True}))
    for name in ["NOTICE", "LICENSE-APACHE-GPUI"]:
        shutil.copy2(ROOT / "desktop" / name, app / "Resources" / name)
    shutil.copy2(ROOT / "licenses/simple-icons-LICENSE.md", app / "Resources/simple-icons-LICENSE.md")
    collect("dgl-desktop", app / "Resources/third-party-rust")
    # Local ad-hoc signing only; no Developer ID or notarization is claimed.
    subprocess.run(["codesign", "--force", "--sign", "-", str(app.parent)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", str(app.parent)], check=True)
    (output / "build.json").write_text(json.dumps({"profile": "release" if args.release else "debug", "arch": "aarch64-apple-darwin",
        "signing": "local-ad-hoc", "notarized": False, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest()}, indent=2) + "\n")
    print(app.parent)

if __name__ == "__main__":
    main()
