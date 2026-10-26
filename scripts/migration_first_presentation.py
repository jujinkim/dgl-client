"""Version-pinned, fail-closed Chromium/WebView presentation diagnostics.

Chromium 133 exposes a main-frame pipeline but WebView's synchronous submission
uses surface_frame_trace_id=-1. That is not a join key. This adapter extracts
partial causal links; it cannot certify the first displayed frame. In particular,
a visual callback, a matching latency ID or a nearby draw is never such proof.
"""
from collections import Counter, defaultdict
import re

PACKAGE = 'kim.jujin.dglc'
PRESENT_TYPES = ('On-time Present', 'Late Present', 'Early Present')


def extract_chromium(query):
    return dict(
        chromiumSlices=query('''SELECT s.*, t.name AS thread_name, p.name AS process_name,
            p.pid, p.upid, t.tid FROM slice s
            LEFT JOIN thread_track tt ON tt.id=s.track_id LEFT JOIN thread t USING(utid)
            LEFT JOIN process p USING(upid)
            WHERE s.category IS NOT NULL OR s.name GLOB 'Choreographer#doFrame*'
               OR s.name GLOB 'DrawFrame*' ORDER BY s.ts,s.id'''),
        chromiumArgs=query('''SELECT a.* FROM args a WHERE a.arg_set_id IN
            (SELECT arg_set_id FROM slice WHERE category IS NOT NULL)'''),
        flows=query('SELECT * FROM flow'),
        clocks=query('SELECT * FROM clock_snapshot ORDER BY ts,id'),
        clockErrors=query("SELECT * FROM stats WHERE value>0 AND (severity='error' OR name GLOB '*clock*error*')"),
        metadata=query("SELECT * FROM metadata WHERE name GLOB '*clock*' OR name GLOB '*trace_config*'"))


def key(value):
    return str(value)


def clock_evidence(raw):
    """Perfetto normalizes Chromium MONOTONIC and ftrace to trace BOOTTIME.

    Native paired readings independently cross-check the offset and bracket the
    input experiment. JS performance.now() values are never used as timestamps.
    The 1ms tolerance covers paired reads; the 5ms bound covers the trace call.
    """
    issues = []
    clocks = raw.get('clocks', [])
    mono = [r for r in clocks if r['clock_name'] == 'MONOTONIC']
    boot = [r for r in clocks if r['clock_name'] == 'BOOTTIME']
    ids = [r.get('int_value') for r in raw.get('metadata', []) if r['name'] == 'trace_time_clock_id']
    if ids != [6] or not mono or not boot:
        issues.append('missing-clock-conversion')
    offsets = [int(r['ts']) - int(r['clock_value']) for r in mono]
    if offsets and max(offsets) - min(offsets) > 1_000_000:
        issues.append('unstable-monotonic-offset')
    if any(int(r['ts']) != int(r['clock_value']) for r in boot):
        issues.append('boottime-trace-clock-mismatch')
    checks = []
    for row in raw['markers']:
        match = re.fullmatch(r'DGL_R1_CLOCK_(\d+)_(\d+)_(\d+)', row['name'])
        if match:
            before, elapsed, after = map(int, match.groups())
            valid = (row['is_main_thread'] in (1, '1') and before <= after and
                     after - before <= 1_000_000 and offsets and
                     before + min(offsets) - 1_000_000 <= elapsed <= after + max(offsets) + 1_000_000 and
                     0 <= int(row['ts']) - elapsed <= 5_000_000)
            checks.append(dict(sliceId=row['id'], traceNs=int(row['ts']), monotonicBeforeNs=before,
                               bootNs=elapsed, monotonicAfterNs=after, valid=bool(valid)))
    inputs = [int(r['ts']) for r in raw['markers'] if r['name'].startswith('DGL_R1_INPUT_')]
    if (len(checks) != 2 or not all(r['valid'] for r in checks) or not inputs or
            not checks[0]['traceNs'] < min(inputs) <= max(inputs) < checks[-1]['traceNs']):
        issues.append('native-clock-cross-check-failed')
    if raw.get('clockErrors'):
        issues.append('trace-parser-errors')
    return dict(valid=not issues, issues=issues, traceClock='BOOTTIME', chromiumClock='MONOTONIC',
                conversion='Perfetto clock_snapshot; no JS wall-clock conversion',
                snapshotIds=sorted({r['snapshot_id'] for r in mono + boot}),
                monotonicOffsetsNs=sorted(set(offsets)), nativeChecks=checks)


