#!/usr/bin/env python3
"""Short API36 presentation-hook verification, not an R1 acceptance run.

Uses only an owned AVD/private SSH fixture. No terminal text or credentials are
written to evidence. Perfetto joins use display-frame end (SurfaceFlinger), never
app frame end (GPU/post). Input results are conservative drawable-frame bounds.
"""
import argparse
import csv
from datetime import datetime, timezone
import io
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import time

from evidence import ROOT, context, sha256, write_report
from migration_performance import CONTRACT, p95
from verify_resources import emulator, run, stop

PACKAGE = 'kim.jujin.rovessh'
SOURCES = [
    'scripts/migration_android_presentation.py',
    'scripts/migration_first_presentation.py',
    'scripts/verify_resources.py', 'scripts/integration.py', 'scripts/resource_workload.py',
    'scripts/evidence.py', 'scripts/migration_performance.py',
    'app/src/resourceProbe/kotlin/kim/jujin/rovessh/ResourceScenario.kt',
    'androidApp/src/androidTest/kotlin/kim/jujin/rovessh/MigrationPresentationProbe.kt',
    'androidApp/src/androidTest/kotlin/kim/jujin/rovessh/ResourceInstrumentation.kt',
]
TRACE_CONFIG = '''buffers { size_kb: 32768 fill_policy: DISCARD }
duration_ms: 90000
data_sources { config { name: "linux.ftrace" ftrace_config {
  atrace_categories: "gfx" atrace_categories: "view"
  atrace_apps: "kim.jujin.rovessh"
} } }
data_sources { config { name: "android.surfaceflinger.frametimeline" } }
data_sources { config { name: "linux.process_stats" process_stats_config { scan_all_processes_on_start: true } } }
'''


CHROMIUM_CATEGORIES = ['input', 'latencyInfo', 'blink', 'blink.user_timing', 'cc', 'viz',
                       'android_webview', 'benchmark', 'disabled-by-default-devtools.timeline',
                       'graphics.pipeline']
WEBVIEW_FLAGS = '_ --enable-features=EnablePerfettoSystemTracing\n'
FLAG_FILE = '/data/local/tmp/webview-command-line'


def trace_config(mode):
    if mode == 'upper-bound':
        return TRACE_CONFIG
    chrome = json.dumps(dict(record_mode='record-until-full',
                             included_categories=CHROMIUM_CATEGORIES, excluded_categories=['*']))
    # Both filters are explicit: Chromium 133 still consumes chrome_config.
    return TRACE_CONFIG + 'data_sources { config { name: "track_event"\n' + \
        ' chrome_config { trace_config: ' + json.dumps(chrome) + ' }\n' + \
        ' track_event_config { disabled_categories: "*" ' + ' '.join(
            'enabled_categories: ' + json.dumps(c) for c in CHROMIUM_CATEGORIES) + ' }\n} }\n'


def query(processor, trace, sql):
    # The shell's CSV printer does not escape embedded quotes/newlines in trace
    # args. Transport each JSON row as hex so IDs, nulls and strings survive exactly.
    columns = next(csv.reader(io.StringIO(run([processor, trace, '-Q',
        'SELECT * FROM (' + sql + ') LIMIT 0'], timeout=60))))
    pairs = ','.join("'" + c.replace("'", "''") + "',\"" + c.replace('"', '""') + '\"' for c in columns)
    encoded = run([processor, trace, '-Q',
        'SELECT hex(json_object(' + pairs + ')) AS row_hex FROM (' + sql + ')'], timeout=60)
    return [json.loads(bytes.fromhex(row['row_hex'])) for row in csv.DictReader(io.StringIO(encoded))]


