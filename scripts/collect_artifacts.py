#!/usr/bin/env python3
"""Verify Android APK policy, Rust payload and notices, then collect deliverables."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import zipfile
from evidence import ROOT, context
from verify_licenses import main as verify_licenses
from collect_rust_android import verify_rust_payload


def license_files():
    return [p for p in (ROOT / 'licenses').rglob('*') if p.is_file() and
            not {'build', '.gradle', '.kotlin', '__pycache__'} & set(p.relative_to(ROOT / 'licenses').parts)]


def verify_apk(source, aapt, release=False):
    badging = subprocess.check_output([str(aapt), 'dump', 'badging', str(source)], text=True, encoding="utf-8")
    for required in ["name='kim.jujin.dglc'", "minSdkVersion:'34'", "targetSdkVersion:'36'", "compileSdkVersion='36'"]:
        if required not in badging:
            raise ValueError('Unexpected APK identity or SDK: ' + required)
    manifest = subprocess.check_output([str(aapt), 'dump', 'xmltree', str(source), '--file', 'AndroidManifest.xml'], text=True, encoding="utf-8")
    for policy in ['allowBackup', 'fullBackupContent', 'usesCleartextTraffic']:
        if not re.search(r':' + policy + r'\(0x[0-9a-f]+\)=false', manifest):
            raise ValueError('APK policy missing: ' + policy)
    if 'dataExtractionRules' not in manifest or 'kim.jujin.dglc.MainActivity' not in manifest:
        raise ValueError('Missing app entry or backup rules')
    if release and re.search(r':debuggable\(0x[0-9a-f]+\)=true', manifest):
        raise ValueError('Release APK is debuggable')
    # AGP 9 optimizes release resource paths (for example res/4j.xml).
    rule_id = re.search(r':dataExtractionRules\(0x[0-9a-f]+\)=@(0x[0-9a-f]+)', manifest)
    resources = subprocess.check_output([str(aapt), 'dump', 'resources', str(source)], text=True, encoding="utf-8")
    rule_file = None if rule_id is None else re.search(r'resource ' + rule_id.group(1) + r' xml/data_extraction_rules\s+\(\) \(file\) (\S+) type=XML', resources)
    if rule_file is None:
        raise ValueError('Cannot resolve manifest backup rules resource')
    rules = subprocess.check_output([str(aapt), 'dump', 'xmltree', str(source), '--file', rule_file.group(1)], text=True, encoding="utf-8")
    if 'E: cloud-backup' not in rules or 'E: device-transfer' not in rules or rules.count('E: exclude') != 10:
        raise ValueError('Backup policy exclusions changed')
    for domain in ['root', 'file', 'database', 'sharedpref', 'external']:
        if len(re.findall(r'A: domain="' + domain + '"', rules)) != 2:
            raise ValueError('Backup domain exclusion changed: ' + domain)
    if rules.count('A: path="."') != 10 or 'A: disableIfNoEncryptionCapabilities=true' not in rules:
        raise ValueError('Backup paths or encryption policy changed')
    with zipfile.ZipFile(source) as apk:
        libraries = verify_rust_payload(apk)
        names = apk.namelist()
        if any(re.search(r'(^|/)(settings\.json|integration\.json|authorized_keys|id_rsa|id_ed25519|local\.properties|\.env)$|\.(keystore|jks|sealed|log)$|/(\.gradle|\.tools)/', n) for n in names):
            raise ValueError('Private runtime/build file found in APK')
        terminal = ROOT / 'app/src/androidMain/assets/terminal'
        expected = {'assets/terminal/' + p.relative_to(terminal).as_posix(): p for p in terminal.rglob('*') if p.is_file()}
        if {n for n in names if n.startswith('assets/terminal/') and not n.endswith('/')} != set(expected):
            raise ValueError('Terminal asset set differs from source')
        expected['assets/third-party/THIRD_PARTY_NOTICES.md'] = ROOT / 'THIRD_PARTY_NOTICES.md'
        for path in license_files():
            expected['assets/third-party/' + path.relative_to(ROOT).as_posix()] = path
        for name, path in expected.items():
            if apk.read(name) != path.read_bytes():
                raise ValueError('Bundled asset mismatch: ' + name)
    return {'name': source.name, 'policy': 'passed', 'terminalAssets': len(list(terminal.iterdir())), 'release': release, 'engine': 'rust', 'librarySha256': libraries}


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    verify_licenses()
    destination = ROOT / 'artifacts'
    destination.mkdir(exist_ok=True)
    sdk = Path(os.environ['ANDROID_HOME'])
    aapt = sdk / 'build-tools/36.0.0' / ('aapt2.exe' if os.name == 'nt' else 'aapt2')
    base = ROOT / 'androidApp/build/outputs/apk'
    source = base / 'debug/androidApp-debug.apk'
    checks = [verify_apk(source, aapt), verify_apk(base / 'release/androidApp-release-unsigned.apk', aapt, release=True)]
    apk_path = destination / 'dgl-client-0.1.0-debug.apk'
    shutil.copyfile(source, apk_path)
    files = [apk_path]
    manifest = [{'name': p.name, 'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in files]
    (destination / 'build-manifest.json').write_text(json.dumps({'schema': 1, 'complete': True, 'status': 'passed',
        'context': context(), 'platform': 'android', 'applicationId': 'kim.jujin.dglc', 'minSdk': 34,
        'compileSdk': 36, 'targetSdk': 36, 'androidSigning': 'debug; release APK only verified unsigned',
        'engine': 'rust', 'apkChecks': checks, 'files': manifest}, indent=2) + '\n', encoding="utf-8")
    (destination / 'SHA256SUMS').write_text(''.join(item['sha256'] + '  ' + item['name'] + '\n' for item in manifest), encoding="utf-8")
    print('Validated and collected: ' + ', '.join(item['name'] for item in manifest))


if __name__ == '__main__':
    main()
