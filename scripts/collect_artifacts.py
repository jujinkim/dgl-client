#!/usr/bin/env python3
"""Verify APK policy and complete third-party materials, then collect deliverables."""
import hashlib
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import zipfile
from audit_dependencies import ROOT
from verify_licenses import main as verify_licenses
from collect_rust_android import verify_rust_payload
from evidence import context, require_passed, safe_relative, sha256, write_report


def license_files():
    return [p for p in (ROOT / 'licenses').rglob('*') if p.is_file() and
            not {'build', '.gradle', '.kotlin', '__pycache__'} & set(p.relative_to(ROOT / 'licenses').parts)]


def verify_apk(source, aapt, release=False):
    badging = subprocess.check_output([str(aapt), 'dump', 'badging', str(source)], text=True, encoding="utf-8")
    for required in ["name='kim.jujin.rovessh'", "minSdkVersion:'34'", "targetSdkVersion:'36'", "compileSdkVersion='36'"]:
        if required not in badging:
            raise ValueError('Unexpected APK identity or SDK: ' + required)
    manifest = subprocess.check_output([str(aapt), 'dump', 'xmltree', str(source), '--file', 'AndroidManifest.xml'], text=True, encoding="utf-8")
    for policy in ['allowBackup', 'fullBackupContent', 'usesCleartextTraffic']:
        if not re.search(r':' + policy + r'\(0x[0-9a-f]+\)=false', manifest):
            raise ValueError('APK policy missing: ' + policy)
    if 'dataExtractionRules' not in manifest or 'kim.jujin.rovessh.MainActivity' not in manifest:
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


def verify_desktop_notices(directory):
    required = ['THIRD_PARTY_NOTICES.md', 'licenses/dependencies.json', 'licenses/native-components.json',
                'licenses/jediterm-rebuild/README.md', 'licenses/jediterm-rebuild/build.gradle.kts',
                'jediterm-core-3.59-sources.jar', 'jediterm-ui-3.59-sources.jar', 'jvm/release', 'SHA256SUMS']
    if any(not (directory / name).is_file() for name in required) or not list((directory / 'jvm/legal').rglob('LICENSE')):
        raise ValueError('Incomplete desktop source/license/JVM bundle')
    checksums = {}
    for line in (directory / 'SHA256SUMS').read_text(encoding="utf-8").splitlines():
        digest, name = line.split('  ', 1)
        safe_relative(name)
        if name in checksums or not re.fullmatch('[a-f0-9]{64}', digest):
            raise ValueError('Unsafe or duplicate notice manifest path')
        checksums[name] = digest
    files = {p.relative_to(directory).as_posix(): p for p in directory.rglob('*') if p.is_file() and p != directory / 'SHA256SUMS'}
    if files.keys() != checksums.keys():
        raise ValueError('Notice manifest file set mismatch')
    for name, path in files.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != checksums[name]:
            raise ValueError('Notice bundle hash mismatch: ' + name)
    for path in license_files() + [ROOT / 'THIRD_PARTY_NOTICES.md']:
        if (directory / path.relative_to(ROOT)).read_bytes() != path.read_bytes():
            raise ValueError('Stale desktop license bundle')
    import xml.etree.ElementTree as ET
    metadata = ET.parse(ROOT / 'gradle/verification-metadata.xml')
    for name in ['jediterm-core', 'jediterm-ui']:
        jar = f'{name}-3.59-sources.jar'
        component = next(c for c in metadata.findall('.//{*}component') if c.attrib == {'group': 'org.jetbrains.jediterm', 'name': name, 'version': '3.59'})
        artifact = next(a for a in component.findall('{*}artifact') if a.attrib['name'] == jar)
        if checksums[jar] not in {v.attrib['value'] for v in artifact.findall('{*}sha256')}:
            raise ValueError('Unverified JediTerm sources')
    return len(files)


def verify_windows_reports(destination, sources, expected_context):
    from verify_test_results import EXPECTED
    reports = {}
    for name in ('windows-tests', 'windows-dpapi', 'windows-installation'):
        report = json.loads((destination / (name + '.json')).read_text(encoding='utf-8'))
        require_passed(report, expected_context)
        if report['context'].get('os') != 'Windows':
            raise ValueError('Windows evidence required')
        reports[name] = report
    tests = reports['windows-tests']
    allowed = {n for n in EXPECTED if any('.' + cls + '.' in n for cls in
               ('IntegrationTest', 'RecoveryTest', 'PerformanceTest'))}
    if tests.get('mode') != 'windows' or tests.get('cases') != {n: 'skipped' if n in allowed else 'passed' for n in EXPECTED}:
        raise ValueError('Missing Windows tests or unexpected skips')
    if reports['windows-dpapi'].get('checks') != {'ownRoundTrips': 2, 'foreignReadable': 2, 'foreignDecryptDenied': 2, 'cleanup': True}:
        raise ValueError('Incomplete DPAPI user isolation evidence')
    installed = reports['windows-installation'].get('installers', [])
    if len(installed) != 2 or {r['kind'] for r in installed} != {'msi', 'exe'}:
        raise ValueError('Both installer reports required')
    for record in installed:
        path = sources[record['kind']]
        if record.get('status') != 'passed' or record.get('sha256') != sha256(path) or record.get('name') != path.name:
            raise ValueError('Installer verification/hash mismatch')
        if record.get('unregistered') is not True or record.get('noResidualFiles') is not True or record.get('noticeFiles', 0) < 1:
            raise ValueError('Missing notice/uninstall evidence')
        if not record.get('registration', {}).get('productCode') or record.get('installExitCode') not in (0, 3010):
            raise ValueError('Missing installation evidence')
        for field in ('launch', 'replacementLaunch'):
            launch = record.get(field, {})
            if launch.get('status') != 'passed' or launch.get('aliveSeconds', 0) < 8 or launch.get('scope') != 'process-liveness-only':
                raise ValueError('Missing launcher evidence')
        jars = record.get('jediterm', [])
        if len(jars) != 2 or {j['name'] for j in jars} != {'jediterm-core-3.59.jar', 'jediterm-ui-3.59.jar'}:
            raise ValueError('Incomplete JediTerm replacement')
        for jar in jars:
            if any(not re.fullmatch('[a-f0-9]{64}', jar.get(key, '')) for key in ('originalSha256', 'rebuiltSha256')):
                raise ValueError('Missing replacement hashes')
        source = record.get('temurin', {})
        if source.get('vendor') != 'Eclipse Adoptium' or source.get('runtimeLegalFiles', 0) < 1 or any(
                not re.fullmatch('[a-f0-9]{64}', source.get(key, '')) for key in ('sourceSha256', 'jvmSha256')):
            raise ValueError('Incomplete Temurin source/runtime evidence')
    return reports


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--platform', choices=['android'], default='android')
    args = parser.parse_args()
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
        'context': context(), 'platform': 'android', 'applicationId': 'kim.jujin.rovessh', 'minSdk': 34,
        'compileSdk': 36, 'targetSdk': 36, 'androidSigning': 'debug; release APK only verified unsigned',
        'engine': 'rust', 'apkChecks': checks, 'files': manifest}, indent=2) + '\n', encoding="utf-8")
    (destination / 'SHA256SUMS').write_text(''.join(item['sha256'] + '  ' + item['name'] + '\n' for item in manifest), encoding="utf-8")
    print('Validated and collected: ' + ', '.join(item['name'] for item in manifest))


if __name__ == '__main__':
    main()
