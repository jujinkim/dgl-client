import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from migration_android_presentation import analyze


def fixture():
    # Fabricated unit-test rows, never collected baseline evidence.
    frames, markers = [], []

    def frame(token, ts):
        frames.extend([
            dict(id=token * 2, ts=ts, dur=2_000_000, surface_frame_token=token, display_frame_token=token + 1000,
                 layer_name='kim.jujin.dglc/kim.jujin.dglc.MainActivity#1',
                 process_name='kim.jujin.dglc', present_type='On-time Present'),
            dict(id=token * 2 + 1, ts=ts + 2_000_000, dur=5_000_000, surface_frame_token=0, display_frame_token=token + 1000,
                 layer_name='', process_name='/system/bin/surfaceflinger', present_type='On-time Present')])

    def marker(name, ts, token=0):
        markers.append(dict(name=name, ts=ts, dur=100, is_main_thread=1, do_frame=f'Choreographer#doFrame {token}'))

    for i in range(1, 9):
        ts = i * 100_000_000
        for offset, phase in enumerate(('INPUT', 'RENDER', 'VISUAL', 'DRAW')):
            marker(f'DGL_R1_{phase}_{i}', ts + offset * 1_000_000, i)
        frame(i, ts + 2_500_000)
    marker('DGL_R1_SCROLL_BEGIN', 1_000_000_000)
    marker('DGL_R1_SCROLL_END', 4_100_000_000)
    for i in range(31):
        frame(100 + i, 1_000_000_000 + i * 100_000_000)
    return dict(frames=frames, markers=markers, dataLoss=[])


class PresentationTests(unittest.TestCase):
    def test_uses_surfaceflinger_end_not_app_post_or_callback(self):
        report = analyze(fixture())
        first = report['localInputUpperBounds'][0]
        self.assertEqual(first['presentedNs'], 109_500_000)
        self.assertEqual(report['inputUpperBoundP95Ms'], 9.5)
        self.assertEqual(len(report['frames']), 30)
        self.assertTrue(all(a['presentedNs'] == b['previousPresentNs'] for a, b in zip(report['frames'], report['frames'][1:])))

    def test_ambiguous_or_missing_tokens_never_substitute_nearest_frame(self):
        for change in (
            lambda r: r['frames'].pop(1),
            lambda r: r['frames'].append(copy.deepcopy(r['frames'][0])),
            lambda r: r['frames'].append(copy.deepcopy(r['frames'][1])),
            lambda r: r['markers'][3].update(do_frame='missing'),
            lambda r: r['frames'][0].update(layer_name='unrelated-window'),
        ):
            raw = fixture(); change(raw)
            with self.assertRaises(ValueError):
                analyze(raw)

    def test_dropped_input_frame_cannot_be_latency_evidence(self):
        for index in (0, 1):
            raw = fixture(); raw['frames'][index]['present_type'] = 'Dropped Frame'
            with self.assertRaises(ValueError):
                analyze(raw)

    def test_scroll_drop_preserves_gap(self):
        raw = fixture()
        raw['frames'][16 + 10 * 2]['present_type'] = 'Dropped Frame'
        result = analyze(raw)
        self.assertEqual(result['droppedSurfaceFrames'], 1)
        self.assertEqual(len(result['frames']), 29)
        self.assertEqual(max(r['presentedNs'] - r['previousPresentNs'] for r in result['frames']), 200_000_000)

    def test_loss_duplicate_order_and_clock_errors_fail_closed(self):
        for change in (
            lambda r: r['dataLoss'].append(dict(name='lost', value=1)),
            lambda r: r['markers'].append(copy.deepcopy(r['markers'][0])),
            lambda r: r['markers'][0].update(is_main_thread=0),
            lambda r: r['markers'][1].update(ts=0),
            lambda r: r['frames'][1].update(dur=-1),
            lambda r: r['frames'][1].update(ts=1),
        ):
            raw = fixture(); change(raw)
            with self.assertRaises(ValueError):
                analyze(raw)

    def test_missing_marker_is_incomplete(self):
        raw = fixture(); raw['markers'].pop(0)
        with self.assertRaises(KeyError):
            analyze(raw)

    def test_unrelated_zero_display_tokens_are_not_used_as_presentations(self):
        raw = fixture()
        raw['frames'] += [dict(raw['frames'][1], id=999 + i, display_frame_token=0) for i in range(2)]
        self.assertEqual(len(analyze(raw)['localInputUpperBounds']), 8)
        raw['frames'][0]['display_frame_token'] = 0
        with self.assertRaises(ValueError):
            analyze(raw)


if __name__ == '__main__':
    unittest.main()
