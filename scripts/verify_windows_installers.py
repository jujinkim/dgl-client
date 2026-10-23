#!/usr/bin/env python3
"""Exercise both installers on a disposable Windows runner; export only structured evidence."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from collect_artifacts import verify_desktop_notices
from evidence import ROOT, context, sha256, write_report
from prepare_temurin_source import release_values


def installers(root=ROOT):
    found = {}
    for kind in ('msi', 'exe'):
        paths = list((root / 'app/build/compose/binaries/main' / kind).glob('*.' + kind))
        if len(paths) != 1 or paths[0].stat().st_size == 0:
            raise ValueError('Expected exactly one nonempty ' + kind + ' installer')
        found[kind] = paths[0]
    return found


def registrations():
    import winreg
    found = []
    for hive in (winreg.HKEY_LOCAL_MACHINE, winreg.HKEY_CURRENT_USER):
        for view in (winreg.KEY_WOW64_64KEY, winreg.KEY_WOW64_32KEY):
            try:
                with winreg.OpenKey(hive, r'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall', 0, winreg.KEY_READ | view) as key:
                    for i in range(winreg.QueryInfoKey(key)[0]):
                        name = winreg.EnumKey(key, i)
                        with winreg.OpenKey(key, name) as item:
                            try:
                                if winreg.QueryValueEx(item, 'DisplayName')[0] == 'dgl-client':
                                    found.append({'productCode': name,
                                        'version': winreg.QueryValueEx(item, 'DisplayVersion')[0]})
                            except FileNotFoundError:
                                pass
            except FileNotFoundError:
                pass
    return list({r['productCode']: r for r in found}.values())


def run_installer(command):
    # Logs remain local: Windows Installer logs may contain user/environment data.
    process = subprocess.Popen([str(c) for c in command], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        code = process.wait(timeout=240)
    except subprocess.TimeoutExpired:
        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], capture_output=True)
        process.wait(timeout=30)
        raise
    if code not in (0, 3010):
        raise ValueError('Installer failed with code ' + str(code))
    return code


def launch(executable, home):
    env = dict(os.environ, _JAVA_OPTIONS='-Duser.home="' + str(home) + '"')
    process = subprocess.Popen([str(executable)], cwd=executable.parent, env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise ValueError('Installed launcher exited early: ' + str(process.returncode))
            time.sleep(.25)
    finally:
        if process.poll() is None:
            subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], check=True, capture_output=True)
        process.wait(timeout=30)
    return {'status': 'passed', 'aliveSeconds': 8, 'scope': 'process-liveness-only'}


def verify_runtime(installation, notices):
    jdk = Path(os.environ['JAVA_HOME'])
    release = release_values(jdk / 'release')
    runtime = installation / 'runtime'
    if release.get('IMPLEMENTOR') != 'Eclipse Adoptium' or (notices / 'jvm/release').read_bytes() != (jdk / 'release').read_bytes():
        raise ValueError('Installed JDK identity differs from build Temurin')
    if release_values(runtime / 'release')['JAVA_VERSION'] != release['JAVA_VERSION']:
        raise ValueError('Runtime version mismatch')
    dll = runtime / 'bin/server/jvm.dll'
    if sha256(dll) != sha256(jdk / 'bin/server/jvm.dll'):
        raise ValueError('Packaged JVM differs from build JDK')
    legal = [p for p in (runtime / 'legal').rglob('*') if p.is_file()]
    if not legal or not any(p.name == 'LICENSE' for p in legal):
        raise ValueError('Runtime legal materials missing')
    for path in legal:
        if path.read_bytes() != (jdk / 'legal' / path.relative_to(runtime / 'legal')).read_bytes():
            raise ValueError('Runtime legal material mismatch')
    source = json.loads((notices / 'jvm/source/source.json').read_text(encoding='utf-8'))
    if source['runtimeVersion'] != release['JAVA_RUNTIME_VERSION'] or source['vendor'] != release['IMPLEMENTOR']:
        raise ValueError('Temurin source identity mismatch')
    if sha256(notices / 'jvm/source/temurin-sources.tar.gz') != source['sourceSha256']:
        raise ValueError('Installed Temurin source hash mismatch')
    if sha256(notices / 'jvm/src.zip') != sha256(jdk / 'lib/src.zip'):
        raise ValueError('Installed JDK Java sources mismatch')
    return dict(source, runtimeLegalFiles=len(legal), jvmSha256=sha256(dll))


def main():
    output = ROOT / 'artifacts/windows-installation.json'
    report = {'schema': 1, 'complete': False, 'status': 'failed', 'context': context(), 'installers': []}
    write_report(output, report)
    if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true':
        raise ValueError('Disposable Windows Actions runner required')
    if registrations():
        raise ValueError('Existing dgl-client installation; refusing to modify it')
    for kind, installer in installers().items():
        record = {'kind': kind, 'name': installer.name, 'sha256': sha256(installer), 'status': 'failed'}
        report['installers'].append(record)
        write_report(output, report)
        with tempfile.TemporaryDirectory(prefix='dgl-install-') as temporary:
            work = Path(temporary)
            installation = work / 'installed'
            product = None
            try:
                command = ['msiexec.exe', '/i', installer] if kind == 'msi' else [installer]
                record['installExitCode'] = run_installer(command + ['/qn', '/norestart', 'INSTALLDIR=' + str(installation)])
                registered = registrations()
                if len(registered) != 1 or registered[0]['version'] != '0.1.0':
                    raise ValueError('Missing/ambiguous installation registration')
                product = registered[0]['productCode']
                record['registration'] = registered[0]
                executable = installation / 'dgl-client.exe'
                if not executable.is_file() or not (installation / 'app').is_dir():
                    raise ValueError('Installed launcher/app missing')
                bundles = [p for p in installation.rglob('third-party') if p.is_dir()]
                if len(bundles) != 1:
                    raise ValueError('Installed third-party bundle missing or ambiguous')
                notices = bundles[0]
                record['noticeFiles'] = verify_desktop_notices(notices)
                record['temurin'] = verify_runtime(installation, notices)
                record['launch'] = launch(executable, work / 'home')
                # Execute the shipped recipe and sources, in a disposable copy to keep notices pristine.
                rebuild = work / 'rebuild'
                shutil.copytree(notices / 'licenses/jediterm-rebuild', rebuild)
                with (work / 'rebuild.log').open('w', encoding='utf-8') as log:
                    subprocess.run([str(ROOT / 'gradlew.bat'), '-p', str(rebuild), 'jar', '--no-build-cache',
                                    '--no-daemon', '-PsourcesDir=' + str(notices)], check=True, timeout=600,
                                   stdout=log, stderr=subprocess.STDOUT)
                replacements = []
                for module in ('jediterm-core', 'jediterm-ui'):
                    name = module + '-3.59.jar'
                    original = installation / 'app' / name
                    rebuilt = rebuild / module / 'build/libs' / name
                    original_sha = sha256(original)
                    shutil.copyfile(original, work / (name + '.original'))
                    shutil.copyfile(rebuilt, original)
                    if sha256(original) != sha256(rebuilt):
                        raise ValueError('JediTerm replacement did not persist')
                    replacements.append({'name': name, 'originalSha256': original_sha, 'rebuiltSha256': sha256(rebuilt)})
                record['jediterm'] = replacements
                record['replacementLaunch'] = launch(executable, work / 'home')
            finally:
                # Even a partial installer failure can register a product.
                remaining = registrations()
                for registration in remaining:
                    run_installer(['msiexec.exe', '/x', registration['productCode'], '/qn', '/norestart'])
                record['unregistered'] = not registrations()
                record['noResidualFiles'] = not installation.exists() or not any(p.is_file() for p in installation.rglob('*'))
                write_report(output, report)
            if not record['unregistered'] or not record['noResidualFiles']:
                raise ValueError('Uninstall left registration or installed files')
            record['status'] = 'passed'
            write_report(output, report)
    report.update(complete=True, status='passed')
    write_report(output, report)
    print('MSI and EXE install, notices, source rebuild/replacement, liveness and uninstall passed')


if __name__ == '__main__':
    main()
