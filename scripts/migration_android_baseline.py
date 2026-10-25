#!/usr/bin/env python3
"""Collect connected idle baseline on the owned API36 AVD, with real SSH/xterm.

Partial R1 evidence only: this does not measure input-to-presentation or startup.
Runs inside integration.py's private SSH fixture and never uses a personal device.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time

from evidence import ROOT, context, sha256, write_report
from migration_performance import CONTRACT, RULES, idle_metrics
from resource_results import renderer_pids
from verify_resources import emulator, run, stop

PACKAGE = 'kim.jujin.dglc'
SOURCES = ['scripts/migration_android_baseline.py', 'scripts/migration_performance.py',
           'scripts/verify_resources.py', 'scripts/integration.py', 'scripts/resource_workload.py',
           'app/src/resourceProbe/kotlin/kim/jujin/rovessh/ResourceScenario.kt',
           'androidApp/src/androidTest/kotlin/kim/jujin/rovessh/ResourceInstrumentation.kt']


def proc_stat(value, page_size, ticks, role):
    pid, _, tail = value.partition(' (')
    _, separator, fields = tail.rpartition(') ')
    parts = fields.split()
    if not separator or len(parts) < 22:
        raise ValueError('Truncated proc stat')
    return dict(pid=int(pid), identity=parts[19], role=role,
                cpuSeconds=(int(parts[11]) + int(parts[12])) / ticks,
                rssBytes=int(parts[21]) * page_size)


def worker(output):
    fixture = Path(os.environ['DGL_TEST_ROOT'])
    port = int(os.environ['DGL_TEST_PORT'])
    config = dict(profile='migration-idle', port=port, user=os.environ['DGL_TEST_USER'],
                  fingerprint=os.environ['DGL_TEST_FINGERPRINT'], key=Path(os.environ['DGL_TEST_KEY_FILE']).read_text(),
                  workloadCommand='exec ' + shlex.join([sys.executable, str(ROOT / 'scripts/resource_workload.py'), str(fixture)]),
                  progressCommand=shlex.join(['cat', str(fixture / 'progress.json')]),
                  pauseCommand=shlex.join(['touch', str(fixture / 'paused')]),
                  resumeCommand=shlex.join(['rm', '-f', str(fixture / 'paused')]))
    report = dict(schema=1, context=context(), contractSha256=sha256(CONTRACT), complete=False,
                  status='incomplete', scope='android-api36-avd', profile='migration-idle',
                  missingMetrics=['localInputP95Ms', 'frameP95Ms'], qualifiesAsPerformanceAcceptance=False,
                  idle=dict(connections=1, terminals=1, outputBytes=0, stabilizeSeconds=30,
                            completeProcessInventory=True, gpuMemory=dict(bytes=None,
                                unavailableReason='Software AVD renderer; per-app GPU allocation unavailable'), samples=[]))
    report['sourceSha256'] = {name: sha256(ROOT / name) for name in SOURCES}
    report['sourceDirty'] = bool(run(['git', 'status', '--porcelain']))
    path = output / 'summary.json'; write_report(path, report)
    process = None
    with emulator('36', output) as adb:
        try:
            apk = ROOT / 'androidApp/build/outputs/apk/debug/androidApp-debug.apk'
            run(adb + ['install', '-r', apk], timeout=120)
            run(adb + ['install', '-r', ROOT / 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk'], timeout=120)
            run(adb + ['reverse', f'tcp:{port}', f'tcp:{port}'])
            run(adb + ['shell', 'run-as', PACKAGE, 'sh', '-c', '"mkdir -p cache; rm -f cache/resource-state.json; cat > cache/resource-config.json"'], input=json.dumps(config))
            report['apkSha256'] = sha256(apk)
            report['instrumentationApkSha256'] = sha256(ROOT / 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk')
            report['downloadBytes'] = apk.stat().st_size
            report['environment'] = {key: run(adb + ['shell', 'getprop', prop]) for key, prop in
                                     [('api', 'ro.build.version.sdk'), ('image', 'ro.build.fingerprint'), ('abi', 'ro.product.cpu.abi')]}
            if report['environment']['api'] != '36':
                raise ValueError('Wrong API')
            report['environment'].update(avd='Dgl_API36', renderer='software', buildMode='debug',
                memoryMetric=RULES['memoryMetric']['android'],
                webview=next((line.strip() for line in run(adb + ['shell', 'dumpsys', 'webviewupdate']).splitlines() if 'Current WebView package' in line), None),
                display=run(adb + ['shell', 'wm', 'size']), density=run(adb + ['shell', 'wm', 'density']))
            ticks = int(run(adb + ['shell', 'getconf', 'CLK_TCK']))
            page_size = int(run(adb + ['shell', 'getconf', 'PAGESIZE']))
            report['environment'].update(clockTicksPerSecond=ticks, pageSizeBytes=page_size)
            with (output / 'probe.log').open('w') as log:
                process = subprocess.Popen([str(x) for x in adb] + ['shell', 'am', 'instrument', '-w', PACKAGE + '.test/kim.jujin.rovessh.ResourceInstrumentation'], stdout=log, stderr=log)
                deadline = time.monotonic() + 180
                origin = None; pid = None; initial_output = None; final = {}
                next_tick = time.monotonic()
                while time.monotonic() < deadline:
                    state = subprocess.run([str(x) for x in adb] + ['shell', 'run-as', PACKAGE, 'cat', 'cache/resource-state.json'], capture_output=True, text=True, timeout=10)
                    if state.returncode == 0:
                        final = json.loads(state.stdout)
                        if final.get('complete'):
                            break
                        if final['phase'] == 'idle':
                            if final['modelPhase'] != 'Ready' or final['controlClients'] != 1:
                                raise ValueError('Wrong connected idle state')
                            current_pid = int(final['appPid'])
                            if pid is not None and current_pid != pid:
                                raise ValueError('App process changed')
                            pid = current_pid
                            if initial_output is None:
                                initial_output = final['outputBytes']
                            if final['outputBytes'] != initial_output:
                                raise ValueError('Idle output changed')
                            renderers = renderer_pids(run(adb + ['shell', 'dumpsys', 'activity', 'services']), PACKAGE)
                            if not renderers:
                                raise ValueError('WebView process not found')
                            now = time.monotonic()
                            if origin is None:
                                origin = now
                            processes = [proc_stat(run(adb + ['shell', 'cat', f'/proc/{p}/stat']), page_size, ticks,
                                         'app' if p == pid else 'webview') for p in [pid] + renderers]
                            report['idle']['samples'].append(dict(elapsedSeconds=now - origin, processes=processes))
                            write_report(path, report)
                    if process.poll() is not None:
                        break
                    next_tick += 1
                    time.sleep(max(0, next_tick - time.monotonic()))
                process.wait(timeout=15)
            if not final.get('complete') or final.get('failure') or process.returncode != 0 or 'resource_result=passed' not in (output / 'probe.log').read_text():
                raise ValueError('Incomplete baseline scenario')
            if final['controlClients'] != 0 or final['outputBytes'] != initial_output:
                raise ValueError('Scenario output/cleanup mismatch')
            report['metrics'] = idle_metrics(report['idle'])
            report.update(complete=True, status='collected', final=final)
        except BaseException as error:
            report['errorType'] = type(error).__name__
            raise
        finally:
            stop(process)
            for command in [['shell', 'am', 'force-stop', PACKAGE],
                            ['shell', 'run-as', PACKAGE, 'rm', '-f', 'cache/resource-config.json', 'cache/resource-state.json', 'cache/resource-state.json.tmp'],
                            ['reverse', '--remove', f'tcp:{port}']]:
                try:
                    run(adb + command)
                except (OSError, subprocess.SubprocessError):
                    report.update(complete=False, status='incomplete', cleanupFailed=True)
            if report['sourceSha256'] != {name: sha256(ROOT / name) for name in SOURCES}:
                report.update(complete=False, status='incomplete', sourceChanged=True)
            write_report(path, report)
    if not report['complete']:
        raise ValueError('Incomplete cleanup')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.worker:
        if args.output is None:
            parser.error('--worker requires --output')
        try:
            worker(args.output)
        except BaseException as error:
            path = args.output / 'summary.json'
            report = json.loads(path.read_text()) if path.exists() else dict(schema=1)
            report.update(complete=False, status='incomplete', errorType=type(error).__name__)
            write_report(path, report)
            raise
        return
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    output = args.output or ROOT / 'artifacts/migration-r1' / ('android-idle-' + stamp)
    output.mkdir(parents=True, exist_ok=False)
    print('Evidence: ' + str(output), flush=True)
    try:
        subprocess.run(['./gradlew', '-PdglResourceProbe=true', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'], check=True, timeout=600)
        subprocess.run([sys.executable, 'scripts/integration.py', '--tmux', '.tools/tmux-3.7c/tmux-3.7c/tmux',
                        '--command', sys.executable, __file__, '--worker', '--output', str(output)], check=True)
    finally:
        # Restore the normal test runner even if a probe fails; do not leave the
        # ordinary APK outputs configured for the resource instrumentation.
        subprocess.run(['./gradlew', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'], check=True, timeout=600)


if __name__ == '__main__':
    main()
