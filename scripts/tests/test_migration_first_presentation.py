"""Fabricated trace rows test rejection rules, never performance evidence."""
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from migration_android_presentation import analyze, query, trace_config, TRACE_CONFIG
from verify_resources import root_owned_adb
import subprocess


def fixture():
    r = dict(markers=[], frames=[], dataLoss=[], chromiumSlices=[], chromiumArgs=[],
             flows=[], clockErrors=[], metadata=[dict(name='trace_time_clock_id', int_value=6)],
             clocks=[dict(id=i, ts=1, clock_value=1, clock_name=name, snapshot_id=0)
                     for i, name in enumerate(['MONOTONIC', 'BOOTTIME'])])

    def native(name, ts, token=None):
        r['markers'].append(dict(id=len(r['markers'])+10000, name=name, ts=ts, dur=100,
                                 is_main_thread=1, do_frame=f'Choreographer#doFrame {token}' if token else None))

    def event(ident, name, ts, parent=None, fields=None):
        r['chromiumSlices'].append(dict(id=ident, name=name, ts=ts, dur=1, parent_id=parent, track_id=1,
            pid=123, process_name='com.google.android.webview:sandboxed_process0', arg_set_id=ident))
        for k, v in (fields or {}).items():
            r['chromiumArgs'].append(dict(arg_set_id=ident, key=k, display_value=v))

    native('DGL_R1_CLOCK_1000000_1000000_1000001', 1000100)
    for i in range(1,18):
        ts = i * 1_000_000_000
        native(f'DGL_R1_INPUT_{i}', ts)
        base=i*100
        event(base, f'DGL_R1_JS_KEY_{i}', ts+1_000_000)
        if i == 17:
            event(base+1, 'DGL_R1_JS_NO_CHANGE_17', ts+201_000_000)
            native('DGL_R1_NO_CHANGE_17', ts+202_000_000)
            continue
        lag = 100_000_000 if i>8 else 0
        for n, phase in enumerate(['WRITE','CONTENT','RENDER_CONTENT'], 1):
            event(base+n, f'DGL_R1_JS_{phase}_{i}', ts+lag+(n+1)*1_000_000, base+10 if n==3 else None)
        frame_fields={'main_frame_pipeline.main_frame_id': str(i), 'main_frame_pipeline.step':'BEGIN_MAIN_FRAME'}
        event(base+10, 'ProxyMain::BeginMainFrame', ts+lag+3_500_000, fields=frame_fields)
        event(base+11, 'MainFrame.Draw', ts+lag+5_000_000, fields=dict(frame_fields, **{'main_frame_pipeline.step':'DRAW'}))
        event(base+12, 'Graphics.Pipeline', ts+lag+5_100_000, base+11,
              {'chrome_graphics_pipeline.step':'STEP_SUBMIT_COMPOSITOR_FRAME', 'chrome_graphics_pipeline.surface_frame_trace_id':'-1'})
        for n, phase in enumerate(['RENDER','VISUAL','DRAW'], 6):
            native(f'DGL_R1_{phase}_{i}', ts+lag+n*1_000_000, i)
        r['frames'].extend([
            dict(id=base+30, ts=ts+lag+7_000_000, dur=3_000_000, surface_frame_token=i, display_frame_token=i+1000,
                 layer_name='kim.jujin.rovessh/kim.jujin.rovessh.MainActivity#1', process_name='kim.jujin.rovessh', present_type='On-time Present'),
            dict(id=base+31, ts=ts+lag+9_000_000, dur=5_000_000, surface_frame_token=0, display_frame_token=i+1000,
                 layer_name=None, process_name='/system/bin/surfaceflinger', present_type='On-time Present')])
    native('DGL_R1_CLOCK_18000000000_18000000000_18000000001', 18000000100)
    return r


def analyze_first(raw):
    return analyze(raw, 'first-presentation')


