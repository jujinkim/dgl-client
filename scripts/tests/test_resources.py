from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from resource_results import evaluate, parse_meminfo, renderer_pids, PROFILES, MIB


def evidence(profile='soak'):
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
                       renderSequence=elapsed, queuedBytes=4096, sshSockets=1,
                       modelPhase='Disconnected' if phase in ('baseline', 'observe') else 'Ready')
            if t % 10 == 0:
                row.update(pssBytes=60*MIB, rendererRssBytes=50*MIB, rendererPssBytes=40*MIB,
                           rendererPids=[789], memoryDetail=True, anrCount=0, crashCount=0)
            rows.append(row)
    final = dict(rows[-1], complete=True, recoveries=spec['repeats'], recreations=spec['repeats'],
                 blockedInputs=spec['repeats'], checks=43, controlClients=0, slowConsumerChecked=True,
                 outputBytesPerSecond=65000)
    return rows, final


class ResourceResultsTest(unittest.TestCase):
    def test_smoke_checks_collection_without_two_minute_memory_windows(self):
        rows, final = evidence('smoke')
        for row in rows:
            if row['phaseElapsed'] < 15:
                for key in ('pssBytes', 'rendererRssBytes', 'rendererPssBytes'):
                    row.pop(key, None)
                row['memoryDetail'] = False
        result = evaluate(rows, final, 'smoke', True)
        self.assertEqual(result['errors'], [])
        self.assertFalse(result['memoryAcceptanceEvaluated'])
        self.assertFalse(result['qualifiesAsSoak'])

    def test_connecting_samples_cannot_inflate_idle_baseline(self):
        rows, final = evidence()
        baseline = [s for s in rows if s['phase'] == 'baseline']
        for row in baseline[3:]: row.update(modelPhase='Connecting', fd=100)
        for row in rows:
            if row['phase'] == 'observe': row['fd'] = 35
        self.assertIn('idle_growth_fd', evaluate(rows, final, 'soak', True)['errors'])

    def test_terminal_collection_is_required_at_end_of_observation(self):
        rows, final = evidence()
        for row in rows:
            if row['phase'] == 'observe': row['liveTerminals'] = 1
        self.assertEqual(evaluate(rows, final, 'soak', True)['errors'], [])
        final['liveTerminals'] = 1
        self.assertIn('retained_liveTerminals', evaluate(rows, final, 'soak', True)['errors'])

    def test_normal_soak(self):
        rows, final = evidence()
        result = evaluate(rows, final, 'soak', True)
        self.assertEqual(result['errors'], [])
        self.assertTrue(result['qualifiesAsSoak'])

    def test_android_renderer_separate(self):
        rows, final = evidence()
        result = evaluate(rows, final, 'soak', True)
        self.assertEqual(result['errors'], [])
        self.assertEqual(result['phases']['output']['rendererPssBytes']['sampleMax'], 40*MIB)

    def test_accumulating_memory(self):
        rows, final = evidence()
        for row in rows:
            if row['phase'] == 'recovery': row['rssBytes'] += row['phaseElapsed'] * MIB
        self.assertIn('recovery_growth_rssBytes', evaluate(rows, final, 'soak', True)['errors'])

    def test_fd_and_thread_growth(self):
        rows, final = evidence()
        for row in rows:
            if row['phase'] == 'observe': row.update(fd=35, threads=25)
        errors = evaluate(rows, final, 'soak', True)['errors']
        self.assertIn('idle_growth_fd', errors)
        self.assertIn('idle_growth_threads', errors)

    def test_missing_is_not_zero(self):
        rows, final = evidence()
        del rows[10]['rssBytes']
        self.assertIn('missing_rssBytes', evaluate(rows, final, 'soak', True)['errors'])
        self.assertEqual(parse_meminfo('permission denied'), dict(pssBytes=None, rssBytes=None))

    def test_interrupted_cannot_pass(self):
        rows, final = evidence()
        self.assertIn('incomplete_or_interrupted', evaluate(rows, final, 'soak', False)['errors'])

    def test_pid_replacement(self):
        for field, reason in [('appPid', 'app_pid_changed_or_missing'), ('remotePid', 'remote_pid_changed_or_missing')]:
            rows, final = evidence()
            rows[10][field] = 999
            self.assertIn(reason, evaluate(rows, final, 'soak', True)['errors'])

    def test_renderer_pid_replacement_tracks_sum(self):
        rows, final = evidence()
        for row in rows:
            if row.get('memoryDetail') and row['phase'] == 'recreate': row['rendererPids'] = [790]
        self.assertEqual(evaluate(rows, final, 'soak', True)['errors'], [])

    def test_missing_renderer_fails(self):
        rows, final = evidence()
        for row in rows:
            if row.get('memoryDetail') and row['phase'] == 'recovery': row['rendererPids'] = []
        self.assertIn('missing_android_memory', evaluate(rows, final, 'soak', True)['errors'])

    def test_smoke_is_never_soak(self):
        rows, final = evidence('smoke')
        self.assertFalse(evaluate(rows, final, 'smoke', True)['qualifiesAsSoak'])
        self.assertFalse(evaluate(rows, final, 'soak', True)['qualifiesAsSoak'])

    def test_short_or_gapped_collection(self):
        rows, final = evidence()
        self.assertIn('sample_gap', evaluate(rows[:20] + rows[30:], final, 'soak', True)['errors'])
        final['recoveries'] = 19
        self.assertIn('recovery_count', evaluate(rows, final, 'soak', True)['errors'])

    def test_meminfo_and_service_linkage(self):
        self.assertEqual(parse_meminfo('TOTAL PSS: 123 TOTAL RSS: 456 TOTAL SWAP PSS: 0'), dict(pssBytes=123*1024, rssBytes=456*1024))
        services = '''* ServiceRecord{a com.webview/SandboxedProcessService0}
 app=ProcessRecord{abc 1234:com.webview:sandboxed_process0/u0i1}
 binding=ProcessRecord{def 5678:kim.jujin.dglc/u0a1}
* ServiceRecord{b com.webview/SandboxedProcessService1}
 app=ProcessRecord{def 9999:com.webview:sandboxed_process1/u0i2}
 binding=ProcessRecord{gh 5555:other.app/u0a2}'''
        self.assertEqual(renderer_pids(services, 'kim.jujin.dglc'), [1234])


