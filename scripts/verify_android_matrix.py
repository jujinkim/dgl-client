#!/usr/bin/env python3
"""Run existing dedicated Dgl AVDs; never attach to or stop a user's emulator."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from evidence import context, require_passed

ROOT = Path(__file__).resolve().parents[1]
SDK = Path(os.environ['ANDROID_HOME'])
ADB = str(SDK / 'platform-tools/adb')
EMULATOR = str(SDK / 'emulator/emulator')
PORT = 5580
SERIAL = f'emulator-{PORT}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--apis', nargs='+', choices=['34', '35', '36'], default=['34', '35', '36'])
    args = parser.parse_args()
    avds = subprocess.check_output([EMULATOR, '-list-avds'], text=True, encoding="utf-8").splitlines()
    required = ['Dgl_API' + api for api in args.apis]
    if not set(required) <= set(avds):
        raise SystemExit('Missing dedicated test AVDs: ' + str(set(required) - set(avds)))
    results = []
    suffix = '' if args.apis == ['34', '35', '36'] else '-' + '-'.join(args.apis)
    report = ROOT / ('artifacts/android-matrix' + suffix + '.json')
    report.parent.mkdir(parents=True, exist_ok=True)
    (ROOT / '.tools').mkdir(exist_ok=True)
    ctx = context()
    report.write_text(json.dumps({'schema': 1, 'context': ctx, 'complete': False, 'status': 'failed', 'results': []}) + '\n', encoding="utf-8")
    for avd in required:
        for port in (PORT, PORT + 1):
            with socket.socket() as probe:
                probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                probe.bind(('127.0.0.1', port))
        with (ROOT / '.tools' / f'p403-{avd}.log').open('w') as log:
            process = subprocess.Popen([EMULATOR, '-avd', avd, '-port', str(PORT), '-no-window', '-no-audio',
                                        '-no-boot-anim', '-no-snapshot', '-gpu', 'software'], stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError('Test emulator exited: ' + avd)
                    boot = subprocess.run([ADB, '-s', SERIAL, 'shell', 'getprop', 'sys.boot_completed'], capture_output=True, text=True, encoding="utf-8")
                    if boot.returncode == 0 and boot.stdout.strip() == '1':
                        break
                    time.sleep(2)
                else:
                    raise RuntimeError('Test emulator boot timeout')
                name = subprocess.check_output([ADB, '-s', SERIAL, 'emu', 'avd', 'name'], text=True, encoding="utf-8").splitlines()[0]
                if name != avd:
                    raise RuntimeError('Unexpected AVD identity')
                subprocess.run([ADB, '-s', SERIAL, 'shell', 'input', 'keyevent', 'KEYCODE_WAKEUP'], check=True)
                subprocess.run([ADB, '-s', SERIAL, 'shell', 'wm', 'dismiss-keyguard'], check=True)
                env = dict(os.environ, ANDROID_SERIAL=SERIAL)
                result = subprocess.run(['python3', 'scripts/integration.py', '--tmux', '.tools/tmux-3.7c/tmux-3.7c/tmux',
                                         '--command', 'python3', 'scripts/android_integration.py'], cwd=ROOT, env=env)
                if result.returncode:
                    raise SystemExit(result.returncode)
                tests = json.loads((ROOT / ('artifacts/android-tests-' + avd[-2:] + '.json')).read_text(encoding='utf-8'))
                require_passed(tests, ctx)
                if tests.get('passed') != 4 or tests.get('skipped') != 0 or tests.get('api') != avd[-2:]:
                    raise ValueError('Incomplete Android tests')
                results.append({'avd': avd, 'api': avd[-2:], 'tmux': '3.7c', 'passed': 4, 'failed': 0, 'skipped': 0})
                report.write_text(json.dumps({'schema': 1, 'context': ctx, 'complete': False, 'status': 'failed', 'results': results}, indent=2) + '\n', encoding="utf-8")
            finally:
                # Only terminate the process created by this script.
                process.terminate()
                try:
                    process.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
    report.write_text(json.dumps({'schema': 1, 'context': ctx, 'complete': True, 'status': 'passed', 'results': results}, indent=2) + '\n', encoding="utf-8")


if __name__ == '__main__':
    main()
