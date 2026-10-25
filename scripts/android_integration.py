#!/usr/bin/env python3
"""Called by integration.py; transfer disposable test credentials over stdin only."""
import json
import os
from pathlib import Path
import subprocess
from evidence import context, write_report
from verify_android_results import verify

sdk = Path(os.environ['ANDROID_HOME'])
adb = str(sdk / 'platform-tools/adb')
serial = os.environ.get('ANDROID_SERIAL', 'emulator-5554')
base = [adb, '-s', serial]
port = os.environ['DGL_TEST_PORT']

def command(args, **kwargs):
    kwargs.setdefault('timeout', 120)
    return subprocess.run(base + args, check=True, capture_output=True, **kwargs)

api = command(['shell', 'getprop', 'ro.build.version.sdk'], text=True, encoding='utf-8').stdout.strip()
if api not in {'34', '35', '36'}:
    raise ValueError('Unexpected Android API')
report_path = Path('artifacts') / ('android-tests-' + api + '.json')
report = {'schema': 1, 'complete': False, 'status': 'failed', 'context': context(), 'api': api}
write_report(report_path, report)

subprocess.run(['./gradlew', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'], check=True, timeout=600)
command(['install', '-r', 'androidApp/build/outputs/apk/debug/androidApp-debug.apk'])
command(['install', '-r', 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk'])
command(['reverse', f'tcp:{port}', f'tcp:{port}'])
keyboard_setting = command(['shell', 'settings', 'get', 'secure', 'show_ime_with_hard_keyboard'], text=True, encoding="utf-8").stdout.strip()
command(['shell', 'settings', 'put', 'secure', 'show_ime_with_hard_keyboard', '1'])
fixture = {
    'port': int(port), 'user': os.environ['DGL_TEST_USER'], 'fingerprint': os.environ['DGL_TEST_FINGERPRINT'],
    'key': Path(os.environ['DGL_TEST_KEY_FILE']).read_text(encoding="utf-8"),
    'rsa': Path(os.environ['DGL_TEST_RSA_FILE']).read_text(encoding="utf-8"),
    'encrypted': Path(os.environ['DGL_TEST_ENCRYPTED_FILE']).read_text(encoding="utf-8"),
    'passphrase': os.environ['DGL_TEST_PASSPHRASE'],
}
try:
    command(['shell', 'run-as', 'kim.jujin.dglc', 'sh', '-c', '"mkdir -p cache && cat > cache/integration.json"'], input=json.dumps(fixture).encode())
    result = command(['shell', 'am', 'instrument', '-w', '-r', '-e', 'dgl.fixture', 'true',
                      'kim.jujin.dglc.test/androidx.test.runner.AndroidJUnitRunner'], text=True, encoding="utf-8", timeout=240)
    print(result.stdout)
    if 'FAILURES' in result.stdout or 'INSTRUMENTATION_FAILED' in result.stdout or 'OK (' not in result.stdout:
        diagnostic = subprocess.run(base + ['exec-out', 'run-as', 'kim.jujin.dglc', 'cat', 'cache/ui-failure.png'], capture_output=True)
        if diagnostic.returncode == 0 and diagnostic.stdout.startswith(b'\x89PNG'):
            Path('.tools/p403-ui-failure.png').write_bytes(diagnostic.stdout)
        raise SystemExit(1)
    report.update(verify(result.stdout))
finally:
    command(['shell', 'run-as', 'kim.jujin.dglc', 'rm', '-f', 'cache/integration.json'])
    command(['shell', 'run-as', 'kim.jujin.dglc', 'rm', '-f', 'cache/ui-failure.png'])
    command(['reverse', '--remove', f'tcp:{port}'])
    if keyboard_setting == 'null':
        command(['shell', 'settings', 'delete', 'secure', 'show_ime_with_hard_keyboard'])
    else:
        command(['shell', 'settings', 'put', 'secure', 'show_ime_with_hard_keyboard', keyboard_setting])

report.update(complete=True, status='passed')
write_report(report_path, report)
