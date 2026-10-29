#!/usr/bin/env python3
"""Called by integration.py; transfer disposable test credentials over stdin only."""
import json
import os
from pathlib import Path
import subprocess
from android_device import APKS, REMOVE_FIXTURE, RUNNER, WRITE_FIXTURE, adb_base, pull_cache_png, restore_setting, ssh_fixture
from evidence import context, write_report
from verify_android_results import verify

serial = os.environ.get('ANDROID_SERIAL', 'emulator-5554')
base = adb_base(serial)
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
for apk in APKS:
    command(['install', '-r', apk])
command(['reverse', f'tcp:{port}', f'tcp:{port}'])
keyboard_setting = command(['shell', 'settings', 'get', 'secure', 'show_ime_with_hard_keyboard'], text=True, encoding="utf-8").stdout.strip()
command(['shell', 'settings', 'put', 'secure', 'show_ime_with_hard_keyboard', '1'])
fixture = ssh_fixture()
try:
    command(WRITE_FIXTURE, input=json.dumps(fixture).encode())
    result = command(['shell', 'am', 'instrument', '-w', '-r', '-e', 'dgl.fixture', 'true', RUNNER], text=True, encoding="utf-8", timeout=240)
    print(result.stdout)
    if 'FAILURES' in result.stdout or 'INSTRUMENTATION_FAILED' in result.stdout or 'OK (' not in result.stdout:
        diagnostic = pull_cache_png(base, 'ui-failure')
        if diagnostic is not None:
            Path('.tools/p403-ui-failure.png').write_bytes(diagnostic)
        raise SystemExit(1)
    report.update(verify(result.stdout))
finally:
    command(REMOVE_FIXTURE)
    command(['shell', 'run-as', 'kim.jujin.dglc', 'rm', '-f', 'cache/ui-failure.png'])
    command(['reverse', '--remove', f'tcp:{port}'])
    command(restore_setting('secure', 'show_ime_with_hard_keyboard', keyboard_setting))

report.update(complete=True, status='passed')
write_report(report_path, report)
