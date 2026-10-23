from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from resource_results import evaluate, parse_meminfo, renderer_pids, PROFILES, MIB


def evidence(profile='soak', android=False):
    spec = PROFILES[profile]
    rows = []
    elapsed = 0
    for phase, seconds in [('warmup', spec['warmup']), ('baseline', 20), ('output', spec['phase']),
                           ('recovery', spec['phase']), ('recreate', spec['phase']), ('observe', spec['observe'])]:
        for t in range(0, seconds, 2):
            elapsed += 2
            row = dict(elapsed=elapsed, phase=phase, phaseElapsed=t, appPid='123', remotePid=456,
                       rssBytes=100*MIB, heapBytes=40*MIB, fd=30, threads=20, gcCount=3, gcMillis=100,
                       liveTerminals=0 if phase in ('baseline', 'observe') else 1, scrollback=10000,
                       renderSequence=elapsed, queuedBytes=4096, sshSockets=1, modelPhase='Ready')
            if android and t % 10 == 0:
                row.update(pssBytes=60*MIB, rendererRssBytes=50*MIB, rendererPssBytes=40*MIB,
                           rendererPids=[789], memoryDetail=True, anrCount=0, crashCount=0)
            rows.append(row)
    final = dict(rows[-1], complete=True, recoveries=spec['repeats'], recreations=spec['repeats'],
                 blockedInputs=spec['repeats'], checks=43, controlClients=0, slowConsumerChecked=True,
                 outputBytesPerSecond=65000)
    return rows, final


class ResourceResultsTest(unittest.TestCase):
    def test_normal_soak(self):
        rows, final = evidence()
        result = evaluate(rows, final, 'soak', 'desktop', True)
        self.assertEqual(result['errors'], [])
        self.assertTrue(result['qualifiesAsSoak'])

    def test_android_renderer_separate(self):
        rows, final = evidence(android=True)
        result = evaluate(rows, final, 'soak', 'android', True)
        self.assertEqual(result['errors'], [])
        self.assertEqual(result['phases']['output']['rendererPssBytes']['sampleMax'], 40*MIB)

    def test_accumulating_memory(self):
        rows, final = evidence()
        for row in rows:
            if row['phase'] == 'recovery': row['rssBytes'] += row['phaseElapsed'] * MIB
        self.assertIn('recovery_growth_rssBytes', evaluate(rows, final, 'soak', 'desktop', True)['errors'])

    def test_fd_and_thread_growth(self):
        rows, final = evidence()
        for row in rows:
            if row['phase'] == 'observe': row.update(fd=35, threads=25)
        errors = evaluate(rows, final, 'soak', 'desktop', True)['errors']
        self.assertIn('idle_growth_fd', errors)
        self.assertIn('idle_growth_threads', errors)

    def test_missing_is_not_zero(self):
        rows, final = evidence()
        del rows[10]['rssBytes']
        self.assertIn('missing_rssBytes', evaluate(rows, final, 'soak', 'desktop', True)['errors'])
        self.assertEqual(parse_meminfo('permission denied'), dict(pssBytes=None, rssBytes=None))

    def test_interrupted_cannot_pass(self):
        rows, final = evidence()
        self.assertIn('incomplete_or_interrupted', evaluate(rows, final, 'soak', 'desktop', False)['errors'])

    def test_pid_replacement(self):
        for field, reason in [('appPid', 'app_pid_changed_or_missing'), ('remotePid', 'remote_pid_changed_or_missing')]:
            rows, final = evidence()
            rows[10][field] = 999
            self.assertIn(reason, evaluate(rows, final, 'soak', 'desktop', True)['errors'])

    def test_renderer_pid_replacement_tracks_sum(self):
        rows, final = evidence(android=True)
        for row in rows:
            if row.get('memoryDetail') and row['phase'] == 'recreate': row['rendererPids'] = [790]
        self.assertEqual(evaluate(rows, final, 'soak', 'android', True)['errors'], [])

    def test_missing_renderer_fails(self):
        rows, final = evidence(android=True)
        for row in rows:
            if row.get('memoryDetail') and row['phase'] == 'recovery': row['rendererPids'] = []
        self.assertIn('missing_android_memory', evaluate(rows, final, 'soak', 'android', True)['errors'])

    def test_smoke_is_never_soak(self):
        rows, final = evidence('smoke')
        self.assertFalse(evaluate(rows, final, 'smoke', 'desktop', True)['qualifiesAsSoak'])
        self.assertFalse(evaluate(rows, final, 'soak', 'desktop', True)['qualifiesAsSoak'])

    def test_short_or_gapped_collection(self):
        rows, final = evidence()
        self.assertIn('sample_gap', evaluate(rows[:20] + rows[30:], final, 'soak', 'desktop', True)['errors'])
        final['recoveries'] = 19
        self.assertIn('recovery_count', evaluate(rows, final, 'soak', 'desktop', True)['errors'])

    def test_meminfo_and_service_linkage(self):
        self.assertEqual(parse_meminfo('TOTAL PSS: 123 TOTAL RSS: 456 TOTAL SWAP PSS: 0'), dict(pssBytes=123*1024, rssBytes=456*1024))
        services = '''* ServiceRecord{a com.webview/SandboxedProcessService0}
 app=ProcessRecord{abc 1234:com.webview:sandboxed_process0/u0i1}
 binding=ProcessRecord{def 5678:kim.jujin.rovessh/u0a1}
* ServiceRecord{b com.webview/SandboxedProcessService1}
 app=ProcessRecord{def 9999:com.webview:sandboxed_process1/u0i2}
 binding=ProcessRecord{gh 5555:other.app/u0a2}'''
        self.assertEqual(renderer_pids(services, 'kim.jujin.rovessh'), [1234])


