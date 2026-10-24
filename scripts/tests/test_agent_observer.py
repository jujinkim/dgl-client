import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[2] / 'crates/dgl-core/agent/observer.py'
spec = importlib.util.spec_from_file_location('observer', SOURCE)
observer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer)


class AgentObserverTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.patches = [patch.object(observer, 'ROOT', self.root / 'dgl'),
                        patch.object(observer, 'HELPER', self.root / 'dgl/observer.py'),
                        patch.object(observer, 'DB', self.root / 'dgl/events.sqlite'),
                        patch.object(observer, 'config_paths', return_value={a: self.root / (a + '.json') for a in observer.EVENTS})]
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
        with patch.object(observer, 'run', return_value='999.0.0'), patch.object(observer.shutil, 'which', return_value='/bin/unknown'):
            preview = observer.inspect('/bin/tmux')
            self.assertFalse(any(c['supported'] for c in preview['configs']))
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

    def test_process_evidence_does_not_match_prompt_or_generic_node(self):
        start = 'Fri Oct 23 12:00:00 2026'
        rows = [f'1 0 {start} /bin/bash bash', f'2 1 {start} /usr/bin/node node arbitrary.js codex',
                f'3 1 {start} /opt/bin/codex /opt/bin/codex',
                f'4 1 {start} /usr/bin/node node /npm/@anthropic-ai/claude-code/cli.js',
                f'5 1 {start} /usr/bin/node node --eval "codex"',
                f'6 1 {start} /bin/echo echo /npm/@openai/codex/bin/codex.js']
        def ps(args):
            if args[-1] == 'pid=,args=':
                return '\n'.join(r.split(None, 1)[0] + ' ' + r.split(None, 8)[8] for r in rows)
            return '\n'.join(' '.join(r.split(None, 8)[:8]) for r in rows)
        with patch.object(observer, 'run', side_effect=ps):
            procs = observer.processes()
        self.assertEqual([pid for pid, p in procs.items() if p['agent']], [3, 4])

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


if __name__ == '__main__':
    unittest.main()