class FirstPresentationTests(unittest.TestCase):
    def test_partial_chain_is_not_first_presentation(self):
        result = analyze_first(fixture())
        self.assertEqual(result['counts'], {'unresolved':16, 'no-content-change':1})
        first = result['inputs'][0]
        self.assertEqual(first['evidence']['chromiumMainFrameId'], '1')
        self.assertEqual(first['evidence']['submitEvents'][0]['sliceId'], 112)
        self.assertEqual(first['drawableUpperBound']['presentedNs'], 1_014_000_000)
        self.assertIsNone(first['firstPresentationNs'])
        self.assertIn('missing-valid-chromium-surface-frame-trace-id', first['issues'])
        self.assertFalse(result['qualifiesAsPerformanceAcceptance'])

    def test_unrelated_render_does_not_get_content_frame(self):
        raw=fixture()
        raw['chromiumSlices']=[r for r in raw['chromiumSlices'] if r['name'] != 'DGL_R1_JS_RENDER_CONTENT_1']
        result=analyze_first(raw)['inputs'][0]
        self.assertNotIn('chromiumMainFrameId', result['evidence'])
        self.assertIsNone(result['firstPresentationNs'])

    def test_no_change_has_no_latency_or_surface(self):
        row=analyze_first(fixture())['inputs'][-1]
        self.assertEqual(row['status'], 'no-content-change')
        self.assertNotIn('drawableUpperBound',row)
        self.assertNotIn('keyToWriteMs',row)

    def test_no_change_contradiction_is_unresolved(self):
        raw=fixture(); raw['chromiumSlices'].append(dict(raw['chromiumSlices'][-1],id=99999,name='DGL_R1_JS_CONTENT_17'))
        row=analyze_first(raw)['inputs'][-1]
        self.assertEqual(row['status'],'unresolved')
        self.assertIn('no-change-contradicted',row['issues'])

    def test_nearby_frames_do_not_replace_missing_or_duplicate_tokens(self):
        for edit in (
            lambda r:r['frames'].pop(1),
            lambda r:r['frames'].append(copy.deepcopy(r['frames'][0])),
            lambda r:r['frames'].append(copy.deepcopy(r['frames'][1])),
            lambda r:r['frames'][0].update(display_frame_token=0),
        ):
            raw=fixture();edit(raw)
            row=analyze_first(raw)['inputs'][0]
            self.assertNotIn('drawableUpperBound',row)
            self.assertIsNone(row['firstPresentationNs'])

    def test_dropped_surface_or_display_is_not_presentation(self):
        for index in (0,1):
            raw=fixture();raw['frames'][index]['present_type']='Dropped Frame'
            row=analyze_first(raw)['inputs'][0]
            self.assertIn('dropped-or-incomplete-frame',row['issues'])
            self.assertNotIn('drawableUpperBound',row)

    def test_loss_or_clock_mismatch_invalidates_even_no_change(self):
        for edit in (
            lambda r:r['dataLoss'].append(dict(name='lost',value=1)),
            lambda r:r['clockErrors'].append(dict(name='clock',value=1)),
            lambda r:r['clocks'][0].update(clock_value=10_000_000),
            lambda r:r['metadata'].clear(),
            lambda r:r['markers'].pop(0),
        ):
            raw=fixture();edit(raw);result=analyze_first(raw)
            self.assertTrue(result['globalIssues'])
            self.assertTrue(all(x['status']=='unresolved' for x in result['inputs']))
            self.assertTrue(all('drawableUpperBound' not in x for x in result['inputs']))

    def test_duplicate_markers_and_bad_order_do_not_prove(self):
        for edit in (
            lambda r:r['markers'].append(copy.deepcopy(r['markers'][1])),
            lambda r:r['chromiumSlices'].append(dict(r['chromiumSlices'][0],id=99999)),
            lambda r:r['chromiumSlices'][0].update(ts=0),
            lambda r:r['markers'][1].update(is_main_thread=0),
        ):
            raw=fixture();edit(raw);row=analyze_first(raw)['inputs'][0]
            self.assertTrue(row['issues']);self.assertIsNone(row['firstPresentationNs'])
            self.assertNotIn('drawableUpperBound',row)

    def test_positive_submit_id_still_requires_bridge_and_earlier_content(self):
        raw=fixture()
        for a in raw['chromiumArgs']:
            if a['key']=='chrome_graphics_pipeline.surface_frame_trace_id':a['display_value']='42'
        row=analyze_first(raw)['inputs'][0]
        self.assertEqual(row['status'],'unresolved')
        self.assertIn('content-coverage-of-all-earlier-frames',row['missingEvidence'])

    def test_delay_experiment_uses_key_to_write_not_presentation(self):
        report=analyze_first(fixture())['delayExperiment']
        self.assertEqual(report['normal']['keyToWriteMs'],[1.0]*8)
        self.assertEqual(report['delayed-100ms']['keyToWriteMs'],[101.0]*8)

    def test_mode_defaults_and_explicit_category_filters(self):
        self.assertEqual(trace_config('upper-bound'), TRACE_CONFIG)
        config=trace_config('first-presentation')
        self.assertIn('name: "track_event"',config)
        self.assertIn('disabled_categories: "*"',config)
        self.assertIn('blink.user_timing',config)

    def test_adb_root_transport_restart_requires_verified_uid(self):
        with patch('verify_resources.run', side_effect=[subprocess.CalledProcessError(1,['adb','root']), '', '0']) as run:
            root_owned_adb(['adb','-s','owned'])
            self.assertEqual(run.call_args.args[0][-3:],['shell','id','-u'])

    def test_adb_root_actual_failure_is_not_swallowed(self):
        with patch('verify_resources.run', side_effect=['', '', '2000']):
            with self.assertRaises(RuntimeError):root_owned_adb(['adb','-s','owned'])

    def test_query_preserves_quotes_newlines_null_and_large_id(self):
        import json
        row=dict(id=9223372036854775806, value='"quoted"\nline', empty=None)
        encoded=json.dumps(row).encode().hex()
        with patch('migration_android_presentation.run',side_effect=['"id","value","empty"\n',f'"row_hex"\n"{encoded}"\n']):
            self.assertEqual(query('processor','trace','SELECT 1'),[row])


if __name__=='__main__':
    unittest.main()