def drawable_link(ident, native, frames):
    """Independent conservative upper bound, never a first-presentation edge."""
    rows = []
    for phase in ('INPUT', 'RENDER', 'VISUAL', 'DRAW'):
        matches = native.get(f'DGL_R1_{phase}_{ident}', [])
        if len(matches) != 1 or matches[0]['is_main_thread'] not in (1, '1'):
            return None, 'missing-or-duplicate-native-' + phase.lower()
        rows.append(matches[0])
    times = [int(r['ts']) for r in rows]
    if any(b <= a for a, b in zip(times, times[1:])):
        return None, 'native-phase-order'
    token = re.fullmatch(r'Choreographer#doFrame (\d+).*', rows[-1].get('do_frame') or '')
    if not token or int(token[1]) <= 0:
        return None, 'missing-draw-surface-token'
    surfaces = [r for r in frames if r['process_name'] == PACKAGE and
                '/kim.jujin.dglc.MainActivity' in (r.get('layer_name') or '') and
                int(r['surface_frame_token']) == int(token[1])]
    if len(surfaces) != 1:
        return None, 'ambiguous-or-missing-app-surface'
    surface = surfaces[0]
    displays = [r for r in frames if r['process_name'] == '/system/bin/surfaceflinger' and
                int(r['display_frame_token']) == int(surface['display_frame_token']) > 0]
    if len(displays) != 1:
        return None, 'ambiguous-or-missing-display-token'
    display = displays[0]
    if any(int(r['dur']) <= 0 or r['present_type'] not in PRESENT_TYPES for r in (surface, display)):
        return None, 'dropped-or-incomplete-frame'
    end = int(display['ts']) + int(display['dur'])
    if end <= max(times[-1], int(surface['ts'])):
        return None, 'display-clock-order'
    return dict(nativeSliceIds=[r['id'] for r in rows], surfaceSliceId=surface['id'],
                displaySliceId=display['id'], surfaceToken=int(token[1]),
                displayToken=int(surface['display_frame_token']), presentedNs=end,
                meaning='drawable-frame-upper-bound-only'), None


