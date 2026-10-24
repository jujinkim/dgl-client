import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from migration_performance import (CONTRACT, RULES, cpu_seconds, durations, evaluate,
                                   idle_metrics, main, package_sizes, p95, sha256)
from migration_android_baseline import proc_stat


def trace(android=False):
    # Unit-test data only, never written as collected acceptance evidence.
    def events(count, start, end, ms):
        return [dict(id=i, **{start: i * 2_000_000_000, end: i * 2_000_000_000 + int(ms * 1e6)}) for i in range(count)]
    rows = [dict(elapsedSeconds=i, processes=[dict(pid=1, identity='start-1', role='app', cpuSeconds=i * .005, rssBytes=50 * 1024**2)] +
                 ([dict(pid=2, identity='start-2', role='webview', cpuSeconds=i * .001, rssBytes=30 * 1024**2)] if android else [])) for i in range(61)]
    return dict(schema=1, scope=RULES['scope'][int(android)], contractSha256=sha256(CONTRACT),
                sourceCommit='a' * 40, binarySha256='b' * 64, synthetic=False,
                clock='monotonic-nanoseconds', inputEnd='presented',
                environment=dict(device='test-fixture', os='test-os', cpu='test-cpu', gpu='test-gpu', ramBytes=16 * 1024**3,
                                 displayHz=60, displayScale=2, powerMode='ac', scenario='one-connection-one-terminal-v1',
                                 buildMode='release', memoryMetric=RULES['memoryMetric']['android' if android else 'macos'],
                                 webviewVersion='fixture-version' if android else None, renderer='fixture-renderer'),
                localInput=events(200, 'inputNs', 'presentedNs', 20),
                frames=[dict(id=i, previousPresentNs=i * 16_000_000, presentedNs=(i + 1) * 16_000_000) for i in range(3600)],
                startup={k: events(30, 'launchNs', 'inputReadyNs', 900) for k in ('cold', 'warm')},
                idle=dict(connections=1, terminals=1, outputBytes=0, stabilizeSeconds=30,
                          completeProcessInventory=True, gpuMemory=dict(bytes=None, unavailableReason='test fixture'), samples=rows),
                package=dict(downloadBytes=40 * 1024**2, installedBytes=140 * 1024**2))