class ResourceCollectorTest(unittest.TestCase):
    def test_restoring_is_valid_memory_and_disconnection_is_not(self):
        import json
        from unittest.mock import patch
        from verify_resources import sample_android_memory
        origin = dict(phase='recreate', phaseElapsed=8, elapsed=108, modelPhase='Restoring')
        with patch('verify_resources.android_memory', return_value=dict(memoryDetail=True, pssBytes=100)):
            result = sample_android_memory(lambda: json.dumps(origin), ['adb'], origin)
            self.assertEqual(result['memoryPhase'], 'recreate')
            result = sample_android_memory(lambda: json.dumps(dict(origin, modelPhase='Disconnected')), ['adb'], origin)
            self.assertEqual(result['memoryPhase'], 'transition')

    def test_slow_memory_detail_does_not_block_app_samples(self):
        import json
        import tempfile
        import threading
        from unittest.mock import Mock, patch
        from verify_resources import collect
        release = threading.Event()
        count = 0
        def read_state():
            return json.dumps(dict(elapsed=(count + 1) * 2, phase='output', phaseElapsed=count * 2,
                                   modelPhase='Ready', complete=count >= 4))
        def memory(*args):
            if not release.wait(timeout=2):
                raise AssertionError('Detail collection blocked periodic app sampling')
            return dict(memoryDetail=True, pssBytes=100)
        def app_sample(state, adb, port, detail):
            nonlocal count
            count += 1
            if count == 4: release.set()
            if detail: state.update(memory())
            return state
        process = Mock(returncode=0)
        process.poll.return_value = None
        with tempfile.TemporaryDirectory() as directory, patch('verify_resources.time.sleep'), \
             patch('verify_resources.sample_process', side_effect=app_sample), \
             patch('verify_resources.android_memory', side_effect=memory) as detailed:
            rows, _, completed = collect(process, read_state, ['adb'], 2222, 'smoke', Path(directory))
        self.assertTrue(completed)
        self.assertEqual(len(rows), 4)
        detailed.assert_called_once()

    def test_memory_windows_use_query_time_not_merge_time(self):
        rows, final = evidence()
        for row in rows:
            if row.get('memoryDetail'): row['memoryPhaseElapsed'] = row['phaseElapsed'] + 250
        errors = evaluate(rows, final, 'soak', True)['errors']
        self.assertIn('output_missing_windows_pssBytes', errors)

    @unittest.skipIf(sys.platform == 'win32', 'Isolated OpenSSH fixture uses POSIX processes')
    def test_fixture_cleanup_kills_only_its_private_process(self):
        import subprocess
        import tempfile
        from integration import cleanup_tmux
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'fixture'
            root.mkdir()
            command = [sys.executable, '-c', 'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); print("ready",flush=True); time.sleep(60)']
            owned = subprocess.Popen(command + [str(root / 'owned')], stdout=subprocess.PIPE, text=True)
            other = subprocess.Popen(command + [str(root) + '-other/owned'], stdout=subprocess.PIPE, text=True)
            try:
                self.assertEqual(owned.stdout.readline().strip(), 'ready')
                self.assertEqual(other.stdout.readline().strip(), 'ready')
                cleanup_tmux(root, '/usr/bin/false')
                owned.wait(timeout=2)
                self.assertIsNone(other.poll(), 'A sibling path is not part of this fixture')
            finally:
                for process in (owned, other):
                    if process.poll() is None: process.kill()
                    process.wait(); process.stdout.close()

    def test_closed_renderer_at_phase_boundary_is_not_a_stall(self):
        rows, final = evidence()
        ending = [s for s in rows if s['phase'] == 'recreate'][-1]
        ending.update(modelPhase='Disconnected', renderSequence=0)
        self.assertEqual(evaluate(rows, final, 'soak', True)['errors'], [])

    def test_cross_phase_renderer_sample_is_not_misattributed(self):
        rows, final = evidence()
        ending = [s for s in rows if s['phase'] == 'recovery' and s.get('memoryDetail')][-1]
        ending.update(memoryPhase='transition', rendererPids=[999])
        self.assertEqual(evaluate(rows, final, 'soak', True)['errors'], [])

    def test_negative_missing_metric_fails(self):
        rows, final = evidence()
        rows[20]['fd'] = -1
        self.assertIn('missing_fd', evaluate(rows, final, 'soak', True)['errors'])

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
            rows, result, completed = collect(process, lambda: json.dumps(final), ['adb'], 2222, 'smoke', Path(directory))
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
            rows, result, completed = collect(process, lambda: next(states), ['adb'], 2222, 'smoke', Path(directory))
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
                collect(process, lambda: state, ['adb'], 2222, 'smoke', Path(directory))

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