def analyze_first(raw):
    native, marks, argsets = defaultdict(list), defaultdict(list), defaultdict(dict)
    for row in raw['markers']:
        native[row['name']].append(row)
    slices = {key(r['id']): r for r in raw.get('chromiumSlices', [])}
    for row in slices.values():
        if row['name'].startswith('DGL_R1_JS_'):
            marks[row['name']].append(row)
    duplicate_args = set()
    for row in raw.get('chromiumArgs', []):
        args = argsets[key(row['arg_set_id'])]
        if row['key'] in args:
            duplicate_args.add(key(row['arg_set_id']))
        args[row['key']] = row['display_value']

    def args(row):
        return argsets[key(row['arg_set_id'])]

    def ancestors(row):
        result, seen = [], set()
        while key(row.get('parent_id')) in slices:
            parent = key(row['parent_id'])
            if parent in seen:
                break
            seen.add(parent)
            row = slices[parent]
            result.append(row)
        return result

    def evidence(row):
        return dict(sliceId=row['id'], name=row['name'], ts=int(row['ts']),
                    pid=row.get('pid'), trackId=row.get('track_id'), args=args(row))

    clock = clock_evidence(raw)
    global_issues = list(clock['issues'])
    if raw['dataLoss']:
        global_issues.append('perfetto-data-loss')
    if len(slices) != len(raw.get('chromiumSlices', [])):
        global_issues.append('duplicate-chromium-slice-id')
    if duplicate_args:
        global_issues.append('duplicate-chromium-argument')
    expected = {f'DGL_R1_INPUT_{i}' for i in range(1, 18)}
    actual = {name for name in native if name.startswith('DGL_R1_INPUT_')}
    if expected != actual:
        global_issues.append('unexpected-native-input-inventory')
    for name, group in list(native.items()) + list(marks.items()):
        if len(group) != 1:
            global_issues.append('duplicate-marker:' + name)
    # Index actual producer main-frame IDs, scoped by renderer process.
    pipelines = defaultdict(list)
    submissions = []
    for row in slices.values():
        fields = args(row)
        if 'main_frame_pipeline.main_frame_id' in fields:
            pipelines[(key(row.get('pid')), fields['main_frame_pipeline.main_frame_id'])].append(row)
        if fields.get('chrome_graphics_pipeline.step') == 'STEP_SUBMIT_COMPOSITOR_FRAME':
            submissions.append(row)
    inputs = []
    for ident in range(1, 18):
        result = dict(id=ident, scenario='normal' if ident <= 8 else 'delayed-100ms' if ident <= 16 else 'no-change',
                      status='unresolved', firstPresentationNs=None, evidence={}, missingEvidence=[])
        issues = list(global_issues)
        phases = ('KEY', 'NO_CHANGE') if ident == 17 else ('KEY', 'WRITE', 'CONTENT', 'RENDER_CONTENT')
        js = []
        source = native.get(f'DGL_R1_INPUT_{ident}', [])
        if len(source) != 1 or source[0]['is_main_thread'] not in (1, '1'):
            issues.append('missing-or-invalid-native-input')
        else:
            result['evidence']['nativeInputSliceId'] = source[0]['id']
            result['inputNs'] = int(source[0]['ts'])
        for phase in phases:
            group = marks.get(f'DGL_R1_JS_{phase}_{ident}', [])
            if len(group) != 1:
                issues.append('missing-or-duplicate-js-' + phase.lower())
            else:
                js.append(group[0])
                result['evidence'][phase.lower()] = evidence(group[0])
        if len(js) == len(phases) and len(source) == 1:
            times = [int(source[0]['ts'])] + [int(r['ts']) for r in js]
            if any(b < a for a, b in zip(times, times[1:])):
                issues.append('input-content-clock-order')
            if len({key(r.get('pid')) for r in js}) != 1 or any(
                    not (r.get('process_name') or '').startswith('com.google.android.webview:sandboxed_process') for r in js):
                issues.append('renderer-identity-mismatch')
        if marks.get(f'DGL_R1_JS_UNEXPECTED_CHANGE_{ident}'):
            issues.append('unexpected-content-change')
        if ident == 17:
            no_change = native.get('DGL_R1_NO_CHANGE_17', [])
            if (len(no_change) != 1 or no_change[0]['is_main_thread'] not in (1, '1') or
                    not js or int(no_change[0]['ts']) < int(js[-1]['ts'])):
                issues.append('missing-native-no-change-confirmation')
            # No-write arm + unchanged cells/cursor + no content-bearing render.
            if any(marks.get(f'DGL_R1_JS_{p}_17') for p in ('WRITE', 'CONTENT', 'RENDER_CONTENT')) or any(
                    native.get(f'DGL_R1_{p}_17') for p in ('RENDER', 'VISUAL', 'DRAW')):
                issues.append('no-change-contradicted')
            if not issues:
                result['status'] = 'no-content-change'
                result['evidence']['nativeNoChangeSliceId'] = no_change[0]['id']
        else:
            link, error = drawable_link(ident, native, raw['frames'])
            if error:
                issues.append(error)
            else:
                result['evidence']['drawableLink'] = link
                if not issues:
                    result['drawableUpperBound'] = link
            if len(js) == len(phases):
                result['keyToWriteMs'] = (int(js[1]['ts']) - int(js[0]['ts'])) / 1e6
                result['inputToContentMs'] = (int(js[2]['ts']) - int(source[0]['ts'])) / 1e6 if len(source) == 1 else None
                render = js[-1]
                chain = ancestors(render)
                main = [r for r in chain if r['name'] == 'ProxyMain::BeginMainFrame' and
                        args(r).get('main_frame_pipeline.step') == 'BEGIN_MAIN_FRAME']
                if len(main) == 1 and args(main[0]).get('main_frame_pipeline.main_frame_id'):
                    frame_id = args(main[0])['main_frame_pipeline.main_frame_id']
                    pipeline = pipelines[(key(main[0].get('pid')), frame_id)]
                    result['evidence']['chromiumMainFrameId'] = frame_id
                    result['evidence']['renderAncestorSliceIds'] = [r['id'] for r in chain]
                    result['evidence']['mainFrameEvents'] = [evidence(r) for r in pipeline]
                    draw_ids = {key(r['id']) for r in pipeline if args(r).get('main_frame_pipeline.step') == 'DRAW'}
                    submits = [r for r in submissions if any(key(a['id']) in draw_ids for a in ancestors(r))]
                    result['evidence']['submitEvents'] = [evidence(r) for r in submits]
                    token_values = {args(r).get('chrome_graphics_pipeline.surface_frame_trace_id') for r in submits}
                    if not submits or any(t is None or int(t) <= 0 for t in token_values):
                        issues.append('missing-valid-chromium-surface-frame-trace-id')
                    selected = {key(r['id']) for r in pipeline + submits + chain + js}
                    result['evidence']['flows'] = [f for f in raw.get('flows', []) if
                        key(f['slice_out']) in selected or key(f['slice_in']) in selected]
                else:
                    issues.append('missing-content-render-main-frame-ancestry')
            # Even a unique submit/latency ID cannot prove that all earlier frames
            # lacked these pixels. Chromium 133 provides neither a content epoch
            # on every compositor frame nor a compositor -> Android token bridge.
            result['missingEvidence'] = ['chromium-submission-to-app-surface-token',
                                         'content-coverage-of-all-earlier-frames']
        result['issues'] = sorted(set(issues))
        inputs.append(result)
    groups = {}
    for name in ('normal', 'delayed-100ms'):
        samples = [r['keyToWriteMs'] for r in inputs if r['scenario'] == name and 'keyToWriteMs' in r]
        groups[name] = dict(samples=len(samples), keyToWriteMs=samples)
    boundary = []
    for row in slices.values():
        fields = args(row)
        if fields.get('chrome_graphics_pipeline.step') == 'STEP_RECEIVE_COMPOSITOR_FRAME':
            boundary.append(evidence(row))
    return dict(inputs=inputs, counts=dict(Counter(r['status'] for r in inputs)),
                chromiumBoundary=dict(receiveEvents=boundary,
                    surfaceTraceIdCounts=dict(Counter(r['args'].get('chrome_graphics_pipeline.surface_frame_trace_id')
                                                       for r in boundary))),
                clock=clock, dataLoss=raw['dataLoss'], parserErrors=raw.get('clockErrors', []),
                globalIssues=sorted(set(global_issues)), delayExperiment=groups,
                proofCapability='partial-links-only-chromium-133',
                qualifiesAsPerformanceAcceptance=False)