class MigrationPerformanceTests(unittest.TestCase):
    def test_nearest_rank_is_not_interpolated(self):
        self.assertEqual(p95(list(range(1, 21))), 19)
        self.assertEqual(p95([1]), 1)

    def test_nonfinite_and_negative_samples_rejected(self):
        for value in (float('nan'), float('inf'), -1, True):
            with self.assertRaises(ValueError):
                p95([value])

    def test_missing_repeated_reversed_and_short_traces_cannot_pass(self):
        for mutate in (lambda t: t.pop('frames'), lambda t: t['localInput'].pop(),
                       lambda t: t['localInput'][1].update(id=0),
                       lambda t: t['localInput'][1].update(inputNs=0),
                       lambda t: t['localInput'][0].update(presentedNs=0),
                       lambda t: t['startup']['cold'].pop()):
            t = trace(); mutate(t)
            self.assertEqual(evaluate(t)['status'], 'incomplete')

    def test_exact_threshold_and_over_budget(self):
        t = trace()
        for sample in t['localInput']:
            sample['presentedNs'] = sample['inputNs'] + 33_000_000
        self.assertEqual(evaluate(t)['status'], 'passed')
        for sample in t['localInput']:
            sample['presentedNs'] += 1
        self.assertEqual(evaluate(t)['status'], 'failed')

    def test_idle_uses_one_core_and_all_processes(self):
        result = idle_metrics(trace(True)['idle'])
        self.assertAlmostEqual(result['idleCpuOneCorePercent'], .6)
        self.assertEqual(result['idleRssBytes'], 80 * 1024**2)

    def test_idle_missing_inventory_counter_reset_and_pid_replacement(self):
        for mutate in (lambda t: t['idle'].update(completeProcessInventory=False),
                       lambda t: t['idle']['samples'][20]['processes'][0].update(cpuSeconds=0),
                       lambda t: t['idle']['samples'][20]['processes'][0].update(identity='restarted'),
                       lambda t: t['idle'].update(outputBytes=1),
                       lambda t: t['idle'].update(stabilizeSeconds=29),
                       lambda t: t['idle']['samples'].pop(30),
                       lambda t: t['idle']['samples'][0]['processes'].append(dict(t['idle']['samples'][0]['processes'][0], role='child')),
                       lambda t: t['idle'].update(gpuMemory={})):
            t = trace(); mutate(t)
            self.assertEqual(evaluate(t)['status'], 'incomplete')

    def test_android_requires_same_measured_baseline(self):
        t = trace(True)
        self.assertEqual(evaluate(t)['status'], 'incomplete')
        baseline = copy.deepcopy(t)
        self.assertEqual(evaluate(t, baseline)['status'], 'passed')
        for key in ('device', 'renderer', 'buildMode', 'webviewVersion', 'displayScale', 'memoryMetric'):
            wrong = copy.deepcopy(baseline); wrong['environment'][key] = 'different'
            self.assertEqual(evaluate(t, wrong)['status'], 'incomplete')

    def test_android_10_percent_and_webview_memory(self):
        baseline = trace(True); t = copy.deepcopy(baseline)
        for sample in t['localInput']:
            sample['presentedNs'] = sample['inputNs'] + 22_000_000
        self.assertEqual(evaluate(t, baseline)['status'], 'passed')
        for sample in t['localInput']:
            sample['presentedNs'] += 1
        self.assertIn('regression:localInputP95Ms', evaluate(t, baseline)['errors'])
        t = copy.deepcopy(baseline)
        for row in t['idle']['samples']:
            row['processes'][1]['rssBytes'] += 10 * 1024**2
        self.assertIn('regression:idleRssBytes', evaluate(t, baseline)['errors'])
        for row in t['idle']['samples']:
            row['processes'].pop()
        self.assertEqual(evaluate(t, baseline)['status'], 'incomplete')

    def test_debug_wrong_contract_ssh_callbacks_and_synthetic_are_not_display_evidence(self):
        for field, value in [('contractSha256', 'a' * 64), ('synthetic', True), ('inputEnd', 'ssh-callback'),
                             ('binarySha256', ''), ('scope', 'windows')]:
            t = trace(); t[field] = value
            self.assertEqual(evaluate(t)['status'], 'incomplete')
        t = trace(); t['environment']['buildMode'] = 'debug'
        self.assertEqual(evaluate(t)['status'], 'incomplete')

    def test_package_does_not_follow_external_symlinks_or_double_count_hardlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); installed = root / 'app'; installed.mkdir()
            (root / 'package').write_bytes(b'x' * 10)
            (root / 'external').write_bytes(b'secret' * 1000)
            (installed / 'file').write_bytes(b'x' * 20)
            os.link(installed / 'file', installed / 'hardlink')
            link = installed / 'link'; link.symlink_to(root / 'external')
            result = package_sizes(root / 'package', installed)
            self.assertEqual(result['downloadBytes'], 10)
            self.assertEqual(result['installedBytes'], 20 + link.lstat().st_size)

    def test_macos_cpu_formats(self):
        self.assertEqual(cpu_seconds('00:01.25'), 1.25)
        self.assertEqual(cpu_seconds('01:02:03'), 3723)
        self.assertEqual(cpu_seconds('1-01:02:03'), 90123)

    def test_android_proc_cpu_rss_and_start_identity(self):
        # comm may contain spaces and closing parentheses; fields start after the last one.
        fields = ['S'] + ['0'] * 21
        fields[11], fields[12], fields[19], fields[21] = '123', '77', '999', '10'
        sample = proc_stat('42 (webview (test)) ' + ' '.join(fields), 4096, 100, 'webview')
        self.assertEqual(sample, dict(pid=42, identity='999', role='webview', cpuSeconds=2, rssBytes=40960))
        with self.assertRaises(ValueError):
            proc_stat('42 (truncated) S 0', 4096, 100, 'webview')

    def test_cli_preserves_inputs_and_invalidates_stale_success(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / 'trace.json'; output = Path(temp) / 'report.json'
            source.write_text('not-json')
            output.write_text('{"status":"passed","complete":true}')
            with patch('sys.argv', ['measure', 'evaluate', str(source), '--output', str(output)]):
                with self.assertRaises(json.JSONDecodeError):
                    main()
            self.assertEqual(json.loads(output.read_text())['status'], 'incomplete')
            with patch('sys.argv', ['measure', 'evaluate', str(source), '--output', str(source)]), patch('sys.stderr'):
                with self.assertRaises(SystemExit):
                    main()
            self.assertEqual(source.read_text(), 'not-json')


if __name__ == '__main__':
    unittest.main()