def extract(processor, trace, mode="upper-bound"):
    # Keep both surface and display records: app ts+dur is NOT presentation.
    frames = query(processor, trace, '''SELECT f.id, f.ts, f.dur, coalesce(f.surface_frame_token,0) AS surface_frame_token,
        f.display_frame_token, f.layer_name, f.present_type, p.name AS process_name
        FROM actual_frame_timeline_slice f JOIN process p USING(upid)
        WHERE p.name IN ('kim.jujin.rovessh', '/system/bin/surfaceflinger') ORDER BY f.ts, f.id''')
    markers = query(processor, trace, '''SELECT s.id, s.name, s.ts, s.dur, t.is_main_thread,
        (SELECT a.name FROM ancestor_slice(s.id) a WHERE a.name GLOB 'Choreographer#doFrame*' LIMIT 1) AS do_frame
        FROM slice s JOIN thread_track tt ON s.track_id=tt.id JOIN thread t USING(utid)
        JOIN process p USING(upid) WHERE p.name='kim.jujin.rovessh' AND s.name GLOB 'DGL_R1_*'
        ORDER BY s.ts, s.id''')
    losses = query(processor, trace, "SELECT name,value FROM stats WHERE value>0 AND (severity='data_loss' OR name GLOB '*buffer*overrun*')")
    raw = dict(frames=frames, markers=markers, dataLoss=losses)
    if mode == 'first-presentation':
        from migration_first_presentation import extract_chromium
        raw.update(extract_chromium(lambda sql: query(processor, trace, sql)))
    return raw


def analyze(raw, mode="upper-bound"):
    if mode == 'first-presentation':
        from migration_first_presentation import analyze_first
        return analyze_first(raw)
    return analyze_upper_bound(raw)


def analyze_upper_bound(raw):
    """Fail closed on missing, dropped, ambiguous, or uncorrelated display evidence."""
    import re
    if raw['dataLoss']:
        raise ValueError('Perfetto data loss')
    markers = {}
    for row in raw['markers']:
        if row['name'] in markers or row['is_main_thread'] not in ('1', 1):
            raise ValueError('Duplicate/non-main-thread marker')
        markers[row['name']] = row
    displays = {}
    surfaces = []
    for row in raw['frames']:
        row = dict(row)
        for key in ('id', 'ts', 'dur', 'surface_frame_token', 'display_frame_token'):
            row[key] = int(row[key])
        if row['process_name'] == '/system/bin/surfaceflinger':
            token = row['display_frame_token']
            # Token 0 is used by unrelated frames with no correlation token.
            # Only an actually referenced positive token may be used below.
            if token > 0:
                displays.setdefault(token, []).append(row)
        elif '/kim.jujin.rovessh.MainActivity' in row['layer_name']:
            surfaces.append(row)
    if not surfaces or not displays:
        raise ValueError('Missing app/SurfaceFlinger frame timeline')

    def presented(surface):
        matches = displays.get(surface['display_frame_token'], [])
        if len(matches) != 1:
            raise ValueError('Ambiguous/missing display frame')
        display = matches[0]
        if any(r['dur'] <= 0 or r['present_type'] not in ('On-time Present', 'Late Present', 'Early Present') for r in (surface, display)):
            raise ValueError('Missing/dropped/incomplete display frame')
        end = display['ts'] + display['dur']
        if end <= surface['ts']:
            raise ValueError('Invalid presentation clock')
        return end

    inputs = []
    for ident in range(1, 9):
        rows = [markers[f'DGL_R1_{phase}_{ident}'] for phase in ('INPUT', 'RENDER', 'VISUAL', 'DRAW')]
        times = [int(row['ts']) for row in rows]
        if any(b <= a for a, b in zip(times, times[1:])):
            raise ValueError('Invalid input/render/visual/draw order')
        token = re.fullmatch(r'Choreographer#doFrame (\d+).*', rows[-1]['do_frame'])
        if token is None:
            raise ValueError('Draw lacks a Choreographer frame token')
        candidates = [f for f in surfaces if f['surface_frame_token'] == int(token[1])]
        if len(candidates) != 1:
            raise ValueError('Ambiguous/missing draw surface')
        end = presented(candidates[0])
        if end <= times[-1]:
            raise ValueError('Presentation precedes draw')
        inputs.append(dict(id=ident, inputNs=times[0], renderCallbackNs=times[1], visualReadyNs=times[2],
                           drawNs=times[3], presentedNs=end, surfaceToken=int(token[1]),
                           displayToken=candidates[0]['display_frame_token']))
    if sum(name.startswith('DGL_R1_INPUT_') for name in markers) != 8:
        raise ValueError('Unexpected input count')
    begin = int(markers['DGL_R1_SCROLL_BEGIN']['ts'])
    end = int(markers['DGL_R1_SCROLL_END']['ts'])
    if end - begin < 3_000_000_000:
        raise ValueError('Short scroll interval')
    # Count all app updates in the interval. Dropped updates have no presentation;
    # retain their count and preserve the elapsed gap between visible frames.
    motion = [f for f in surfaces if begin <= f['ts'] < end]
    dropped = sum(f['present_type'] == 'Dropped Frame' for f in motion)
    points = sorted({presented(f) for f in motion if f['present_type'] != 'Dropped Frame'})
    if len(points) < 30:
        raise ValueError('Insufficient scroll presentation evidence')
    intervals = [dict(id=i, previousPresentNs=a, presentedNs=b) for i, (a, b) in enumerate(zip(points, points[1:]))]
    return dict(localInputUpperBounds=inputs, frames=intervals, droppedSurfaceFrames=dropped,
                inputUpperBoundP95Ms=p95([(r['presentedNs'] - r['inputNs']) / 1e6 for r in inputs]),
                scrollFrameP95Ms=p95([(r['presentedNs'] - r['previousPresentNs']) / 1e6 for r in intervals]))


