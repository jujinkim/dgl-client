#!/usr/bin/env python3
"""Explicit Android resource matrix. Default: three sequential, real 30-minute workloads.

The probe drives the product WebView terminal through the SSHJ reference engine,
so its figures are not measurements of the Rust connection.

No raw terminal content, keys, or historical application objects are retained.
Interruptions produce failed evidence and unwind only resources owned by this run.
"""
import argparse
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import re
import shlex
import signal
import socket
import subprocess
import sys
import time

from evidence import ROOT, context, write_report
from resource_results import evaluate, parse_meminfo, renderer_pids, PROFILES

PACKAGE = 'kim.jujin.dglc'
ENVIRONMENTS = ['android-34', 'android-35', 'android-36']


def run(args, **kwargs):
    kwargs.setdefault('timeout', 30)
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, text=True, encoding='utf-8', **kwargs).stdout.strip()


def stop(process, group=False):
    if process is not None and process.poll() is None:
        if group:
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            if group:
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
            process.wait(timeout=10)


def root_owned_adb(adb):
    # adbd can close the transport while successfully restarting as root. Its
    # exit status alone is insufficient; require a reconnected, actual root UID.
    try:
        run(adb + ['root'])
    except subprocess.CalledProcessError:
        pass
    run(adb + ['wait-for-device'])
    if run(adb + ['shell', 'id', '-u']) != '0':
        raise RuntimeError('Owned AVD adbd did not become root')


