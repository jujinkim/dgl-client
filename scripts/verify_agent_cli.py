#!/usr/bin/env python3
"""Real CLI hooks in an isolated HOME and private tmux socket. No real user hooks change.

Copies CLI auth into a mode-0700 temporary directory, deletes it on exit. Captures
only hook event names in the report; no transcript, credentials or prompt output.
Codex remains subject to its ordinary /hooks trust review (never bypassed).
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import sqlite3
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'crates/dgl-core/agent/observer.py'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--agent', choices=['Claude', 'Codex'], required=True)
    parser.add_argument('--interactive', action='store_true', help='Keep private Codex TUI open for explicit /hooks review via tmux; accepts terminal keys on stdin')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    real_home = Path.home()
    binary = shutil.which(args.agent.lower())
    tmux = shutil.which('tmux')
    if not binary or not tmux:
        raise SystemExit('CLI and tmux are required')
    version = subprocess.check_output([binary, '--version'], text=True).strip()
    with tempfile.TemporaryDirectory(prefix='dgl-cli-') as folder:
        root = Path(folder)
        os.chmod(root, 0o700)
        home = root / 'home'
        home.mkdir(mode=0o700)
        for cli, auth in [('codex', 'auth.json'), ('claude', '.credentials.json')]:
            target = home / ('.' + cli)
            target.mkdir(mode=0o700)
            source = real_home / ('.' + cli) / auth
            if source.exists():
                shutil.copyfile(source, target / auth)
                (target / auth).chmod(0o600)
        (home / '.claude.json').write_text(json.dumps({'hasCompletedOnboarding': True}))
        wrapper = root / 'tmux'
        wrapper.write_text('#!/bin/sh\nexec ' + shlex.join([tmux, '-S', str(root / 'tmux.socket')]) + ' "$@"\n')
        wrapper.chmod(0o700)
        installed = home / '.local/share/dgl-agent/v1/observer.py'
        installed.parent.mkdir(mode=0o700, parents=True)
        shutil.copyfile(HELPER, installed)
        spec = importlib.util.spec_from_file_location('observer', HELPER)
        observer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(observer)
        observer.HELPER = installed
        config = observer.transformed({}, args.agent, str(wrapper), True)
        path = home / ('.codex/hooks.json' if args.agent == 'Codex' else '.claude/settings.json')
        path.write_text(json.dumps(config))
        env = dict(os.environ, HOME=str(home), CODEX_HOME=str(home / '.codex'), CLAUDE_CONFIG_DIR=str(home / '.claude'))
        # Avoid inheriting parent-session launch guards without changing any running CLI.
        env.pop('CLAUDECODE', None)
        env.pop('TMUX', None)
        prompt = 'Use the shell tool to run printf DGL_HOOK_PROBE, then reply DGL_OK.'
        cli = [binary] if args.interactive else ([binary, '-p', prompt, '--max-turns', '3', '--allowedTools', 'Bash'] if args.agent == 'Claude' else [binary, 'exec', '--skip-git-repo-check', prompt])
        launcher = root / 'launch.sh'
        launcher.write_text('#!/bin/sh\ncd ' + shlex.quote(str(root)) + '\n' + shlex.join(cli) + '\nprintf "\\nDGL_EXIT=%s\\n" "$?"\nsleep 300\n')
        launcher.chmod(0o700)
        def tm(*params):
            return subprocess.check_output([str(wrapper), *params], env=env, text=True, stderr=subprocess.DEVNULL)
        try:
            tm('new-session', '-d', '-x', '120', '-y', '45', '-s', 'probe', str(launcher))
            if args.interactive:
                print('Private tmux wrapper: ' + str(wrapper), flush=True)
                print('Review only these temporary hooks in /hooks. Enter tmux keys as JSON arrays; ["done"] ends the probe.', flush=True)
                while True:
                    line = input()
                    keys = json.loads(line)
                    if keys == ['done']:
                        break
                    if keys == ['screen']:
                        print(tm('capture-pane', '-p', '-t', 'probe'), flush=True)
                    else:
                        tm('send-keys', '-t', 'probe', *keys)
            else:
                deadline = time.monotonic() + 90
                while time.monotonic() < deadline:
                    if 'DGL_EXIT=' in tm('capture-pane', '-p', '-t', 'probe'):
                        break
                    time.sleep(1)
            database = installed.parent / 'events.sqlite'
            events = []
            if database.exists():
                with sqlite3.connect(database) as db:
                    records = [json.loads(row[0]) for row in db.execute('SELECT data FROM events ORDER BY seq')]
                    events = [r['event'] for r in records]
            screen = tm('capture-pane', '-p', '-t', 'probe')
            report = {'agent': args.agent, 'version': version, 'events': events,
                      'observedKinds': sorted(set(r['kind'] for r in records)) if events else [],
                      'subagentEvents': [r['event'] for r in records if r['actor'] != 'parent'] if events else [],
                      'replyObserved': 'DGL_OK' in screen, 'hookTrustBypassed': False,
                      'isolatedHome': True, 'privateTmux': True}
            if args.report:
                args.report.parent.mkdir(parents=True, exist_ok=True)
                args.report.write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps(report), flush=True)
        finally:
            subprocess.run([str(wrapper), 'kill-server'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == '__main__':
    main()
