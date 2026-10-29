#!/usr/bin/env python3
"""Collect registry-provided license texts and a locked dependency inventory for a candidate."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from build_rust_android import TARGETS as ANDROID_TARGETS

ROOT = Path(__file__).resolve().parents[1]
# Packages shipped only for fixed targets; notices follow those targets when no --target is given.
PACKAGE_TARGETS = {"dgl-uniffi": tuple(target for target, _ in ANDROID_TARGETS.values())}

def host_target():
    output = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True)
    return next(line.split(":", 1)[1].strip() for line in output.splitlines() if line.startswith("host:"))

def collect(package, output, target=None):
    targets = [target] if isinstance(target, str) else list(target or PACKAGE_TARGETS.get(package, ()))
    if not targets:
        raise ValueError(f"No packaging target for {package}; notices must follow the shipped platform")
    command = ["cargo", "metadata", "--locked", "--format-version", "1"]
    for item in targets:
        command += ["--filter-platform", item]
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    start = next(p["id"] for p in metadata["packages"] if p["name"] == package)
    pending, seen = [start], set()
    while pending:
        item = pending.pop()
        if item in seen:
            continue
        seen.add(item)
        pending.extend(nodes[item]["dependencies"])
    if (output / "DEPENDENCIES.json").is_file():
        shutil.rmtree(output)  # A previous collection; drop crates no longer in the filtered graph.
    output.mkdir(parents=True, exist_ok=True)
    inventory = []
    for identifier in sorted(seen):
        p = packages[identifier]
        if not p["source"]:
            continue
        source = Path(p["manifest_path"]).parent
        dest = output / f'{p["name"]}-{p["version"]}'
        dest.mkdir(exist_ok=True)
        files = set()
        for pattern in ["LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "licenses/*", "LICENSES/*"]:
            files.update(f for f in source.glob(pattern) if f.is_file() and not f.is_symlink())
        if p.get("license_file"):
            license_file = source / p["license_file"]
            if license_file.is_file() and source in license_file.resolve().parents:
                files.add(license_file)
        hashes = {}
        for file in sorted(files):
            relative = file.relative_to(source)
            target = dest / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, target)
            hashes[str(relative)] = hashlib.sha256(file.read_bytes()).hexdigest()
        shutil.copy2(source / "Cargo.toml", dest / "Cargo.toml")
        inventory.append({"name": p["name"], "version": p["version"], "license": p["license"],
                          "repository": p["repository"], "licenseFiles": hashes,
                          "source": f'https://crates.io/crates/{p["name"]}/{p["version"]}'})
    (output / "DEPENDENCIES.json").write_text(json.dumps(inventory, indent=2) + "\n")
    (output / "README.txt").write_text("Registry-provided license texts for the locked candidate dependency graph.\n"
                                    f"Filtered to packaged target(s): {', '.join(targets)}; build dependencies of that graph are included. Cargo.lock records registry checksums.\n"
                                    "A missing license text is recorded by an empty licenseFiles map; no license is invented.\n")
    print(f"Collected {len(inventory)} Rust dependency entries for {package}; {sum(not i['licenseFiles'] for i in inventory)} use manifest license declarations only.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", action="append", help="Packaged target triple (repeatable); defaults to the package's shipped targets")
    args = parser.parse_args()
    collect(args.package, args.output, args.target)