class ResourceCollectorTest(unittest.TestCase):
    def test_closed_renderer_at_phase_boundary_is_not_a_stall(self):
        rows, final = evidence(android=True)
        ending = [s for s in rows if s['phase'] == 'recreate'][-1]
        ending.update(modelPhase='Disconnected', renderSequence=0)
        self.assertEqual(evaluate(rows, final, 'soak', 'android', True)['errors'], [])

    def test_cross_phase_renderer_sample_is_not_misattributed(self):
        rows, final = evidence(android=True)
        ending = [s for s in rows if s['phase'] == 'recovery' and s.get('memoryDetail')][-1]
        ending.update(memoryPhase='transition', rendererPids=[999])
        self.assertEqual(evaluate(rows, final, 'soak', 'android', True)['errors'], [])

    def test_negative_missing_metric_fails(self):
        rows, final = evidence()
        rows[20]['fd'] = -1
        self.assertIn('missing_fd', evaluate(rows, final, 'soak', 'desktop', True)['errors'])

    def test_host_pid_replacement_is_rejected(self):
        from unittest.mock import patch
        from verify_resources import sample_process
        with patch('verify_resources.run', return_value='999'):
            with self.assertRaisesRegex(RuntimeError, 'PID changed'):
                sample_process(dict(appPid='123'), ['adb'], 2222, False)

    def test_completion_does_not_query_dead_process(self):
        import json
        import tempfile
        from unittest.mock import Mock, patch
        from verify_resources import collect
        process = Mock(returncode=0)
        final = dict(complete=True, elapsed=120, phase='observe')
        with tempfile.TemporaryDirectory() as directory, patch('verify_resources.sample_process') as sample:
            rows, result, completed = collect(process, lambda: json.dumps(final), None, 2222, 'smoke', Path(directory))
            self.assertTrue(completed)
            self.assertEqual(result, final)
            self.assertEqual(rows, [])
            sample.assert_not_called()

    def test_exit_between_state_and_os_sample(self):
        import json
        import subprocess
        import tempfile
        from unittest.mock import Mock, patch
        from verify_resources import collect
        process = Mock(returncode=0)
        process.poll.return_value = None
        states = iter([json.dumps(dict(complete=False, elapsed=100, phase='observe')),
                       json.dumps(dict(complete=True, elapsed=102, phase='observe'))])
        with tempfile.TemporaryDirectory() as directory, patch('verify_resources.sample_process', side_effect=subprocess.CalledProcessError(1, ['ps'])):
            rows, result, completed = collect(process, lambda: next(states), None, 2222, 'smoke', Path(directory))
            self.assertTrue(completed)
            self.assertTrue(result['complete'])

    def test_collection_failure_is_not_hidden(self):
        import json
        import subprocess
        import tempfile
        from unittest.mock import Mock, patch
        from verify_resources import collect
        process = Mock()
        process.poll.return_value = None
        state = json.dumps(dict(complete=False, elapsed=100, phase='output'))
        with tempfile.TemporaryDirectory() as directory, patch('verify_resources.sample_process', side_effect=subprocess.CalledProcessError(1, ['ps'])):
            with self.assertRaises(subprocess.CalledProcessError):
                collect(process, lambda: state, None, 2222, 'smoke', Path(directory))

    def test_interruption_stops_owned_group(self):
        from unittest.mock import Mock, patch
        import signal
        from verify_resources import stop
        process = Mock(pid=123)
        process.poll.return_value = None
        with patch('verify_resources.os.killpg') as kill:
            stop(process, group=True)
            kill.assert_called_once_with(123, signal.SIGTERM)
            process.wait.assert_called_once()


if __name__ == '__main__':
    unittest.main()