def worker(output, processor, mode="upper-bound"):
    fixture = Path(os.environ['DGL_TEST_ROOT'])
    port = int(os.environ['DGL_TEST_PORT'])
    config = dict(profile='migration-presentation', presentationMode=mode, port=port, user=os.environ['DGL_TEST_USER'],
                  fingerprint=os.environ['DGL_TEST_FINGERPRINT'], key=Path(os.environ['DGL_TEST_KEY_FILE']).read_text(),
                  workloadCommand='exec ' + shlex.join([sys.executable, str(ROOT / 'scripts/resource_workload.py'), str(fixture)]),
                  progressCommand=shlex.join(['cat', str(fixture / 'progress.json')]),
                  pauseCommand=shlex.join(['touch', str(fixture / 'paused')]))
    report = dict(schema=1, context=context(), complete=False, status='incomplete',
                  scope='android-api36-avd', profile='presentation-hook-smoke', mode=mode,
                  qualifiesAsPerformanceAcceptance=False, contractSha256=sha256(CONTRACT),
                  clock='perfetto-trace-clock-nanoseconds',
                  inputEnd=('first-presentation-diagnostic' if mode == 'first-presentation' else
                            'first-proven-drawable-frame-presentation-upper-bound'),
                  missingMetrics=['exact-first-input-presentation', '200-input-samples',
                                  '3600-frame-samples', 'resize-and-tab-scenarios', 'tool-overhead'],
                  sourceSha256={name: sha256(ROOT / name) for name in SOURCES},
                  processorSha256=sha256(processor), processorVersion=run([processor, '--version']))
    summary = output / 'summary.json'
    write_report(summary, report)
    process = None
    trace_pid = None
    flags_touched = False
    flag_backup = None
    flag_existed = False
    import uuid
    remote_flag_backup = FLAG_FILE + '.dgl-r1-' + uuid.uuid4().hex
    remote = '/data/misc/perfetto-traces/dgl-r1-presentation.trace'
    try:
        with emulator('36', output) as adb:
            try:
                apk = ROOT / 'androidApp/build/outputs/apk/debug/androidApp-debug.apk'
                test_apk = ROOT / 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk'
                run(adb + ['install', '-r', apk], timeout=120)
                run(adb + ['install', '-r', test_apk], timeout=120)
                report.update(apkSha256=sha256(apk), instrumentationApkSha256=sha256(test_apk),
                    adbSha256=sha256(adb[0]), emulatorSha256=sha256(Path(os.environ['ANDROID_HOME']) / 'emulator/emulator'),
                    devicePerfettoSha256=run(adb + ['shell', 'sha256sum', '/system/bin/perfetto']).split()[0])
                report['environment'] = dict(avd='Dgl_API36', renderer='software', buildMode='debug',
                    image=run(adb + ['shell', 'getprop', 'ro.build.fingerprint']),
                    api=run(adb + ['shell', 'getprop', 'ro.build.version.sdk']),
                    display=run(adb + ['shell', 'wm', 'size']), density=run(adb + ['shell', 'wm', 'density']),
                    webview=next((s.strip() for s in run(adb + ['shell', 'dumpsys', 'webviewupdate']).splitlines() if 'Current WebView package' in s), None))
                if report['environment']['api'] != '36':
                    raise ValueError('Wrong Android API')
                if '133.0.6943.137' not in (report['environment']['webview'] or ''):
                    raise ValueError('WebView version differs from the pinned R1 environment')
                if mode == 'first-presentation':
                    # Only reached within the owned/identity-checked API36 context.
                    flag_existed = run(adb + ['shell', 'sh', '-c',
                        shlex.quote(f'if test -e {FLAG_FILE}; then echo yes; else echo no; fi')]) == 'yes'
                    flag_backup = output / 'webview-command-line.before'
                    if flag_existed:
                        run(adb + ['pull', FLAG_FILE, flag_backup])
                        report['originalWebviewFlagsSha256'] = sha256(flag_backup)
                    if flag_existed:
                        run(adb + ['shell', 'cp', '-p', FLAG_FILE, remote_flag_backup])
                    flags_touched = True
                    run(adb + ['shell', 'sh', '-c', shlex.quote(f'cat > {FLAG_FILE}')], input=WEBVIEW_FLAGS)
                    run(adb + ['shell', 'chmod', '644', FLAG_FILE])
                    if run(adb + ['shell', 'cat', FLAG_FILE]) != WEBVIEW_FLAGS.strip():
                        raise ValueError('WebView tracing flags not installed')
                    run(adb + ['shell', 'am', 'force-stop', PACKAGE])
                    report['webviewFlags'] = WEBVIEW_FLAGS.strip()
                run(adb + ['reverse', f'tcp:{port}', f'tcp:{port}'])
                run(adb + ['shell', 'run-as', PACKAGE, 'sh', '-c', '"mkdir -p cache; rm -f cache/resource-state.json; cat > cache/resource-config.json"'], input=json.dumps(config))
                active_config = trace_config(mode)
                (output / 'config.pbtxt').write_text(active_config)
                report['traceConfigSha256'] = sha256(output / 'config.pbtxt')
                trace_pid = int(run(adb + ['shell', 'perfetto', '--background-wait', '--txt', '-c', '-', '-o', remote], input=active_config).splitlines()[-1])
                with (output / 'probe.log').open('w') as log:
                    process = subprocess.Popen([str(a) for a in adb] + ['shell', 'am', 'instrument', '-w', PACKAGE + '.test/kim.jujin.rovessh.ResourceInstrumentation'], stdout=log, stderr=log)
                    process.wait(timeout=75)
                final = json.loads(run(adb + ['shell', 'run-as', PACKAGE, 'cat', 'cache/resource-state.json']))
                report['final'] = final
                if process.returncode != 0 or not final.get('complete') or final.get('failure') or final['controlClients'] != 0 or 'resource_result=passed' not in (output / 'probe.log').read_text():
                    raise ValueError('Presentation scenario failed')
            finally:
                stop(process)
                try:
                    if trace_pid is not None:
                        # Only signal the Perfetto child created for this run, on the owned AVD.
                        run(adb + ['shell', 'kill', '-INT', str(trace_pid)])
                        deadline = time.monotonic() + 10
                        while time.monotonic() < deadline:
                            exists = subprocess.run([str(a) for a in adb] + ['shell', 'kill', '-0', str(trace_pid)], capture_output=True, timeout=5)
                            if exists.returncode:
                                break
                            time.sleep(.2)
                        else:
                            raise TimeoutError('Perfetto did not flush')
                        run(adb + ['pull', remote, output / 'presentation.perfetto-trace'])
                except (OSError, subprocess.SubprocessError, TimeoutError):
                    report['traceFlushFailed'] = True
                try:
                    (output / 'diagnostic.log').write_text(run(adb + ['logcat', '-d', '-s', 'ResourceProbe:E']))
                except (OSError, subprocess.SubprocessError):
                    report['diagnosticUnavailable'] = True
                if flags_touched:
                    try:
                        run(adb + ['shell', 'am', 'force-stop', PACKAGE])
                        if flag_existed:
                            # cp -p backup preserves permissions/ownership on device (see setup below).
                            run(adb + ['shell', 'mv', remote_flag_backup, FLAG_FILE])
                            restored = output / 'webview-command-line.restored'
                            run(adb + ['pull', FLAG_FILE, restored])
                            if sha256(restored) != sha256(flag_backup):
                                raise ValueError('WebView flags restore mismatch')
                        else:
                            run(adb + ['shell', 'rm', '-f', FLAG_FILE])
                            if run(adb + ['shell', 'sh', '-c', shlex.quote(
                                    f'if test -e {FLAG_FILE}; then echo yes; else echo no; fi')]) != 'no':
                                raise ValueError('WebView flags removal failed')
                        report['webviewFlagsRestored'] = True
                    except (OSError, subprocess.SubprocessError, ValueError):
                        report['cleanupFailed'] = True
                for command in [['shell', 'am', 'force-stop', PACKAGE],
                                ['shell', 'run-as', PACKAGE, 'rm', '-f', 'cache/resource-config.json', 'cache/resource-state.json', 'cache/resource-state.json.tmp'],
                                ['reverse', '--remove', f'tcp:{port}'], ['shell', 'rm', '-f', remote]]:
                    try:
                        run(adb + command)
                    except (OSError, subprocess.SubprocessError):
                        report['cleanupFailed'] = True
        report['ownedAvdStopped'] = True
        raw = extract(processor, output / 'presentation.perfetto-trace', mode)
        write_report(output / 'extracted.json', raw)
        report['traceSha256'] = sha256(output / 'presentation.perfetto-trace')
        report['measurement'] = analyze(raw, mode)
        if report.get('cleanupFailed') or report.get('traceFlushFailed') or report['sourceSha256'] != {name: sha256(ROOT / name) for name in SOURCES}:
            raise ValueError('Cleanup/source integrity failed')
        if mode == 'first-presentation':
            invalid = bool(report['measurement']['globalIssues'])
            report.update(complete=not invalid, collectionComplete=True,
                          status='invalid_trace' if invalid else 'unresolved', firstPresentationProven=False)
        else:
            report.update(complete=True, status='hook_verified')
    except BaseException as error:
        report.update(complete=False, errorType=type(error).__name__)
        raise
    finally:
        write_report(summary, report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    mode.add_argument('--analyze', type=Path, help='Analyze an existing raw Perfetto trace without starting an AVD')
    parser.add_argument('--mode', choices=['upper-bound', 'first-presentation'], default='upper-bound')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--trace-processor', type=Path, required=True, help='Local Perfetto trace_processor_shell binary')
    args = parser.parse_args()
    processor = args.trace_processor.resolve()
    if not processor.is_file():
        parser.error('Trace processor binary missing')
    if args.analyze:
        if (args.output is None or args.output.exists() or args.output.with_suffix('.extracted.json').exists() or
                args.output.resolve() in (args.analyze.resolve(), processor) or
                args.output.with_suffix('.extracted.json').resolve() in (args.analyze.resolve(), processor)):
            parser.error('--analyze requires new, separate report and extracted JSON paths')
        report = dict(schema=1, complete=False, status='incomplete', qualifiesAsPerformanceAcceptance=False, mode=args.mode,
                      inputEnd=('first-presentation-diagnostic' if args.mode == 'first-presentation' else
                                'first-proven-drawable-frame-presentation-upper-bound'))
        write_report(args.output, report)
        try:
            report.update(traceSha256=sha256(args.analyze), analyzerSha256=sha256(Path(__file__)),
                          analyzerSourceSha256={name: sha256(ROOT / name) for name in SOURCES if name.startswith('scripts/')},
                          processorSha256=sha256(processor), processorVersion=run([processor, '--version']))
            raw = extract(processor, args.analyze, args.mode)
            write_report(args.output.with_suffix('.extracted.json'), raw)
            report['measurement'] = analyze(raw, args.mode)
            if args.mode == 'first-presentation':
                invalid = bool(report['measurement']['globalIssues'])
                report.update(complete=not invalid, status='invalid_trace' if invalid else 'unresolved',
                              firstPresentationProven=False)
            else:
                report.update(complete=True, status='trace_analyzed')
        finally:
            write_report(args.output, report)
        print(report['status'])
        return
    if args.worker:
        if args.output is None:
            parser.error('--worker requires --output')
        return worker(args.output, processor, args.mode)
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    output = args.output or ROOT / 'artifacts/migration-r1' / ('android-presentation-' + stamp)
    output.mkdir(parents=True, exist_ok=False)
    print('Evidence: ' + str(output), flush=True)
    fixture_process = None
    try:
        subprocess.run(['./gradlew', '-PdglResourceProbe=true', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'], check=True, timeout=600)
        fixture_process = subprocess.Popen([sys.executable, 'scripts/integration.py', '--tmux', '.tools/tmux-3.7c/tmux-3.7c/tmux',
                        '--command', sys.executable, __file__, '--worker', '--output', str(output), '--trace-processor', str(processor), '--mode', args.mode])
        if fixture_process.wait() != 0:
            raise RuntimeError('Presentation hook verification failed; see ' + str(output))
    finally:
        try:
            if fixture_process is not None and fixture_process.poll() is None:
                # integration.py forwards SIGTERM to the worker and cleans private SSH/tmux.
                fixture_process.terminate()
                fixture_process.wait(timeout=45)
        finally:
            subprocess.run(['./gradlew', ':androidApp:assembleDebug', ':androidApp:assembleDebugAndroidTest'], check=True, timeout=600)
            verify_restored_runner(output)


def verify_restored_runner(output):
    import xml.etree.ElementTree as ET
    path = output / 'summary.json'
    report = json.loads(path.read_text()) if path.exists() else dict(schema=1, complete=False,
        status='incomplete', qualifiesAsPerformanceAcceptance=False)
    try:
        sdk = Path(os.environ['ANDROID_HOME'])
        analyzer = sdk / 'cmdline-tools/latest/bin/apkanalyzer'
        apk = ROOT / 'androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk'
        manifest = ET.fromstring(run([analyzer, 'manifest', 'print', apk]))
        runners = [e.attrib.get('{http://schemas.android.com/apk/res/android}name')
                   for e in manifest.findall('instrumentation')]
        if runners != ['androidx.test.runner.AndroidJUnitRunner']:
            raise ValueError('Default instrumentation runner was not restored')
        report['restoredRunner'] = runners[0]
        report['restoredInstrumentationApkSha256'] = sha256(apk)
    except BaseException:
        report.update(complete=False, cleanupFailed=True)
        raise
    finally:
        write_report(path, report)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    main()
