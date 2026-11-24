import calendar
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import contextlib
import shlex
import shutil
import sqlite3
import subprocess
import time
import unittest
from unittest.mock import patch


def node_strips_types():
    """Node runs TypeScript by stripping types from 23.6 (22.18 on the 22 line)."""
    if not shutil.which('node'):
        return False
    try:
        version = subprocess.run(['node', '--version'], stdout=subprocess.PIPE, timeout=10).stdout.decode()
        major, minor = (int(x) for x in version.strip().lstrip('v').split('.')[:2])
    except (OSError, ValueError, subprocess.SubprocessError):
        return False
    return (major, minor) >= (23, 6) or (major == 22 and minor >= 18)

SOURCE = Path(__file__).resolve().parents[2] / 'crates/dgl-core/agent/observer.py'
spec = importlib.util.spec_from_file_location('observer', SOURCE)
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class AgentObserverTest(unittest.TestCase):
    def setUp(self):
        self.config_paths = observer.config_paths
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.patches = [patch.object(observer, 'ROOT', self.root / 'dgl'),
                        patch.object(observer, 'HELPER', self.root / 'dgl/observer.py'),
                        patch.object(observer, 'DB', self.root / 'dgl/events.sqlite'),
                        patch.object(observer, 'config_paths', return_value={a: self.root / (a + '.json') for a in observer.EVENTS}),
                        patch.object(observer, 'status_paths', return_value={'Agy': self.root / 'agy-settings.json'}),
                        # Passive status sources must never read this machine's CLI homes.
                        patch.dict(os.environ, {'GROK_HOME': str(self.root / 'grok-home'), 'CODEX_HOME': str(self.root / 'codex-home')})]
        for p in self.patches:
            p.start()
        self.addCleanup(self.tmp.cleanup)
        for p in self.patches:
            self.addCleanup(p.stop)

    def test_install_remove_preserves_foreign_groups_and_is_idempotent(self):
        foreign = {'type': 'command', 'command': 'echo user-owned'}
        config = {'permissions': {'allow': ['Read']}, 'hooks': {'Stop': [{'matcher': 'x', 'hooks': [foreign]}]}}
        installed = observer.transformed(config, 'Codex', '/usr/bin/tmux', True)
        self.assertEqual(installed['hooks']['Stop'][0], config['hooks']['Stop'][0])
        self.assertEqual(installed, observer.transformed(installed, 'Codex', '/usr/bin/tmux', True))
        self.assertEqual(config, observer.transformed(installed, 'Codex', '/usr/bin/tmux', False))
        # Co-located foreign handlers must survive removal.
        installed['hooks']['Stop'][1]['hooks'].append(foreign)
        self.assertEqual(observer.transformed(installed, 'Codex', '/usr/bin/tmux', False)['hooks']['Stop'][1]['hooks'], [foreign])

    def test_preview_conflict_backups_and_unsupported_versions(self):
        with patch.object(observer.sys, 'version_info', (3, 8, 0)):
            with self.assertRaisesRegex(ValueError, 'Python 3.9'):
                observer.inspect('/bin/tmux')
        path = self.root / 'Codex.json'
        path.write_text('{"theme":"user"}')
        def version(args):
            return 'codex-cli 0.163.0' if 'codex' in args[0] else 'Claude Code 2.1.310' if 'claude' in args[0] else 'tmux 3.3a'
        with patch.object(observer, 'run', side_effect=version), patch.object(observer.shutil, 'which', side_effect=lambda s: '/bin/' + s):
            before = observer.inspect('/bin/tmux')
            path.write_text('{"theme":"changed"}')
            with self.assertRaisesRegex(ValueError, 'changed'):
                observer.configure('/bin/tmux', before['token'], True, SOURCE.read_text())
            self.assertFalse(observer.HELPER.exists())
            fresh = observer.inspect('/bin/tmux')
            installed = observer.configure('/bin/tmux', fresh['token'], True, SOURCE.read_text())
            self.assertTrue(all(c['installed'] for c in installed['configs']))
            self.assertTrue(list(self.root.glob('Codex.json.dgl-backup-*')))
            removed = observer.configure('/bin/tmux', installed['token'], False, '')
            self.assertFalse(any(c['installed'] for c in removed['configs']))
            self.assertEqual(json.loads(path.read_text()), {'theme': 'changed'})
        with patch.object(observer, 'run', return_value='0.1.0'), patch.object(observer.shutil, 'which', return_value='/bin/unknown'):
            preview = observer.inspect('/bin/tmux')
            self.assertFalse(any(c['supported'] for c in preview['configs'] if not c['experimental']))
            self.assertTrue(all(c['supported'] for c in preview['configs'] if c['experimental']))
        with patch.object(observer.shutil, 'which', return_value=None), patch.object(observer, 'run', return_value='tmux 3.3a'):
            preview = observer.inspect('/bin/tmux')
            with self.assertRaisesRegex(ValueError, 'unsupported'):
                observer.configure('/bin/tmux', preview['token'], True, SOURCE.read_text())

    def test_atomic_replace_rejects_external_changes_and_symlinks(self):
        path = self.root / 'settings.json'
        path.write_bytes(b'new external content')
        with self.assertRaisesRegex(ValueError, 'changed'):
            observer.atomic_write(path, b'ours', b'old')
        self.assertEqual(path.read_bytes(), b'new external content')
        link = self.root / 'link.json'
        link.symlink_to(path)
        with self.assertRaises(ValueError):
            observer.load_config(link)

    def test_current_phase_tracks_parallel_tools_for_all_providers(self):
        for agent, tool in [('Codex', 'exec_command'), ('Claude', 'Bash'), ('Agy', 'run_command'), ('Grok', 'run_terminal_cmd')]:
            with self.subTest(agent=agent), observer.database() as db:
                def step(event, ident=None, name=None, **fields):
                    payload = dict(fields, tool_input={'command': 'SECRET'})
                    if agent in ('Claude', 'Codex'):
                        payload.update(tool_use_id=ident, tool_name=name)
                    elif agent == 'Agy':
                        payload.update(toolCall={'id': ident, 'name': name})
                    else:
                        payload.update(toolUseId=ident, toolName=name)
                    observer.update_phase(db, 'run', agent, agent, event, payload, observer.kind(agent, event, payload), 1, 1)
                    raw, = db.execute('SELECT data FROM phases WHERE run=? AND actor=?', ('run', agent)).fetchone()
                    self.assertNotIn('SECRET', raw)
                    return observer.phase_value(json.loads(raw))
                self.assertEqual(step('SessionStart'), 'idle')
                self.assertEqual(step('PreToolUse', 'first', tool), 'command')
                self.assertEqual(step('PreToolUse', 'second', tool), 'command')
                if agent == 'Agy':
                    self.assertEqual(step('PreToolUse', 'consent', 'ask_permission'), 'approval')
                    self.assertEqual(step('PostToolUse', 'consent', 'ask_permission'), 'command')
                elif agent == 'Grok':
                    self.assertEqual(step('Notification', notificationType='permission_prompt'), 'approval')
                else:
                    self.assertEqual(step('PermissionRequest', 'second', tool), 'approval')
                self.assertEqual(step('PostToolUse', 'second', tool), 'command')
                self.assertEqual(step('PostToolUse', 'first', tool), 'working')
                self.assertEqual(step('Stop', terminationReason='model_stop'), 'idle')

    def test_statusline_reports_phase_without_numeric_fields_or_events(self):
        pane = {'session': '$1', 'window': '@1', 'pane': '%0', 'pid': 100}
        procs = {100: {'agent': 'Agy', 'parent': 10, 'token': 'native'}}
        with patch.object(observer, 'resolve', return_value=('server', pane, procs, 100)), \
                patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.dict(os.environ, TMUX='socket'):
            for payload, expected in [({'agent_state': 'reviewing'}, 'reviewing'),
                                      ({'agent_state': 'tool_use', 'tool_confirmation_pending': True}, 'approval'),
                                      ({'agent_state': 'idle'}, 'idle')]:
                observer.record_statusline('Agy', 'tmux', payload)
                frame = observer.poll('tmux', '', 0, '')
                self.assertEqual(frame['observations'][0]['phase'], expected)
                self.assertEqual(frame['cursor'], 0)

    def test_psmux_scopes_reused_ids_and_resolves_hook_by_process_tree(self):
        def mux(args):
            if 'list-sessions' in args:
                return '$1\n$2'
            return '@0|%0|' + ('10' if '$1' in args else '20')
        procs = {pid: {'agent': 'Claude', 'token': str(pid), 'parent': 0} for pid in [10, 20]}
        with patch.object(observer, 'WINDOWS', True), patch.object(observer, 'processes', return_value=procs), \
                patch.object(observer, 'run', side_effect=mux), patch.object(observer.os, 'getppid', return_value=20):
            _, panes, _ = observer.topology('psmux')
            self.assertEqual([p['pane'] for p in panes], ['%4294967296', '%8589934592'])
            self.assertEqual(observer.resolve('Claude', 'psmux')[1], panes[1])

    def test_windows_config_commands_roundtrip_paths_as_data(self):
        with patch.object(observer, 'WINDOWS', True), patch.object(observer, 'HELPER', Path('C:/user & %example%/DGL/observer.py')):
            cmd = observer.hook_command('Claude', 'C:/Program Files/psmux.exe', 'Stop')
            self.assertNotIn('%example%', cmd)
            self.assertTrue(observer.owned({'type': 'command', 'command': cmd}))
            self.assertFalse(observer.owned({'type': 'command', 'command': cmd + ' & other'}))
            self.assertTrue(observer.owned_tee({'command': observer.tee_command('psmux.exe')}))

    def test_default_poll_reads_live_state_without_install_or_quota_requests(self):
        pane, procs = self.claude_pane('Codex')
        live = observer.LiveCollector()
        def native_phase(candidates, *_):
            return {run: 'idle' for _, _, run in candidates}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), \
                patch.object(observer, 'codex_phases', side_effect=native_phase), \
                patch.object(live, 'fetch', side_effect=lambda key, query, ttl: query()), \
                patch.object(observer, 'cached_account') as quota:
            frame = observer.poll('tmux', '', 0, '', live)
        self.assertEqual(frame['observations'][0]['phase'], 'idle')
        self.assertFalse(frame['account_limits'])
        quota.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [])

    def test_native_phase_attribution_and_limit_consent(self):
        thread = '019ca012-1111-7333-a444-555555555555'
        peer = {'originator': 'codex-tui', 'source': 'vscode', 'cwd': '/p', 'status': {'type': 'active', 'activeFlags': ['waitingOnApproval']}}
        def rpc(method, params):
            if method == 'thread/loaded/list':
                return {'data': [thread]}
            if method == 'thread/read':
                self.assertIs(params['includeTurns'], False)
                return {'thread': peer}
            self.assertEqual(method, 'account/rateLimits/read')
            return {'rateLimitsByLimitId': {'model': {'primary': {'usedPercent': 29, 'windowDurationMins': 300}}}}
        with patch.object(observer, 'CodexControl') as control, patch.object(observer, 'daemon_clients', return_value={1, 2}), \
                patch.object(observer, 'process_cwd', side_effect=lambda pid: '/p' if pid == 1 else '/q'):
            control.return_value.__enter__.return_value.request.side_effect = rpc
            procs = {pid: {'agent': 'Codex'} for pid in (1, 2)}
            candidates = [('%1', 1, 'a')]
            self.assertEqual(observer.codex_phases(candidates, procs, {}), {'a': 'approval'})
            def overlapping(method, params):
                if method == 'thread/loaded/list':
                    return {'data': [thread, '019ca012-1111-7333-a444-555555555556']}
                return rpc(method, params)
            control.return_value.__enter__.return_value.request.side_effect = overlapping
            self.assertEqual(observer.codex_phases(candidates, procs, {}), {})
            control.return_value.__enter__.return_value.request.side_effect = rpc
            peer['cwd'] = '/q'  # Another CLI outside this tmux server.
            self.assertEqual(observer.codex_phases(candidates, procs, {}), {})
            self.assertEqual(observer.cached_account('Codex'), {})
            with observer.database() as db:
                db.execute("INSERT OR REPLACE INTO meta VALUES('account_limits','1')")
            value = observer.cached_account('Codex')
            self.assertEqual(value['limits'][0]['used_percent'], 29)
            self.assertEqual(observer.cached_account('Codex'), value)
            methods = [args.args[0] for args in control.return_value.__enter__.return_value.request.call_args_list]
            self.assertEqual(methods.count('account/rateLimits/read'), 1)
        with patch.object(observer, 'grok_billing', return_value={'creditUsagePercent': 0, 'currentPeriod': {'end': '2026-06-08T00:00:00Z'}}):
            limit = observer.account_sample('Grok')['limits'][0]
            self.assertEqual(limit['used_percent'], 0)
            self.assertGreater(limit['resets_at'], 0)

    def test_serve_answers_each_poll_line_until_stdin_closes(self):
        requests = [{'session': '$1', 'after': 3, 'epoch': 'e'}, {'session': '$2', 'after': 0, 'epoch': ''}]
        stdin = io.StringIO(''.join(json.dumps(r) + '\n' for r in requests) + 'not json\n')
        output = io.StringIO()
        polled = []
        def poll(tmux, session, after, epoch, live):
            self.assertIsInstance(live, observer.LiveCollector)
            polled.append((tmux, session, after, epoch))
            if session == '$2':
                raise OSError('secret detail')
            return {'server': 's', 'note': '한글'}
        with patch.object(observer.sys, 'stdin', stdin), patch.object(observer, 'poll', poll), \
                patch.object(observer.sys, 'argv', ['observer.py', 'serve', '--tmux', '/bin/tmux']), \
                contextlib.redirect_stdout(output):
            observer.main()
        self.assertEqual(polled, [('/bin/tmux', '$1', 3, 'e'), ('/bin/tmux', '$2', 0, '')])
        # Not splitlines(): it also breaks at RS.
        lines = output.getvalue().split('\n')[:-1]
        self.assertTrue(all(line.startswith('\x1e') for line in lines))
        self.assertEqual([json.loads(line[1:]) for line in lines],
                         [{'server': 's', 'note': '한글'}, {'error': 'failed'}, {'error': 'failed'}])

    def test_an_empty_session_observes_every_session_of_the_server(self):
        procs = {10: {'parent': 1, 'token': 't', 'agent': None, 'launcher': False}}
        def run(args):
            if args[1] == 'display-message':
                return '/socket|10'
            return '$1|@1|%1|20\n$2|@2|%2|30'
        with patch.object(observer, 'processes', return_value=procs), patch.object(observer, 'run', side_effect=run):
            _, every, _ = observer.topology('/bin/tmux', '')
            _, attached, _ = observer.topology('/bin/tmux', '$1')
        self.assertEqual([p['session'] for p in every], ['$1', '$2'])
        self.assertEqual([p['session'] for p in attached], ['$1'])

    def test_helper_output_has_codes_not_ui_copy(self):
        source = SOURCE.read_text(encoding='utf-8')
        self.assertFalse(any('\uac00' <= c <= '\ud7a3' for c in source), 'UI copy belongs to the app catalog')

    def test_process_evidence_does_not_match_prompt_or_generic_node(self):
        start = 'Fri Oct 23 12:00:00 2026'
        rows = [f'1 0 {start} /bin/bash bash', f'2 1 {start} /usr/bin/node node arbitrary.js codex',
                f'3 1 {start} /opt/bin/codex /opt/bin/codex',
                f'4 1 {start} /usr/bin/node node /npm/@anthropic-ai/claude-code/cli.js',
                f'5 1 {start} /usr/bin/node node --eval "codex"',
                f'6 1 {start} /bin/echo echo /npm/@openai/codex/bin/codex.js',
                f'7 1 {start} /opt/bin/agy /opt/bin/agy',
                f'8 1 {start} /opt/bin/grok /opt/bin/grok',
                f'9 1 {start} /usr/bin/node node arbitrary.js agy grok']
        def ps(args):
            if args[-1] == 'pid=,args=':
                return '\n'.join(r.split(None, 1)[0] + ' ' + r.split(None, 8)[8] for r in rows)
            return '\n'.join(' '.join(r.split(None, 8)[:8]) for r in rows)
        with patch.object(observer, 'run', side_effect=ps):
            procs = observer.processes()
        self.assertEqual([pid for pid, p in procs.items() if p['agent']], [3, 4, 7, 8])

    def test_official_contract_kinds_and_neutral_output_even_on_failure(self):
        fixture = json.loads((SOURCE.parents[3] / 'fixtures/agent-hooks/contracts.json').read_text())
        for case in fixture['cases']:
            with self.subTest(agent=case['agent'], event=case['event'], payload=case['payload']):
                self.assertEqual(observer.kind(case['agent'], case['event'], case['payload']), case['kind'])
                for fail in (False, True):
                    output = io.StringIO()
                    argv = ['observer.py', 'record', '--agent', case['agent'], '--event', case['event'], '--tmux', '/bin/tmux']
                    with patch.object(observer.sys, 'argv', argv), patch.object(observer, 'record', side_effect=ValueError if fail else None), contextlib.redirect_stdout(output):
                        observer.main()
                    self.assertEqual(json.loads(output.getvalue()), case['output'])

    def test_agy_and_grok_config_shapes_preservation_and_paths(self):
        foreign = {'type': 'command', 'command': 'echo user-owned'}
        user_namespace = {'dgl-observer-v1': {'enabled': False}}
        self.assertEqual(observer.transformed(user_namespace, 'Agy', '/bin/tmux', False), user_namespace)
        with self.assertRaisesRegex(ValueError, 'collision'):
            observer.transformed(user_namespace, 'Agy', '/bin/tmux', True)
        for agent in ('Agy', 'Grok'):
            original = {'user-hook': {'SessionStart': [foreign]}} if agent == 'Agy' else {'theme': 'user', 'hooks': {'Stop': [{'hooks': [foreign]}]}}
            installed = observer.transformed(original, agent, '/bin/tmux', True)
            self.assertEqual(installed, observer.transformed(installed, agent, '/bin/tmux', True))
            self.assertEqual(original, observer.transformed(installed, agent, '/bin/tmux', False))
            if agent == 'Agy':
                definition = installed['dgl-observer-v1']
                self.assertIn('--event SessionStart', definition['SessionStart'][0]['command'])
                self.assertIn('hooks', definition['PreToolUse'][0])
                definition['Stop'].append(foreign)
                definition['PreToolUse'][0]['hooks'].append(foreign)
                removed = observer.transformed(installed, agent, '/bin/tmux', False)
                self.assertEqual(removed['dgl-observer-v1']['Stop'], [foreign])
                self.assertEqual(removed['dgl-observer-v1']['PreToolUse'][0]['hooks'], [foreign])
        # Use the real path resolver, rather than this test class's isolated resolver.
        with patch.dict(os.environ, {'GROK_HOME': '/fixture/grok'}), patch.object(observer.Path, 'home', return_value=Path('/fixture/home')):
            paths = self.config_paths()
            self.assertEqual(paths['Grok'], Path('/fixture/grok/hooks/dgl-observer-v1.json'))
            self.assertEqual(paths['Agy'], Path('/fixture/home/.gemini/config/hooks.json'))

    def test_versions_and_bad_cli_configs_are_isolated(self):
        # The oldest really received release and every later stable release are installable.
        cases = [('0.163.0', '2.1.310', True), ('0.164.0', '2.1.330', True), ('0.171.0', '2.10.0', True),
                 ('0.162.9', '2.1.309', False), ('0.164.00', '2.1.310-beta.1', False),
                 ('0.164.0-alpha.1', '2.1.0310', False)]
        for codex, claude, expected in cases:
            def run(args):
                if args[0].endswith('codex'):
                    return 'codex-cli ' + codex
                return claude + ' (Claude Code)' if args[0].endswith('claude') else 'fixture 1.4.4'
            with patch.object(observer, 'run', side_effect=run), patch.object(observer.shutil, 'which', side_effect=lambda s: '/bin/' + s):
                configs = {c['agent']: c for c in observer.inspect('/bin/tmux')['configs']}
                self.assertEqual(configs['Codex']['supported'], expected, codex)
                self.assertEqual(configs['Claude']['supported'], expected, claude)
                self.assertTrue(configs['Agy']['experimental'])
                self.assertIn('Question', configs['Grok']['capabilities'])
        (self.root / 'Agy.json').write_text('{invalid')
        (self.root / 'Grok.json').write_text('{"hooks":{"Stop":"invalid"}}')
        with patch.object(observer, 'run', return_value='codex-cli 0.164.0'), patch.object(observer.shutil, 'which', side_effect=lambda s: '/bin/' + s):
            preview = observer.inspect('/bin/tmux')
            configs = {c['agent']: c for c in preview['configs']}
            self.assertTrue(configs['Codex']['supported'])
            self.assertTrue(configs['Agy']['error'])
            self.assertTrue(configs['Grok']['error'])
            installed = observer.configure('/bin/tmux', preview['token'], True, SOURCE.read_text())
            self.assertTrue(installed['configs'][0]['installed'])
            self.assertEqual((self.root / 'Agy.json').read_text(), '{invalid')

    def emit(self, agent, event, payload, explicit=True):
        payload = dict(payload)
        if not explicit:
            payload['hook_event_name'] = event
        stdin = type('Input', (), {'buffer': io.BytesIO(json.dumps(payload).encode())})()
        with patch.object(observer.sys, 'stdin', stdin):
            observer.record(agent, '/bin/tmux', event if explicit else None)

    def test_codex_app_server_daemon_is_not_a_cli(self):
        start = 'Fri Oct 23 12:00:00 2026'
        rows = [f'1 0 {start} /bin/bash bash',
                f'2 1 {start} /opt/bin/codex /opt/bin/codex',
                f'3 2 {start} /home/.codex/packages/app-server-daemon/bin/codex /home/.codex/packages/app-server-daemon/bin/codex app-server --listen unix:// --managed-daemon',
                f'4 3 {start} /opt/bin/codex codex app-server daemon pid-update-loop',
                f'5 1 {start} /usr/bin/node node /npm/@openai/codex/bin/codex.js app-server --listen unix://',
                f'6 1 {start} /usr/bin/node node /npm/@openai/codex/bin/codex.js --model app-server',
                f'7 1 {start} /Users/a_b/codex /Users/a b/.codex/bin/codex app-server --listen unix:// --managed-daemon']
        def ps(args):
            if args[-1] == 'pid=,args=':
                return '\n'.join(r.split(None, 1)[0] + ' ' + r.split(None, 8)[8] for r in rows)
            return '\n'.join(' '.join(r.split(None, 8)[:8]) for r in rows)
        with patch.object(observer, 'run', side_effect=ps):
            procs = observer.processes()
        self.assertEqual([pid for pid, p in procs.items() if p['agent']], [2, 6])
        self.assertEqual([pid for pid, p in procs.items() if p['daemon']], [3, 4, 5, 7])
        # Only the TUIs' managed daemon is attributed; an IDE's own app-server never is.
        self.assertEqual([pid for pid, p in procs.items() if p['managed']], [3, 7])
        pane = {'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1}
        # Only the TUI and the unrelated launcher; the daemon under the TUI is no second Codex.
        self.assertEqual([c[1] for c in observer.pane_clis('server', [pane], procs)], [2, 6])
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)):
            self.assertEqual([o['agent'] for o in observer.poll('/bin/tmux', '', 0, '')['observations']], ['Codex', 'Codex'])

    def test_codex_daemon_hooks_bind_threads_to_the_tui_by_working_directory(self):
        panes = [{'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1},
                 {'session': '$1', 'window': '@2', 'pane': '%2', 'pid': 10},
                 {'session': '$1', 'window': '@3', 'pane': '%3', 'pid': 20}]
        procs = {1: {'parent': 0, 'token': 'shell-1', 'agent': None},
                 2: {'parent': 1, 'token': 'tui-1', 'agent': 'Codex'},
                 # The daemon descends from the first TUI and carries that pane's environment.
                 3: {'parent': 2, 'token': 'daemon', 'agent': None, 'daemon': 'Codex', 'managed': True},
                 4: {'parent': 3, 'token': 'hook-shell', 'agent': None},
                 10: {'parent': 0, 'token': 'shell-2', 'agent': None},
                 11: {'parent': 10, 'token': 'tui-2', 'agent': 'Codex'}}
        cwd = {2: '/w1', 11: '/w2', 21: '/w2'}
        def pane_of(event):
            return event['pane'] if event else None
        def poll():
            return observer.poll('/bin/tmux', '', 0, observer.poll('/bin/tmux', '', 0, '')['epoch'])
        with patch.object(observer, 'topology', return_value=('server', panes, procs)), \
                patch.object(observer, 'process_cwd', side_effect=lambda pid: cwd.get(pid)), \
                patch.object(observer, 'daemon_clients', return_value=None), \
                patch.object(observer.os.path, 'realpath', side_effect=lambda p: p), \
                patch.object(observer.os, 'getppid', return_value=4), \
                patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.emit('Codex', 'SessionStart', {'session_id': 'thread-a', 'cwd': '/w2'})
            self.emit('Codex', 'SessionStart', {'session_id': 'thread-b', 'cwd': '/w1'})
            snapshot = poll()
            self.assertEqual([pane_of(e) for e in snapshot['events']], ['%2', '%1'])
            self.assertEqual(sorted(o['pane'] for o in snapshot['observations']), ['%1', '%2'])
            # A bound thread stays with its TUI even if the payload's cwd changes later.
            self.emit('Codex', 'Stop', {'session_id': 'thread-a', 'cwd': '/elsewhere'})
            # `/new` in the only TUI of a directory: the new thread joins that TUI's run.
            self.emit('Codex', 'SessionStart', {'session_id': 'thread-c', 'cwd': '/w2'})
            # A second TUI in that directory makes an unbound thread ambiguous: dropped, never guessed.
            procs[20] = {'parent': 0, 'token': 'shell-3', 'agent': None}
            procs[21] = {'parent': 20, 'token': 'tui-3', 'agent': 'Codex'}
            self.emit('Codex', 'SessionStart', {'session_id': 'thread-d', 'cwd': '/w2'})
            # Bound threads keep their pane even then.
            self.emit('Codex', 'Stop', {'session_id': 'thread-a', 'cwd': '/w2'})
            # No matching directory or no thread id: dropped.
            self.emit('Codex', 'SessionStart', {'session_id': 'thread-e', 'cwd': '/nowhere'})
            self.emit('Codex', 'SessionStart', {'cwd': '/w1'})
            events = poll()['events']
            self.assertEqual([(e['event'], pane_of(e)) for e in events],
                             [('SessionStart', '%2'), ('SessionStart', '%1'), ('Stop', '%2'), ('SessionStart', '%2'), ('Stop', '%2')])
            # An IDE's own (unmanaged) app-server hook is dropped even when a TUI shares its directory.
            procs[3]['managed'] = False
            self.emit('Codex', 'SessionStart', {'session_id': 'ide-thread', 'cwd': '/w1'})
            self.assertEqual(len(poll()['events']), len(events))
            procs[3]['managed'] = True
            # Status-only PostModelRequest follows the same binding.
            self.emit('Codex', 'PostModelRequest', {'session_id': 'thread-b', 'cwd': '/w1', 'model': 'gpt-x', 'kind': 'sampling',
                                                    'token_usage': {'input_tokens': 10, 'cached_input_tokens': 4, 'output_tokens': 2, 'total_tokens': 12}})
            first = next(o for o in poll()['observations'] if o['pane'] == '%1')
            self.assertEqual(first['status']['cache']['read'], 4)
        # An orphaned daemon (parent launchd) without TMUX still reaches the bound TUI.
        procs[3]['parent'] = 0
        with patch.object(observer, 'topology', return_value=('server', panes, procs)), \
                patch.object(observer, 'process_cwd', side_effect=lambda pid: cwd.get(pid)), \
                patch.object(observer.os.path, 'realpath', side_effect=lambda p: p), \
                patch.object(observer.os, 'getppid', return_value=4), patch.dict(os.environ, {}, clear=True):
            self.emit('Codex', 'Stop', {'session_id': 'thread-b', 'cwd': '/w1'})
            self.assertEqual(pane_of(poll()['events'][-1]), '%1')
        # A legacy in-process hook (no daemon) still needs its own pane.
        procs[4]['parent'] = 2
        with patch.object(observer, 'topology', return_value=('server', panes, procs)), \
                patch.object(observer.os, 'getppid', return_value=4), patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.emit('Codex', 'Stop', {'session_id': 'legacy'})
            self.assertEqual(pane_of(poll()['events'][-1]), '%1')

    def test_codex_daemon_hooks_tell_same_directory_tuis_apart_by_socket_and_start_order(self):
        s0 = 1795356660
        def v7(ms, n):
            h = '%012x' % ms
            return '%s-%s-7%03x-8000-%012x' % (h[:8], h[8:], n, n)
        ta, tb, tc, tn = v7(s0 * 1000 + 5200, 1), v7(s0 * 1000 + 8250, 2), v7(s0 * 1000 + 16300, 3), v7(s0 * 1000 + 45000, 4)
        panes = [{'session': '$1', 'window': '@%d' % n, 'pane': '%%%d' % n, 'pid': n * 10} for n in range(1, 7)]
        procs = {3: {'parent': 12, 'token': 'daemon', 'agent': None, 'daemon': 'Codex', 'managed': True},
                 4: {'parent': 3, 'token': 'hook-shell', 'agent': None}}
        # Five TUIs: three in /w (started 0 s, 7 s, 15 s), one outside the daemon, one with --cd.
        for n, begun in ((1, s0), (2, s0 + 7), (3, s0 + 15), (4, s0 + 16), (5, s0 + 60), (6, s0 + 61)):
            procs[n * 10] = {'parent': 0, 'token': 'shell-%d' % n, 'agent': None}
            procs[n * 10 + 2] = {'parent': n * 10, 'token': 'tui-%d' % n, 'agent': 'Codex', 'started': begun,
                                 'threads': frozenset(), 'reuses': False}
        cwd = {12: '/w', 22: '/w', 32: '/w', 42: '/w', 52: '/home', 62: '/w'}
        clients = {12, 22, 32, 52, 62}
        home = self.root / 'codex-home'
        # The first TUI's startup thread already unloaded after its `/new`; it is saved, being prompted.
        (home / 'thread-writer-locks').mkdir(parents=True)
        for thread in (tb, tc, tn):
            (home / 'thread-writer-locks' / (thread + '.lock')).touch()
        with contextlib.closing(sqlite3.connect(home / 'state_5.sqlite')) as db, db:
            db.execute('CREATE TABLE threads (id TEXT PRIMARY KEY, reasoning_effort TEXT, model TEXT)')
            db.executemany('INSERT INTO threads(id) VALUES (?)', [(ta,), (tb,), (tn,)])
        def poll():
            return observer.poll('/bin/tmux', '', 0, observer.poll('/bin/tmux', '', 0, '')['epoch'])
        with patch.object(observer, 'topology', return_value=('server', panes, procs)), \
                patch.object(observer, 'process_cwd', side_effect=lambda pid: cwd.get(pid)), \
                patch.object(observer, 'daemon_clients', side_effect=lambda daemon, pids: clients & pids if daemon == 3 else None), \
                patch.object(observer.os.path, 'realpath', side_effect=lambda p: p), \
                patch.object(observer.os, 'getppid', return_value=4), \
                patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            # Each startup thread goes to the TUI that created it, even prompted out of order.
            for thread in (tc, ta, tb):
                self.emit('Codex', 'SessionStart', {'session_id': thread, 'cwd': '/w'})
            # A later `/new` among several TUIs of the directory has no owner: dropped.
            self.emit('Codex', 'SessionStart', {'session_id': tn, 'cwd': '/w'})
            # Overlapping starts (60 s and 61 s) with no known thread between: either TUI could own it.
            t61 = v7((s0 + 61) * 1000 + 900, 6)
            self.emit('Codex', 'SessionStart', {'session_id': t61, 'cwd': '/w'})
            # `codex --cd`: no TUI has the thread's directory, so start order alone decides.
            tcd = v7((s0 + 60) * 1000 + 500, 5)
            (home / 'thread-writer-locks' / (tcd + '.lock')).touch()
            self.emit('Codex', 'SessionStart', {'session_id': tcd, 'cwd': '/elsewhere'})
            # `codex resume <id>` owns that thread wherever it runs.
            old = '0190aaaa-0000-7000-8000-000000000001'
            procs[62].update(threads=frozenset({old}), reuses=True)
            self.emit('Codex', 'SessionStart', {'session_id': old, 'cwd': '/w'})
            events = [(e['event'], e['pane']) for e in poll()['events']]
            self.assertEqual(events, [('SessionStart', '%3'), ('SessionStart', '%1'), ('SessionStart', '%2'),
                                      ('SessionStart', '%5'), ('SessionStart', '%6')])
            # A TUI outside the daemon (no socket to it) never takes a thread, even started in time.
            # Without the socket table, start order is not used at all.
            clients.clear()
            self.emit('Codex', 'SessionStart', {'session_id': v7(s0 * 1000 + 16500, 7), 'cwd': '/w'})
            self.assertEqual(len(poll()['events']), len(events))
        # The process table keeps only the thread IDs and the reuse flag, never the command line.
        args = {1: '/opt/bin/codex resume 0190AAAA-0000-7000-8000-000000000001', 2: '/opt/bin/codex fix resume flow'}
        def ps(argv):
            if argv[-1] == 'pid=,args=':
                return '\n'.join('%d %s' % kv for kv in args.items())
            return '\n'.join('%d 0 Fri Oct 23 12:00:00 2026 /opt/bin/codex' % pid for pid in args)
        with patch.object(observer, 'run', side_effect=ps):
            table = observer.processes()
        self.assertEqual(table[1]['threads'], frozenset({old}))
        self.assertTrue(table[1]['reuses'])
        # A prompt word in a later argument never counts as a subcommand.
        self.assertFalse(table[2]['reuses'])

    def test_codex_daemon_clients_pair_socket_endpoints_only(self):
        lsof_daemon = 'p7\nf7\nd0xa\nn->0xb\nf32\nd0xlisten\nn/tmp/codex-daemon-1/s\nf37\nd0xc1\nn/tmp/codex-daemon-1/s\nf43\nd0xc2\nn/tmp/codex-daemon-1/s\n'
        lsof_tuis = 'p20\nf38\nd0xt1\nn->0xc1\np30\nf38\nd0xt3\nn->0xother\np40\nf38\nd0xt4\nn->0xc2\n'
        with patch.object(observer, 'output', side_effect=lambda a: lsof_daemon if '7' in a else lsof_tuis):
            self.assertEqual(observer.clients_from_lsof(7, {20, 30}), {20})
        ss = ('Netid State Recv-Q Send-Q Local Address:Port Peer Address:Port Process\n'
              'u_str ESTAB 0 0 /tmp/codex-daemon-1/s 100 * 200 users:(("codex",pid=7,fd=37))\n'
              'u_str ESTAB 0 0 * 200 * 100 users:(("codex",pid=20,fd=38))\n'
              'u_str ESTAB 0 0 * 300 * 301 users:(("codex",pid=30,fd=38))\n'
              'u_str ESTAB 0 0 /run/other 301 * 300 users:(("other",pid=9,fd=3))\n')
        with patch.object(observer, 'output', return_value=ss):
            self.assertEqual(observer.clients_from_ss(7, {20, 30}), {20})
        with patch.object(observer, 'clients_from_lsof', side_effect=OSError), patch.object(observer.sys, 'platform', 'darwin'):
            self.assertIsNone(observer.daemon_clients(7, {20}))
        self.assertIsNone(observer.daemon_clients(None, {20}))
        self.assertEqual(observer.thread_born('01a20398-a56d-77c3-b1f7-a0293ac8e9c0'), 0x01a20398a56d)
        self.assertIsNone(observer.thread_born('thread-a'))
        self.assertIsNone(observer.thread_born('01a20398-a56d-47c3-b1f7-a0293ac8e9c0'))

    def test_new_agents_process_binding_conversations_children_and_deduplication(self):
        pane = {'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1}
        procs = {1: {'parent': 0, 'token': 'shell', 'agent': None}, 2: {'parent': 1, 'token': 'grok', 'agent': 'Grok'}}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.emit('Claude', 'Stop', {'session_id': 'root'})  # Claude compatibility file
            self.assertFalse(observer.DB.exists())
            self.emit('Grok', 'SessionStart', {'sessionId': 'root'})
            initial = observer.poll('/bin/tmux', '$1', 0, '')
            self.emit('Grok', 'Stop', {'sessionId': 'root', 'promptId': 'turn-1'}, explicit=False)
            self.emit('Grok', 'Stop', {'sessionId': 'root', 'promptId': 'turn-1'})
            self.emit('Grok', 'Notification', {'notificationType': 'task_complete'})
            self.emit('Grok', 'Notification', {'notificationType': 'idle_prompt'})
            self.emit('Grok', 'StopFailure', {'sessionId': 'child', 'subagentType': 'explore'})
            self.emit('Grok', 'SessionEnd', {'sessionId': 'child', 'subagentType': 'explore'})
            delta = observer.poll('/bin/tmux', '$1', initial['cursor'], initial['epoch'])
            self.assertEqual([e['kind'] for e in delta['events']], ['Response', 'Error', 'Ended'])
            parent = next(o for o in delta['observations'] if o['actor'] == 'parent')
            self.assertEqual(parent['last']['event'], 'Stop')
            self.assertNotIn('Error', parent['supported'])
            procs[2].update(agent='Agy', token='agy')
            base = {'conversationId': 'root', 'transcriptPath': '/home/.gemini/antigravity-cli/brain/root/logs/transcript.jsonl'}
            self.emit('Agy', 'SessionStart', dict(base, transcriptPath='/home/.gemini/antigravity/brain/root/logs/transcript.jsonl'))
            self.emit('Agy', 'Stop', dict(base, terminationReason='model_stop'))  # no SessionStart yet
            self.emit('Agy', 'SessionStart', base)
            self.emit('Agy', 'Stop', dict(base, conversationId='child', terminationReason='model_stop'))
            snapshot = observer.poll('/bin/tmux', '$1', 0, '')
            parent = next(o for o in snapshot['observations'] if o['actor'] == 'parent')
            self.assertEqual(parent['last']['event'], 'SessionStart')
            self.assertNotIn('Response', parent['supported'])
            # A nested other CLI must not walk past itself to an AGY ancestor.
            procs[3] = {'parent': 2, 'token': 'nested-grok', 'agent': 'Grok'}
            with patch.object(observer.os, 'getppid', return_value=3):
                self.emit('Agy', 'Stop', dict(base, terminationReason='model_stop'))
            self.assertEqual(observer.poll('/bin/tmux', '$1', 0, '')['cursor'], snapshot['cursor'])

    def test_legacy_event_argument_and_nested_native_runs(self):
        # Old record commands get a neutral response from the input event, too.
        for event, expected in [('PreToolUse', {'decision': 'allow'}), ('Stop', {'decision': ''})]:
            payload = {'hook_event_name': event, 'conversationId': 'root',
                       'transcriptPath': '/fixture/antigravity-cli/brain/root/transcript.jsonl'}
            stdin = type('Input', (), {'buffer': io.BytesIO(json.dumps(payload).encode())})()
            output = io.StringIO()
            with patch.object(observer.sys, 'stdin', stdin), patch.object(observer.sys, 'argv', ['observer.py', 'record', '--agent', 'Agy', '--tmux', '/bin/tmux']), patch.object(observer, 'topology', side_effect=ValueError), contextlib.redirect_stdout(output):
                observer.main()
            self.assertEqual(json.loads(output.getvalue()), expected)
        pane = {'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1}
        procs = {1: {'parent': 0, 'token': 'shell', 'agent': None},
                 2: {'parent': 1, 'token': 'native-root', 'agent': 'Grok'},
                 3: {'parent': 2, 'token': 'native-child', 'agent': 'Grok'}}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.emit('Grok', 'Stop', {})  # no session identity cannot become a root observation
            self.assertFalse(observer.DB.exists())
            snapshot = observer.poll('/bin/tmux', '$1', 0, '')
            self.assertEqual(len(snapshot['observations']), 2)
            procs[2]['launcher'] = True
            self.assertEqual(len(observer.poll('/bin/tmux', '$1', 0, '')['observations']), 1)

    def record(self, name, actor=None, tool=None):
        payload = {'hook_event_name': name, 'session_id': 'session', 'prompt': 'DO NOT STORE SECRET', 'tool_input': {'command': 'DO NOT STORE COMMAND'}, 'last_assistant_message': 'DO NOT STORE RESPONSE'}
        if actor:
            payload['agent_id'] = actor
        if tool:
            payload['tool_name'] = tool
        stdin = type('Input', (), {'buffer': io.BytesIO(json.dumps(payload).encode())})()
        with patch.object(observer.sys, 'stdin', stdin):
            observer.record('Claude', '/bin/tmux')

    def test_events_identity_privacy_resume_child_stop_retention_and_gap(self):
        pane = {'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1}
        procs = {1: {'parent': 0, 'token': 'shell', 'agent': None}, 2: {'parent': 1, 'token': 'cli-start', 'agent': 'Claude'}}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.record('UserPromptSubmit')
            self.record('PreToolUse', tool='AskUserQuestion')
            self.record('PreToolUse', actor='child', tool='Bash')
            self.record('SubagentStop', actor='child')
            self.record('Stop')
            self.record('PreToolUse', tool='Bash')
            initial = observer.poll('/bin/tmux', '$1', 0, '')
            self.assertEqual(initial['events'], [])
            parent = next(o for o in initial['observations'] if o['actor'] == 'parent')
            self.assertEqual(parent['last']['kind'], 'Activity')
            child = next(o for o in initial['observations'] if o['actor'] != 'parent')
            self.assertEqual(child['last']['kind'], 'Ended')
            self.record('PermissionRequest')
            self.record('PostToolUse')
            delta = observer.poll('/bin/tmux', '$1', initial['cursor'], initial['epoch'])
            self.assertEqual([e['kind'] for e in delta['events']], ['Approval', 'Activity'])
            # Process restart must not inherit observations or historical events.
            procs[2]['token'] = 'restarted'
            restarted = observer.poll('/bin/tmux', '$1', initial['cursor'], initial['epoch'])
            self.assertTrue(all(o['last'] is None for o in restarted['observations']))
            self.assertEqual(restarted['events'], [])
            with patch.object(observer, 'topology', return_value=('restarted-server', [pane], procs)):
                server_restart = observer.poll('/bin/tmux', '$1', initial['cursor'], initial['epoch'])
                self.assertTrue(all(o['last'] is None for o in server_restart['observations']))
                self.assertEqual(server_restart['events'], [])
            with patch.object(observer, 'LIMIT', 2):
                self.record('SubagentStop', actor='old-child')
                self.record('UserPromptSubmit')
                self.record('StopFailure')
                self.record('Stop')
            gap = observer.poll('/bin/tmux', '$1', 1, initial['epoch'])
            self.assertTrue(gap['gap'])
            self.assertEqual(gap['events'], [])
            def version(args):
                return 'codex-cli 0.163.0' if 'codex' in args[0] else 'Claude Code 2.1.310' if 'claude' in args[0] else 'tmux 3.3a'
            with patch.object(observer, 'run', side_effect=version), patch.object(observer.shutil, 'which', side_effect=lambda s: '/bin/' + s):
                preview = observer.inspect('/bin/tmux')
                observer.configure('/bin/tmux', preview['token'], False, '')
            removed = observer.poll('/bin/tmux', '$1', gap['cursor'], gap['epoch'])
            self.assertNotEqual(removed['epoch'], gap['epoch'])
            self.assertEqual(removed['events'], [])
            self.assertTrue(all(o['last'] is None for o in removed['observations']))
        with observer.database() as db:
            text = '\n'.join(row[0] for row in db.execute('SELECT data FROM events'))
            self.assertNotIn('DO NOT STORE', text)
            self.assertNotIn('tool_input', text)
            self.assertEqual(db.execute('SELECT count(*) FROM events').fetchone()[0], 2)
            self.assertLessEqual(db.execute('SELECT count(*) FROM latest').fetchone()[0], 2)
        self.assertEqual(observer.DB.stat().st_mode & 0o777, 0o600)


    # --- Detailed status collection -------------------------------------------------

    FIXTURE = SOURCE.parents[3] / 'fixtures/agent-hooks/status-payloads.json'

    def status_fixture(self):
        return json.loads(self.FIXTURE.read_text())

    def assertNoSecrets(self, value, fixture):
        encoded = json.dumps(value)
        for secret in fixture['secrets']:
            self.assertNotIn(secret, encoded)

    def assertSubset(self, expected, actual):
        for key, value in expected.items():
            if isinstance(value, dict):
                self.assertIsInstance(actual.get(key), dict, key)
                self.assertSubset(value, actual[key])
            else:
                self.assertEqual(actual.get(key), value, key)

    def test_status_whitelists_keep_figures_and_drop_sensitive_fields(self):
        fixture = self.status_fixture()
        now = 1763790000000
        for case in fixture['hooks']:
            with self.subTest(agent=case['agent'], event=case['event']):
                status = observer.status_from_hook(case['agent'], case['event'], case['payload'], now)
                self.assertNoSecrets(status, fixture)
                if not case['status']:
                    self.assertEqual(status, {})
                    continue
                self.assertSubset(case['status'], status)
                self.assertEqual(status['sources'], ['hook'])
        # Codex compaction requests and a null token_usage never become a cache sample.
        compaction = fixture['hooks'][5]
        self.assertNotIn('cache', observer.status_from_hook('Codex', 'PostModelRequest', compaction['payload'], now))
        self.assertEqual(observer.status_from_hook('Claude', 'Stop', {}, now), {'alert': None, 'sources': ['hook'], 'at': now})

        line = observer.status_from_statusline('Agy', fixture['statusline']['payload'], now)
        self.assertNoSecrets(line, fixture)
        self.assertEqual(line['model'], {'id': 'gemini-3.x-pro', 'name': 'Gemini 3.x Pro'})
        self.assertEqual(line['effort'], 'high')
        self.assertEqual(line['context'], {'percent': 42.5, 'size': 1000000, 'used': 84000})
        self.assertEqual({k: line['cache'][k] for k in ('input', 'read', 'write', 'output')},
                         {'input': 1200, 'read': 80000, 'write': 2800, 'output': 300})
        self.assertAlmostEqual(line['cache']['hit_ratio'], 80000 / 84000)
        self.assertNotIn('last_at', line['cache'])  # a statusline does not say when the request ended
        limits = {l['id']: l for l in line['limits']}
        self.assertEqual(sorted(limits), ['gemini-flash', 'gemini-pro'])
        self.assertEqual(limits['gemini-pro']['used_percent'], 63.0)
        self.assertEqual(limits['gemini-pro']['resets_at'], calendar.timegm((2026, 11, 22, 12, 30, 0)))
        self.assertNotIn('used_percent', limits['gemini-flash'])  # an amount is not a percentage
        self.assertEqual(limits['gemini-flash']['resets_at'], now // 1000 + 600)
        self.assertEqual(line['account']['plan'], 'pro')
        self.assertRegex(line['account']['key'], '^[0-9a-f]{64}$')
        self.assertEqual(line['cost_usd'], 1.25)
        self.assertEqual(sorted(line), ['account', 'at', 'cache', 'context', 'cost_usd', 'effort', 'limits', 'model', 'sources'])

        doc = dict(fixture['claude_mod'], at=now)
        mod = observer.status_from_claude_mod(doc, 4242, now)
        self.assertNoSecrets(mod, fixture)
        rows = mod['context']['breakdown']
        self.assertEqual([r['category'] for r in rows],
                         ['system_prompt', 'system_tools', 'memory_files', 'messages', 'other',
                          'autocompact_buffer', 'free', 'deferred'])
        self.assertEqual(rows[4]['name'], 'A new row')
        self.assertTrue(all(sorted(r) == ['category', 'kind', 'name', 'tokens'] for r in rows))
        self.assertEqual(mod['context']['breakdown_window'], 200000)
        self.assertEqual(mod['cache']['ttl'], '1h')
        self.assertEqual(mod['account']['kind'], 'subscription')
        self.assertRegex(mod['account']['key'], '^[0-9a-f]{64}$')
        self.assertEqual([l['id'] for l in mod['limits']], ['five_hour', 'seven_day'])
        self.assertEqual(mod['sources'], ['mod'])
        self.assertEqual(observer.status_from_claude_mod(doc, 4243, now), {})  # another process
        self.assertEqual(observer.status_from_claude_mod(dict(doc, at=now - 7 * 3600 * 1000), 4242, now), {})
        self.assertEqual(observer.status_from_claude_mod(dict(doc, v=2), 4242, now), {})

        grok = observer.status_from_grok_files(fixture['grok']['signals'], fixture['grok']['usage'], now)
        self.assertNoSecrets(grok, fixture)
        self.assertEqual(grok['context'], {'size': 200000, 'used': 84000, 'percent': 42})
        self.assertEqual(grok['model'], {'id': 'grok-code-x'})
        self.assertAlmostEqual(grok['cost_usd'], 1.23)
        self.assertEqual({k: grok['cache'][k] for k in ('input', 'read', 'write', 'output', 'last_at')},
                         {'input': 5000, 'read': 80000, 'write': 0, 'output': 900,
                          'last_at': calendar.timegm((2026, 11, 22, 10, 0, 0)) * 1000 + 123})
        # A signals file written before a measurement holds zeros, which are unknown.
        empty = observer.status_from_grok_files({'contextWindowUsage': 0, 'contextTokensUsed': 0, 'contextWindowTokens': 0}, None, now)
        self.assertEqual(empty, {})

    def test_status_bounds_and_merge_rules(self):
        bounded = observer.clean_status({'effort': 'x' * 33, 'model': {'id': 'm', 'name': 'n' * 65},
                                         'context': {'percent': 120, 'used': -1, 'size': 1.5},
                                         'cache': {'hit_ratio': 2, 'ttl': '2h'}, 'cost_usd': float('nan'),
                                         'limits': [{'id': str(i), 'used_percent': 1500} for i in range(20)],
                                         'alert': 'unknown', 'sources': ['mod', 'evil'], 'email': 'person@example.com'})
        self.assertEqual(bounded['model'], {'id': 'm'})
        self.assertNotIn('context', bounded)
        self.assertNotIn('cache', bounded)
        self.assertEqual(len(bounded['limits']), 16)
        self.assertTrue(all('used_percent' not in l for l in bounded['limits']))
        self.assertEqual(bounded['sources'], ['mod'])
        self.assertEqual(sorted(bounded), ['limits', 'model', 'sources'])
        self.assertEqual(observer.clean_status({'context': {'percent': 0}})['context'], {'percent': 0})

        first = observer.merge_status({}, {'model': {'id': 'a', 'name': 'A'}, 'effort': 'high', 'alert': 'rate_limit',
                                           'sources': ['statusline'], 'at': 1}, now=1)
        same = observer.merge_status(first, {'model': {'id': 'a'}, 'sources': ['hook'], 'at': 2}, now=2)
        self.assertEqual(same['model'], {'id': 'a', 'name': 'A'})
        self.assertEqual(same['effort'], 'high')  # a collector replaces only the keys it reports
        self.assertEqual(same['sources'], ['statusline', 'hook'])
        switched = observer.merge_status(same, {'model': {'id': 'b'}, 'alert': None, 'at': 3}, now=3)
        self.assertEqual(switched['model'], {'id': 'b'})
        self.assertNotIn('alert', switched)
        self.assertEqual(switched['_ts'], {'model': 3, 'effort': 1, 'alert': 3})
        # The newer sample of each key wins; the mod wins a tie.
        mod = {'model': {'id': 'b', 'name': 'B'}, 'effort': 'low', 'context': {'percent': 5}, 'sources': ['mod'], 'at': 2}
        wire = observer.wire_status(switched, mod)
        self.assertEqual(wire['model'], {'id': 'b', 'name': 'B'})
        self.assertEqual(wire['effort'], 'low')
        self.assertEqual(wire['context'], {'percent': 5})
        self.assertEqual(sorted(wire['sources']), ['hook', 'mod', 'statusline'])
        self.assertEqual(wire['at'], 3)
        newer = observer.merge_status(switched, {'effort': 'max', 'at': 9}, now=9)
        self.assertEqual(observer.wire_status(newer, mod)['effort'], 'max')
        self.assertNotIn('_ts', observer.wire_status(newer, None))
        self.assertEqual(observer.wire_status({'alert': 'rate_limit', '_ts': {}}, {})['alert'], 'rate_limit')
        self.assertEqual(observer.wire_status(None, None), {})

    def claude_pane(self, agent='Claude', pid=2):
        pane = {'session': '$1', 'window': '@1', 'pane': '%1', 'pid': 1}
        procs = {1: {'parent': 0, 'token': 'shell', 'agent': None}, pid: {'parent': 1, 'token': 'cli-' + agent, 'agent': agent}}
        return pane, procs

    def test_mod_samples_keep_their_own_model_and_limits_times(self):
        now = 1763790000000
        # A /model switch after the last turn end wins over a later measure-only flush.
        row = observer.merge_status({}, observer.status_from_hook('Claude', 'PostModelSwitch', {'to_model': 'claude-sonnet-x'}, now - 5000), now - 5000)
        doc = {'v': 1, 'pid': 42, 'at': now, 'model_at': now - 9000,
               'status': {'model': {'id': 'claude-opus-x'}, 'context': {'percent': 10}, 'limits': [{'id': 'five_hour', 'used_percent': 3}]}}
        mod = observer.status_from_claude_mod(doc, 42, now)
        wire = observer.wire_status(row, mod)
        self.assertEqual(wire['model'], {'id': 'claude-sonnet-x'})
        self.assertEqual(wire['context'], {'percent': 10})
        self.assertEqual(wire['limits_at'], now)
        # The next turn end samples the model again.
        doc['model_at'] = now
        self.assertEqual(observer.wire_status(row, observer.status_from_claude_mod(doc, 42, now))['model'], {'id': 'claude-opus-x'})
        # A later hook moves `at` but never the limits' own sample time.
        hook = observer.merge_status({}, {'effort': 'high', 'sources': ['hook'], 'at': now + 3000}, now + 3000)
        wire = observer.wire_status(hook, mod)
        self.assertEqual((wire['at'], wire['limits_at']), (now + 3000, now))
        self.assertEqual(wire['context_at'], now)
        self.assertNotIn('_ts', wire)
        # Without limits there is no limits time.
        self.assertNotIn('limits_at', observer.wire_status(hook, {}))

    def test_status_only_events_never_create_events_or_notifications(self):
        fixture = self.status_fixture()
        codex_home = self.root / 'codex'
        codex_home.mkdir()
        (codex_home / 'models_cache.json').write_text(json.dumps({'identity': 'SECRET', 'models': [
            {'slug': 'gpt-5.x-codex', 'context_window': 272000, 'effective_context_window_percent': 95},
            {'slug': 'other', 'context_window': 1}]}))
        for number, effort in ((3, 'low'), (5, 'xhigh')):
            with contextlib.closing(sqlite3.connect(codex_home / ('state_%d.sqlite' % number))) as db:
                db.execute('CREATE TABLE threads (id TEXT PRIMARY KEY, model TEXT, reasoning_effort TEXT, title TEXT)')
                db.execute('INSERT INTO threads VALUES (?,?,?,?)', ('sess-raw-id', 'gpt-5.x-codex', effort, 'SECRET'))
                db.commit()
        pane, procs = self.claude_pane('Codex')
        env = {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1', 'CODEX_HOME': str(codex_home)}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, env):
            self.emit('Codex', 'PostModelRequest', fixture['hooks'][4]['payload'], explicit=False)
            with observer.database() as db:
                self.assertEqual(db.execute('SELECT count(*) FROM events').fetchone()[0], 0)
                self.assertEqual(db.execute('SELECT count(*) FROM latest').fetchone()[0], 0)
                self.assertEqual(db.execute('SELECT count(*) FROM conversations').fetchone()[0], 0)
                stored = db.execute('SELECT data FROM status').fetchone()[0]
            self.assertNoSecrets(stored, fixture)
            snapshot = observer.poll('/bin/tmux', '', 0, '')
            obs = snapshot['observations'][0]
            self.assertIsNone(obs['last'])
            self.assertEqual(obs['supported'], [])
            self.assertEqual(snapshot['events'], [])
            status = obs['status']
            self.assertEqual(status['context'], {'used': 50900, 'size': 258400, 'percent': 19.7})
            self.assertEqual(status['effort'], 'xhigh')  # the highest-numbered state database
            self.assertEqual(status['cache']['read'], 46000)
            self.assertNotIn('pid', obs)
            self.assertNotIn('_ts', status)
            self.assertNoSecrets(snapshot, fixture)
            # A regular hook still records its event and keeps the status.
            self.emit('Codex', 'Stop', {'session_id': 'sess-raw-id', 'model': 'gpt-5.x-codex'}, explicit=False)
            after = observer.poll('/bin/tmux', '', 0, '')['observations'][0]
            self.assertEqual(after['last']['event'], 'Stop')
            self.assertEqual(after['status']['cache']['read'], 46000)
        pane, procs = self.claude_pane('Claude')
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}):
            self.emit('Claude', 'PostModelSwitch', fixture['hooks'][1]['payload'], explicit=False)
            self.emit('Claude', 'PreToolUse', fixture['hooks'][0]['payload'], explicit=False)
            self.emit('Claude', 'StopFailure', fixture['hooks'][2]['payload'], explicit=False)
            obs = observer.poll('/bin/tmux', '', 0, '')['observations'][0]
            self.assertEqual(obs['status']['model'], {'id': 'claude-opus-4-x'})
            self.assertEqual(obs['status']['effort'], 'high')
            self.assertEqual(obs['status']['alert'], 'rate_limit')
            self.assertEqual(obs['supported'], ['Activity', 'Error'])
            self.emit('Claude', 'Stop', {'session_id': 'sess-raw-id'}, explicit=False)
            self.assertNotIn('alert', observer.poll('/bin/tmux', '', 0, '')['observations'][0]['status'])
        # Removal clears status rows with the rest of the live state.
        with patch.object(observer, 'run', return_value='tmux 3.3a'), patch.object(observer.shutil, 'which', return_value=None):
            observer.configure('/bin/tmux', observer.inspect('/bin/tmux')['token'], False, '')
        with observer.database() as db:
            self.assertEqual(db.execute('SELECT count(*) FROM status').fetchone()[0], 0)

    def test_status_rows_expire_after_a_day_and_old_databases_poll(self):
        with observer.database() as db:
            observer.upsert_status(db, 'old', 'parent', 'Codex', {'effort': 'low', 'at': 1}, now=1)
            observer.upsert_status(db, 'new', 'parent', 'Codex', {'effort': 'low'}, now=observer.STATUS_RETENTION_MS + 2)
            self.assertEqual([r[0] for r in db.execute('SELECT run FROM status')], ['new'])
            observer.upsert_status(db, 'empty', 'parent', 'Claude', {'alert': None}, now=5)
            self.assertIsNone(db.execute("SELECT 1 FROM status WHERE run='empty'").fetchone())
            db.execute('DROP TABLE status')
        pane, procs = self.claude_pane('Codex')
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)):
            snapshot = observer.poll('/bin/tmux', '', 0, '')
        self.assertNotIn('status', snapshot['observations'][0])

    def statusline_input(self, payload_bytes):
        return type('Input', (), {'buffer': io.BytesIO(payload_bytes)})()

    def test_agy_statusline_tee_is_byte_transparent_and_records_a_whitelist(self):
        fixture = self.status_fixture()
        raw = json.dumps(fixture['statusline']['payload']).encode() + b'\n\xff'
        observer.ROOT.mkdir(parents=True)
        pane, procs = self.claude_pane('Agy')
        env = {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1'}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, env):
            # No original: nothing printed, success, still recorded.
            with patch.object(observer.sys, 'stdin', self.statusline_input(json.dumps(fixture['statusline']['payload']).encode())):
                self.assertEqual(observer.statusline('Agy', '/bin/tmux'), (0, b''))
            obs = observer.poll('/bin/tmux', '', 0, '')['observations'][0]
            self.assertEqual(obs['status']['sources'], ['statusline'])
            self.assertEqual(obs['status']['context']['percent'], 42.5)
            self.assertIsNone(obs['last'])
            self.assertNoSecrets(obs, fixture)
            # The saved original runs with the same bytes; its stdout and exit code are relayed.
            original = {'type': 'command', 'command': "cat; printf '\\377'; echo \"$DGL_FIXTURE_ENV\"; exit 3", 'padding': 1}
            observer.saved_path('Agy').write_text(json.dumps(original))
            with patch.object(observer.sys, 'stdin', self.statusline_input(raw)), patch.dict(os.environ, {'DGL_FIXTURE_ENV': 'kept'}):
                code, out = observer.statusline('Agy', '/bin/tmux')
            self.assertEqual((code, out), (3, raw + b'\xffkept\n'))
            # Through main(): raw bytes only, the exit code relayed, never an error object.
            stdout = type('Output', (), {'buffer': io.BytesIO(), 'flush': lambda self: None, 'write': lambda self, s: None})()
            with patch.object(observer.sys, 'stdin', self.statusline_input(b'not json')), patch.object(observer.sys, 'stdout', stdout), \
                    patch.object(observer.sys, 'argv', ['observer.py', 'statusline', '--agent', 'Agy', '--tmux', '/bin/tmux']):
                with self.assertRaises(SystemExit) as exit_code:
                    observer.main()
            self.assertEqual(exit_code.exception.code, 3)
            self.assertEqual(stdout.buffer.getvalue(), b'not json\xff\n')
            # A disabled or self-referencing original is not run.
            for value in ({'type': 'command', 'command': 'echo hidden', 'enabled': False},
                          {'type': 'command', 'command': observer.tee_command('/bin/tmux')}, None):
                observer.saved_path('Agy').write_text(json.dumps(value))
                with patch.object(observer.sys, 'stdin', self.statusline_input(raw)):
                    self.assertEqual(observer.statusline('Agy', '/bin/tmux'), (0, b''))
            # Oversized input is relayed whole but never parsed or recorded.
            observer.saved_path('Agy').write_text(json.dumps({'type': 'command', 'command': 'wc -c'}))
            big = b'{' + b' ' * (observer.INPUT_LIMIT + 10) + b'}'
            with patch.object(observer, 'record_statusline', side_effect=AssertionError('parsed')), \
                    patch.object(observer.sys, 'stdin', self.statusline_input(big)):
                code, out = observer.statusline('Agy', '/bin/tmux')
            self.assertEqual((code, int(out.split()[0])), (0, len(big)))
            # A failing recorder never fails the statusline.
            with patch.object(observer, 'topology', side_effect=OSError('secret')), patch.object(observer.sys, 'stdin', self.statusline_input(raw)):
                self.assertEqual(observer.statusline('Agy', '/bin/tmux')[0], 0)
            # Other agents are not wrapped.
            with patch.object(observer.sys, 'stdin', self.statusline_input(raw)):
                self.assertEqual(observer.statusline('Claude', '/bin/tmux'), (0, b''))

    def test_agy_statusline_tee_keeps_its_deadline_and_exit_code_with_background_jobs(self):
        observer.ROOT.mkdir(parents=True)
        marker = self.root / 'grandchild.pid'
        cases = [
            # A slow original is killed with its whole process group at the deadline.
            ('echo hi; sleep 30 & echo $! > %s; wait; true' % shlex.quote(str(marker)), 124, b'hi\n'),
            # A finished original keeps its own exit code although a background job holds stdout.
            ('(sleep 30 &); echo ok; exit 5', 5, b'ok\n'),
        ]
        with patch.object(observer, 'TEE_TIMEOUT', 1.0), patch.object(observer, 'record_statusline'):
            for command, code, out in cases:
                with self.subTest(command=command):
                    observer.saved_path('Agy').write_text(json.dumps({'type': 'command', 'command': command}))
                    started = time.monotonic()
                    with patch.object(observer.sys, 'stdin', self.statusline_input(b'{}')):
                        self.assertEqual(observer.statusline('Agy', '/bin/tmux'), (code, out))
                    self.assertLess(time.monotonic() - started, 2.0)
        pid = int(marker.read_text())
        for _ in range(50):
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                break
            time.sleep(0.02)
        else:
            self.fail('the timed-out original left its background job running')

    def versions(self, codex='0.171.0', claude='2.1.330'):
        def version(args):
            name = Path(args[0]).name
            return {'codex': 'codex-cli ' + codex, 'claude': claude + ' (Claude Code)', 'agy': 'agy 1.5.13',
                    'grok': 'grok 1.0.45'}.get(name, 'tmux 3.3a')
        return patch.object(observer, 'run', side_effect=version), patch.object(observer.shutil, 'which', side_effect=lambda s: '/bin/' + s)

    def configure(self, install, status=False):
        return observer.configure('/bin/tmux', observer.inspect('/bin/tmux')['token'], install, SOURCE.read_text(), status)

    def test_status_install_remove_restores_settings_and_detects_replacement(self):
        claude_path, agy_path = self.root / 'Claude.json', self.root / 'agy-settings.json'
        claude_original = {'model': 'opus', 'env': {'CLAUDE_CODE_PLUGIN_DIRS': '/opt/user-mod', 'OTHER': '1'}}
        agy_original = {'theme': 'dark', 'statusLine': {'type': 'command', 'command': 'echo mine', 'padding': 1, 'stack_with_default': False}}
        claude_path.write_text(json.dumps(claude_original))
        agy_path.write_text(json.dumps(agy_original))
        folder = str(observer.mod_dir())
        run, which = self.versions()
        with run, which:
            preview = {c['agent']: c for c in observer.inspect('/bin/tmux')['configs']}
            self.assertEqual(preview['Claude']['status_collection'], {'supported': True, 'installed': False, 'replaced': False, 'kind': 'mod'})
            self.assertEqual(preview['Agy']['status_collection'], {'supported': True, 'installed': False, 'replaced': False, 'kind': 'statusline'})
            self.assertEqual(preview['Codex']['status_collection'], {'supported': False, 'installed': False, 'replaced': False, 'kind': 'hooks'})
            self.assertEqual(preview['Grok']['status_collection']['kind'], 'files')
            installed = {c['agent']: c for c in self.configure(True, status=True)['configs']}
            self.assertTrue(installed['Claude']['status_collection']['installed'])
            self.assertTrue(installed['Agy']['status_collection']['installed'])
            settings = json.loads(claude_path.read_text())
            self.assertEqual(settings['env'], {'CLAUDE_CODE_PLUGIN_DIRS': '/opt/user-mod:' + folder, 'OTHER': '1'})
            self.assertIn('PostModelSwitch', settings['hooks'])
            source = (observer.mod_dir() / 'hooks/register.ts').read_text()
            self.assertIn(json.dumps(str(observer.ROOT / 'claude')), source)
            self.assertNotIn('__DGL_DIR__', source)
            self.assertEqual(json.loads((observer.mod_dir() / 'hooks/hooks.json').read_text()), {'modules': ['./register.ts']})
            line = json.loads(agy_path.read_text())['statusLine']
            self.assertTrue(observer.owned_tee(line))
            self.assertEqual((line['padding'], line['stack_with_default']), (1, False))
            self.assertEqual(json.loads(observer.saved_path('Agy').read_text()), agy_original['statusLine'])
            self.assertEqual(observer.saved_path('Agy').stat().st_mode & 0o777, 0o600)
            self.assertEqual(json.loads(agy_path.read_text())['theme'], 'dark')
            # Reinstalling is idempotent.
            again = self.configure(True, status=True)
            self.assertEqual(json.loads(claude_path.read_text())['env'], settings['env'])
            self.assertTrue(all(c['status_collection']['installed'] for c in again['configs'] if c['agent'] in ('Claude', 'Agy')))
            # Install without --status removes only the status parts and restores the originals.
            plain = {c['agent']: c for c in self.configure(True)['configs']}
            self.assertTrue(plain['Claude']['installed'])
            self.assertFalse(plain['Claude']['status_collection']['installed'])
            restored = json.loads(claude_path.read_text())
            self.assertEqual(restored['env'], claude_original['env'])
            self.assertEqual(json.loads(agy_path.read_text()), agy_original)
            self.assertFalse(observer.saved_path('Agy').exists())
            self.assertFalse(observer.saved_path('Claude').exists())
            self.assertFalse(observer.mod_dir().exists())
            # A user who replaces our entries is never re-wrapped.
            self.configure(True, status=True)
            user = json.loads(agy_path.read_text())
            user['statusLine'] = {'type': 'command', 'command': 'echo new'}
            agy_path.write_text(json.dumps(user))
            claude = json.loads(claude_path.read_text())
            claude['env']['CLAUDE_CODE_PLUGIN_DIRS'] = '/opt/user-mod'
            claude_path.write_text(json.dumps(claude))
            replaced = {c['agent']: c['status_collection'] for c in observer.inspect('/bin/tmux')['configs']}
            self.assertEqual((replaced['Agy']['replaced'], replaced['Agy']['installed']), (True, False))
            self.assertEqual((replaced['Claude']['replaced'], replaced['Claude']['installed']), (True, False))
            self.configure(True, status=True)
            self.assertEqual(json.loads(agy_path.read_text())['statusLine'], user['statusLine'])
            self.assertEqual(json.loads(claude_path.read_text())['env']['CLAUDE_CODE_PLUGIN_DIRS'], '/opt/user-mod')
            removed = {c['agent']: c for c in self.configure(False)['configs']}
            self.assertFalse(any(c['installed'] or c['status_collection']['replaced'] for c in removed.values()))
            self.assertEqual(json.loads(agy_path.read_text())['statusLine'], user['statusLine'])
            self.assertEqual(json.loads(claude_path.read_text()), dict(claude_original, env=claude['env']))
            self.assertFalse(observer.saved_path('Agy').exists())

    def test_a_failed_status_install_keeps_no_saved_original_and_can_be_retried(self):
        claude_path, agy_path = self.root / 'Claude.json', self.root / 'agy-settings.json'
        run, which = self.versions()
        real = observer.atomic_write
        for failing in ('Agy', 'Claude'):
            with self.subTest(failing=failing), run, which:
                claude_path.write_text('{}')
                agy_path.write_text(json.dumps({'statusLine': {'type': 'command', 'command': 'echo mine'}}))
                target = agy_path if failing == 'Agy' else claude_path

                def flaky(path, raw, expected):
                    if path == target:
                        raise ValueError('configuration changed; inspect again')  # a concurrent writer
                    return real(path, raw, expected)
                with patch.object(observer, 'atomic_write', side_effect=flaky):
                    with self.assertRaisesRegex(ValueError, 'configuration changed'):
                        self.configure(True, status=True)
                self.assertFalse(observer.saved_path(failing).exists())
                if failing == 'Claude':
                    self.assertFalse(observer.mod_dir().exists())
                state = {c['agent']: c['status_collection'] for c in observer.inspect('/bin/tmux')['configs']}
                self.assertEqual((state[failing]['installed'], state[failing]['replaced']), (False, False))
                retried = {c['agent']: c['status_collection'] for c in self.configure(True, status=True)['configs']}
                self.assertTrue(retried[failing]['installed'])
                self.configure(False)

    def test_status_install_without_prior_settings_restores_absent_keys(self):
        claude_path, agy_path = self.root / 'Claude.json', self.root / 'agy-settings.json'
        cases = [({}, {}), ({'env': {}}, {'statusLine': None}), ({'env': {'CLAUDE_CODE_PLUGIN_DIRS': ''}}, {'theme': 'x'})]
        run, which = self.versions()
        with run, which:
            for claude_original, agy_original in cases:
                with self.subTest(claude=claude_original, agy=agy_original):
                    claude_path.write_text(json.dumps(claude_original))
                    agy_path.write_text(json.dumps(agy_original))
                    self.configure(True, status=True)
                    claude = json.loads(claude_path.read_text())
                    self.assertEqual(claude['env']['CLAUDE_CODE_PLUGIN_DIRS'], str(observer.mod_dir()))
                    line = json.loads(agy_path.read_text())['statusLine']
                    self.assertTrue(line['stack_with_default'])
                    self.assertEqual(sorted(line), ['command', 'stack_with_default', 'type'])
                    self.configure(False)
                    self.assertEqual(json.loads(claude_path.read_text()), claude_original)
                    expected = dict(agy_original)
                    if expected.get('statusLine', 0) is None:
                        expected.pop('statusLine')  # a null statusLine is restored as absent, the same setting
                    self.assertEqual(json.loads(agy_path.read_text()), expected)
            # Unsupported or unreadable settings are never rewritten and do not block hooks.
            agy_path.write_text('{broken')
            claude_path.write_text(json.dumps({'env': 'not an object'}))
            preview = {c['agent']: c for c in observer.inspect('/bin/tmux')['configs']}
            self.assertFalse(preview['Agy']['status_collection']['supported'])
            self.assertFalse(preview['Claude']['status_collection']['supported'])
            result = {c['agent']: c for c in self.configure(True, status=True)['configs']}
            self.assertTrue(result['Agy']['installed'])
            self.assertEqual(agy_path.read_text(), '{broken')
            self.assertEqual(json.loads(claude_path.read_text())['env'], 'not an object')
        with patch.object(observer, 'run', return_value='0.1.0'), patch.object(observer.shutil, 'which', return_value=None):
            preview = {c['agent']: c for c in observer.inspect('/bin/tmux')['configs']}
            self.assertFalse(any(c['status_collection']['supported'] for c in preview.values()))

    def test_codex_post_model_request_is_installed_only_from_0_171(self):
        for version, expected in (('0.170.9', False), ('0.171.0', True), ('0.172.1', True), ('0.171.0-alpha.1', False)):
            run, which = self.versions(codex=version)
            with run, which:
                configs = {c['agent']: c for c in observer.inspect('/bin/tmux')['configs']}
                if version.endswith('alpha.1'):
                    self.assertFalse(configs['Codex']['supported'])
                    continue
                self.assertEqual('PostModelRequest' in configs['Codex']['events'], expected, version)
                self.assertIn('PostModelSwitch', configs['Claude']['events'])
                self.configure(True)
                hooks = json.loads((self.root / 'Codex.json').read_text())['hooks']
                self.assertEqual('PostModelRequest' in hooks, expected, version)
                self.assertEqual(len(hooks['SessionStart']), 1)
        self.assertNotIn('PostModelRequest', observer.hook_events('Codex'))
        self.assertIn('PostModelRequest', observer.hook_events('Codex', 'codex-cli 0.171.0'))

    def test_grok_session_files_feed_status_on_its_hooks(self):
        fixture = self.status_fixture()
        grok_home = self.root / 'grok'
        session = grok_home / 'sessions/cwd-group/root-session'
        session.mkdir(parents=True)
        (session / 'signals.json').write_text(json.dumps(fixture['grok']['signals']))
        (session / 'usage.json').write_text(json.dumps(fixture['grok']['usage']))
        pane, procs = self.claude_pane('Grok')
        env = {'TMUX': '/socket,10,0', 'TMUX_PANE': '%1', 'GROK_HOME': str(grok_home)}
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)), patch.object(observer.os, 'getppid', return_value=2), patch.dict(os.environ, env):
            self.emit('Grok', 'SessionStart', {'sessionId': 'root-session'})
            self.emit('Grok', 'StopFailure', {'sessionId': 'root-session', 'errorType': 'rate_limit', 'errorDetails': 'SECRET'})
            obs = observer.poll('/bin/tmux', '', 0, '')['observations'][0]
            self.assertEqual(obs['status']['context']['percent'], 42)
            self.assertAlmostEqual(obs['status']['cost_usd'], 1.23)
            self.assertEqual(obs['status']['alert'], 'rate_limit')
            self.assertEqual(sorted(obs['status']['sources']), ['file', 'hook'])
            self.assertNoSecrets(obs, fixture)
            self.assertEqual(observer.grok_files('../root-session'), (None, None))
            self.assertEqual(observer.grok_files('*'), (None, None))
            self.assertEqual(observer.grok_files('missing'), (None, None))
            (session / 'signals.json').write_bytes(b'{' + b' ' * (256 * 1024) + b'}')
            self.assertIsNone(observer.grok_files('root-session')[0])

    def test_claude_mod_file_maps_to_its_pane_by_pid(self):
        fixture = self.status_fixture()
        pane, procs = self.claude_pane('Claude', pid=4242)
        folder = observer.ROOT / 'claude'
        folder.mkdir(parents=True)
        now = int(time.time() * 1000)
        def write(doc):
            (folder / '4242.json').write_text(json.dumps(doc) if isinstance(doc, dict) else doc)
        with patch.object(observer, 'topology', return_value=('server', [pane], procs)):
            self.assertFalse(observer.DB.exists())
            write(dict(fixture['claude_mod'], at=now))
            obs = observer.poll('/bin/tmux', '', 0, '')['observations'][0]
            self.assertEqual(obs['status']['sources'], ['mod'])
            self.assertEqual(obs['status']['context']['percent'], 42)
            self.assertNotIn('pid', obs)
            self.assertNoSecrets(obs, fixture)
            self.assertFalse(observer.DB.exists())  # polling stays read-only
            for doc in (dict(fixture['claude_mod'], at=now - 7 * 3600 * 1000), dict(fixture['claude_mod'], at=now, pid=1),
                        '{"v": 1, "pid": 4242, "at": ', '[]'):
                write(doc)
                self.assertNotIn('status', observer.poll('/bin/tmux', '', 0, '')['observations'][0])
            # A file older than the process belongs to a recycled pid.
            write(dict(fixture['claude_mod'], at=now - 3600 * 1000))
            procs[4242]['started'] = now // 1000
            self.assertNotIn('status', observer.poll('/bin/tmux', '', 0, '')['observations'][0])
            procs[4242]['started'] = None
            self.assertIn('status', observer.poll('/bin/tmux', '', 0, '')['observations'][0])
            # Another CLI's pane never reads a Claude file.
            procs[4242]['agent'] = 'Codex'
            self.assertNotIn('status', observer.poll('/bin/tmux', '', 0, '')['observations'][0])
            procs[4242]['agent'] = 'Claude'
            (folder / '4242.json').unlink()
            (folder / '4242.json').symlink_to(self.root / 'elsewhere.json')
            (self.root / 'elsewhere.json').write_text(json.dumps(dict(fixture['claude_mod'], at=now)))
            self.assertNotIn('status', observer.poll('/bin/tmux', '', 0, '')['observations'][0])

    def test_generated_claude_mod_bakes_the_helper_root_and_stays_private(self):
        files = observer.mod_files()
        self.assertEqual(sorted(files), ['.claude-plugin/plugin.json', 'hooks/hooks.json', 'hooks/register.ts'])
        source = files['hooks/register.ts'].decode()
        self.assertIn('const DIR: string = ' + json.dumps(str(observer.ROOT / 'claude')), source)
        self.assertEqual(json.loads(files['.claude-plugin/plugin.json'])['name'], observer.MOD_NAME)
        # The mod hooks observation events only and never reads identity fields it must not keep.
        self.assertEqual(sorted(set(__import__('re').findall(r"on\('([a-z.]+)'", source))),
                         ['session.end', 'session.measure', 'turn.complete'])
        for forbidden in ('email', 'memoryFiles', 'mcpTools', 'transcript', 'messages(', 'import('):
            self.assertNotIn(forbidden, source)

    @unittest.skipUnless(shutil.which('claude'), 'claude CLI is not on PATH')
    def test_generated_claude_mod_passes_claude_plugin_validate(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / observer.MOD_NAME
            for name, raw in observer.mod_files().items():
                (target / name).parent.mkdir(parents=True, exist_ok=True)
                (target / name).write_bytes(raw)
            result = subprocess.run(['claude', 'plugin', 'validate', str(target)], stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=120)
            output = result.stdout.decode('utf-8', 'replace')
            self.assertEqual(result.returncode, 0, output)
            self.assertIn('Validation passed', output)
            self.assertNotIn('error TS', output)

    @unittest.skipUnless(node_strips_types(), 'node with TypeScript type stripping is not on PATH')
    def test_generated_claude_mod_falls_back_to_a_subscription_when_identity_is_refused(self):
        harness = r"""
const [, , modPath, identity, limits] = process.argv
const { register } = await import(modPath)
const handlers = {}
register((name, fn) => { handlers[name] = fn })
const writes = []
const $ = {
  process: { hookEnv: async () => ({ CLAUDE_PID: '42' }) },
  clock: { now: async () => 1763790000000, after: () => undefined },
  session: { id: async () => 'session', identity: async () => JSON.parse(identity) },
  fs: { write: async (path, text) => { writes.push(JSON.parse(text)) } },
}
await handlers['session.measure']($, { context: { tokens: 1000, window: 200000, percent: 0.5 },
  rateLimits: JSON.parse(limits), changed: [] }, async () => undefined)
process.stdout.write(JSON.stringify(writes))
"""
        limit = [{'kind': 'five_hour', 'percentUsed': 12, 'resetsAt': '2026-11-22T10:00:00Z'}]
        cases = [({'deny': 'user tier'}, limit, 'subscription'), ({'kind': 'none', 'isVerified': False}, limit, 'subscription'),
                 ({'kind': 'console', 'isVerified': False}, limit, 'api_key'), ({'deny': 'user tier'}, [], None)]
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'register.ts').write_bytes(observer.mod_files()['hooks/register.ts'])
            (Path(folder) / 'run.mjs').write_text(harness)
            for identity, limits, kind in cases:
                with self.subTest(identity=identity, limits=bool(limits)):
                    result = subprocess.run(['node', 'run.mjs', './register.ts', json.dumps(identity), json.dumps(limits)],
                                            cwd=folder, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
                    self.assertEqual(result.returncode, 0, result.stdout.decode('utf-8', 'replace'))
                    [doc] = json.loads(result.stdout)
                    status = observer.status_from_claude_mod(doc, 42, doc['at'])
                    self.assertEqual((status or {}).get('account', {}).get('kind'), kind)

if __name__ == '__main__':
    unittest.main()