@contextmanager
def emulator(api, output):
    sdk = Path(os.environ['ANDROID_HOME'])
    binary = sdk / 'emulator/emulator'
    adb = [sdk / 'platform-tools/adb', '-s', 'emulator-5580']
    name = 'Dgl_API' + api
    if name not in run([binary, '-list-avds']).splitlines():
        raise RuntimeError('Missing dedicated AVD ' + name)
    for port in (5580, 5581):
        with socket.socket() as probe:
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind(('127.0.0.1', port))
    # Emulator lock protects against an already running copy on another port.
    with (output / 'emulator.log').open('w') as log:
        process = subprocess.Popen([str(binary), '-avd', name, '-port', '5580', '-no-window', '-no-audio',
                                    '-no-boot-anim', '-no-snapshot', '-gpu', 'software'], stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError('Owned emulator exited')
                result = subprocess.run([str(x) for x in adb] + ['shell', 'getprop', 'sys.boot_completed'], capture_output=True, text=True, timeout=10)
                if result.returncode == 0 and result.stdout.strip() == '1':
                    break
                time.sleep(2)
            else:
                raise TimeoutError('Emulator boot timeout')
            if run(adb + ['emu', 'avd', 'name']).splitlines()[0] != name:
                raise RuntimeError('Wrong AVD identity')
            root_owned_adb(adb)
            run(adb + ['shell', 'input', 'keyevent', 'KEYCODE_WAKEUP'])
            run(adb + ['shell', 'wm', 'dismiss-keyguard'])
            yield adb
        finally:
            stop(process)


def android_sockets(adb, pid, port):
    # One directory listing, not hundreds of readlink subprocesses per 2-second tick.
    text = run(adb + ['shell', f'ls -l /proc/{pid}/fd; cat /proc/{pid}/net/tcp /proc/{pid}/net/tcp6'])
    inodes = set(re.findall(r'socket:\[(\d+)\]', text))
    return sum(1 for line in text.splitlines() if len(parts := line.split()) >= 10 and parts[3] == '01'
               and parts[9] in inodes and int(parts[2].split(':')[-1], 16) == port)


def android_memory(adb, state):
    pid = int(state['appPid'])
    result = parse_meminfo(run(adb + ['shell', 'dumpsys', 'meminfo', str(pid)]))
    result['detailRssBytes'] = result.pop('rssBytes')
    result['memoryDetail'] = True
    events = run(adb + ['logcat', '-b', 'events', '-d', '-s', 'am_anr:I', 'am_crash:I'])
    result['anrCount'] = sum('am_anr' in line and PACKAGE in line for line in events.splitlines())
    result['crashCount'] = sum('am_crash' in line and PACKAGE in line for line in events.splitlines())
    services = run(adb + ['shell', 'dumpsys', 'activity', 'services'])
    pids = renderer_pids(services, PACKAGE)
    result['rendererPids'] = pids
    details = [parse_meminfo(run(adb + ['shell', 'dumpsys', 'meminfo', str(p)])) for p in pids]
    for metric in ('pssBytes', 'rssBytes'):
        result['renderer' + metric[0].upper() + metric[1:]] = (sum(row[metric] for row in details)
            if details and all(row[metric] is not None for row in details) else None)
    return result


def sample_process(state, adb, port, detail):
    pid = int(state['appPid'])
    # Detect process replacement; never silently switch the measured host.
    actual = run(adb + ['shell', 'pidof', PACKAGE])
    if actual != str(pid):
        raise RuntimeError('Application PID changed')
    state['sshSockets'] = android_sockets(adb, pid, port)
    if detail:
        state.update(android_memory(adb, state))
    return state


def sample_android_memory(read_state, adb, origin):
    detail = android_memory(adb, origin)
    ending = json.loads(read_state())
    active = ('Ready', 'Restoring')
    stable = (ending['phase'] == origin['phase'] and origin.get('modelPhase') in active
              and ending.get('modelPhase') in active)
    detail.update(memoryPhase=origin['phase'] if stable else 'transition',
                  memoryPhaseElapsed=origin['phaseElapsed'],
                  memoryStartedElapsed=origin['elapsed'], memoryFinishedElapsed=ending['elapsed'],
                  memoryStartedModelPhase=origin.get('modelPhase'), memoryFinishedModelPhase=ending.get('modelPhase'))
    return detail


def collect(process, read_state, adb, port, profile, output):
    samples = []
    final = {}
    last_elapsed = -1
    next_tick = time.monotonic()
    next_detail = next_tick
    detail_future = None
    detail_origin = None
    deadline = next_tick + PROFILES[profile]['warmup'] + 3 * PROFILES[profile]['phase'] + PROFILES[profile]['observe'] + 240
    with ThreadPoolExecutor(max_workers=1, thread_name_prefix='resource-meminfo') as details, (output / 'samples.jsonl').open('w') as stream:
        while time.monotonic() < deadline:
            value = read_state()
            if value:
                final = json.loads(value)
                if final.get('complete'):
                    break
                if final['elapsed'] > last_elapsed:
                    last_elapsed = final['elapsed']
                    if final['phase'] != 'setup' and process.poll() is None:
                        try:
                            state = sample_process(dict(final), adb, port, False)
                        except (subprocess.CalledProcessError, ValueError):
                            # The app may exit between reading its last state and /proc.
                            # Accept only an explicit completed state plus successful exit.
                            latest = read_state()
                            if latest and json.loads(latest).get('complete'):
                                final = json.loads(latest)
                                break
                            raise
                        if detail_future is not None and detail_future.done():
                            detail = detail_future.result()
                            if detail['memoryPhase'] != state['phase']:
                                detail['memoryPhase'] = 'transition'
                            state.update(detail)
                            detail_future = None
                        if detail_future is None and time.monotonic() >= next_detail:
                            detail_origin = dict(final)
                            detail_future = details.submit(sample_android_memory, read_state, adb, detail_origin)
                            next_detail = max(next_detail + 10, time.monotonic())
                        samples.append(state)
                        stream.write(json.dumps(state) + '\n'); stream.flush()
            if process.poll() is not None:
                break
            next_tick += 2
            time.sleep(max(0, next_tick - time.monotonic()))
        else:
            raise TimeoutError('Resource scenario deadline')
        if detail_future is not None:
            try:
                detail_future.result()
            except (subprocess.SubprocessError, ValueError):
                # Only the final idle query may race a successfully completed app.
                if not (final.get('complete') and detail_origin['phase'] == 'observe'):
                    raise
    process.wait(timeout=30)
    return samples, final, process.returncode == 0


def worker(args):
    output = Path(args.output)
    fixture = Path(os.environ['DGL_TEST_ROOT'])
    port = int(os.environ['DGL_TEST_PORT'])
    config = dict(profile=args.profile, port=port, user=os.environ['DGL_TEST_USER'],
                  fingerprint=os.environ['DGL_TEST_FINGERPRINT'], key=Path(os.environ['DGL_TEST_KEY_FILE']).read_text(),
                  workloadCommand='exec ' + shlex.join([sys.executable, str(ROOT / 'scripts/resource_workload.py'), str(fixture)]),
                  progressCommand=shlex.join(['cat', str(fixture / 'progress.json')]),
                  pauseCommand=shlex.join(['touch', str(fixture / 'paused')]),
                  resumeCommand=shlex.join(['rm', '-f', str(fixture / 'paused')]))
    report = dict(schema=1, context=context(), environment=args.worker, profile=args.profile,
                  complete=False, status='failed', started=datetime.now(timezone.utc).isoformat(),
                  measurementSeconds=PROFILES[args.profile]['phase'] * 3)
    report['sourceDirty'] = bool(run(['git', 'status', '--porcelain']))
    report['tmux'] = os.environ['DGL_TEST_TMUX_VERSION']
    write_report(output / 'summary.json', report)
    process = None
    samples, final, completed = [], {}, False
    try:
        if report['tmux'] != '3.7c':
            raise RuntimeError('Wrong tmux version')
        if args.profile == 'soak' and report['sourceDirty']:
            raise RuntimeError('Source changed before measurement')
        with emulator(args.worker.split('-')[1], output) as adb:
            try:
                run(adb + ['install', '-r', ROOT / 'androidApp/build/outputs/apk/debug/androidApp-debug.apk'], timeout=120)
                run(adb + ['install', '-r', ROOT / 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk'], timeout=120)
                run(adb + ['reverse', f'tcp:{port}', f'tcp:{port}'])
                run(adb + ['shell', 'run-as', PACKAGE, 'sh', '-c', '"mkdir -p cache; rm -f cache/resource-state.json; cat > cache/resource-config.json"'], input=json.dumps(config))
                cmd = [str(x) for x in adb] + ['shell', 'am', 'instrument', '-w', PACKAGE + '.test/kim.jujin.dglc.ResourceInstrumentation']
                def read_state():
                    result = subprocess.run([str(x) for x in adb] + ['shell', '-T', 'run-as', PACKAGE, 'cat', 'cache/resource-state.json'], capture_output=True, text=True, timeout=10)
                    return result.stdout if result.returncode == 0 else ''
                report['system'] = {key: run(adb + ['shell', 'getprop', prop]) for key, prop in
                                    [('api', 'ro.build.version.sdk'), ('image', 'ro.build.fingerprint'), ('abi', 'ro.product.cpu.abi')]}
                if report['system']['api'] != args.worker.split('-')[1]:
                    raise RuntimeError('Wrong Android API')
                webview = run(adb + ['shell', 'dumpsys', 'webviewupdate'])
                report['system']['webview'] = next((line.strip() for line in webview.splitlines() if 'Current WebView package' in line), None)
                with (output / 'probe.log').open('w') as log:
                    process = subprocess.Popen(cmd, stdout=log, stderr=log, start_new_session=True)
                    samples, final, completed = collect(process, read_state, adb, port, args.profile, output)
                completed = completed and 'resource_result=passed' in (output / 'probe.log').read_text()
            finally:
                stop(process, group=True)
                try:
                    diagnostic = run(adb + ['logcat', '-d', '-s', 'ResourceProbe:E'])
                    (output / 'probe-diagnostic.log').write_text(diagnostic)
                except (subprocess.SubprocessError, OSError):
                    report['diagnosticUnavailable'] = True
                # This app/port is on the dedicated emulator started above.
                for command in [['shell', 'am', 'force-stop', PACKAGE],
                                ['shell', 'run-as', PACKAGE, 'rm', '-f', 'cache/resource-config.json', 'cache/resource-state.json', 'cache/resource-state.json.tmp'],
                                ['reverse', '--remove', f'tcp:{port}']]:
                    try:
                        run(adb + command)
                    except (subprocess.SubprocessError, OSError):
                        report['cleanupFailed'] = True
    except BaseException as error:
        report['errorType'] = type(error).__name__
        # Keep partial samples available for a failed/interrupted verdict.
        path = output / 'samples.jsonl'
        if path.exists():
            samples = [json.loads(line) for line in path.read_text().splitlines()]
        if samples:
            final = samples[-1]
        raise
    finally:
        verdict = evaluate(samples, final, args.profile, completed)
        report.update(verdict, complete=completed, finished=datetime.now(timezone.utc).isoformat(), final=final, sampleCount=len(samples))
        if args.profile == 'soak' and (run(['git', 'rev-parse', 'HEAD']) != report['context']['commit'] or run(['git', 'status', '--porcelain'])):
            report.update(status='failed', qualifiesAsSoak=False)
            report['errors'].append('source_changed_during_measurement')
        if report.get('cleanupFailed'):
            report.update(status='failed', qualifiesAsSoak=False)
        write_report(output / 'summary.json', report)
        print(args.worker + ': ' + report['status'] + ' ' + ', '.join(report['errors']), flush=True)
    return 0 if report['status'] in ('passed', 'smoke_passed') else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', choices=['smoke', 'soak'], default='soak')
    parser.add_argument('--environments', nargs='+', choices=ENVIRONMENTS)
    parser.add_argument('--worker', choices=ENVIRONMENTS, help=argparse.SUPPRESS)
    parser.add_argument('--output', help=argparse.SUPPRESS)
    args = parser.parse_args()
    os.environ.setdefault('ANDROID_HOME', str(Path.home() / 'Library/Android/sdk'))
    os.environ.setdefault('JAVA_HOME', '/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home')
    if args.worker:
        return worker(args)
    if platform.system() != 'Darwin':
        raise SystemExit('This resource matrix targets dedicated local Android AVDs on macOS')
    if args.profile == 'soak' and run(['git', 'status', '--porcelain']):
        raise SystemExit('Commit the verified code before soak measurement')
    environments = args.environments or ENVIRONMENTS
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    output = ROOT / 'artifacts' / ('resources-' + args.profile + '-' + stamp)
    output.mkdir(parents=True)
    report = dict(schema=1, context=context(), profile=args.profile, complete=False, status='failed', results=[], environments=environments)
    write_report(output / 'matrix.json', report)
    print('Resource evidence: ' + str(output), flush=True)
    with (output / 'build.log').open('w') as log:
        subprocess.run(['./gradlew', '-PdglResourceProbe=true', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'],
                       cwd=ROOT, check=True, stdout=log, stderr=log, timeout=600)
    process = None
    try:
        for environment in environments:
            folder = output / environment
            folder.mkdir()
            command = [sys.executable, 'scripts/integration.py', '--tmux', '.tools/tmux-3.7c/tmux-3.7c/tmux',
                       '--command', sys.executable, 'scripts/verify_resources.py', '--worker', environment,
                       '--profile', args.profile, '--output', str(folder)]
            print('Starting ' + environment, flush=True)
            process = subprocess.Popen(command, cwd=ROOT)
            process.wait()
            summary = folder / 'summary.json'
            if summary.exists(): report['results'].append(json.loads(summary.read_text()))
            write_report(output / 'matrix.json', report)
            if process.returncode:
                return 1
        report['complete'] = True
        report['status'] = 'passed' if args.profile == 'soak' else 'smoke_passed'
        report['qualifiesAsSoak'] = args.profile == 'soak' and environments == ENVIRONMENTS
        return 0
    finally:
        stop(process)
        write_report(output / 'matrix.json', report)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    sys.exit(main())
