#!/usr/bin/env python3
"""dgl's opt-in observer. No transcripts, prompt text, tool inputs or outputs are persisted.

Also runs from stdin/-c for read-only discovery before installation. Python 3.9+,
POSIX tmux hosts; the SSH client itself may run on Windows, macOS or Android.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid

VERSION = 1
LIMIT = 2048
ROOT = Path.home() / '.local/share/dgl-agent/v1'
HELPER = ROOT / 'observer.py'
DB = ROOT / 'events.sqlite'
EVENTS = {
    'Codex': ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse',
              'PermissionRequest', 'Stop', 'SubagentStart', 'SubagentStop', 'SessionEnd', 'Interrupt'],
    'Claude': ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse',
               'PermissionRequest', 'Stop', 'SubagentStart', 'SubagentStop', 'SessionEnd',
               'PostToolUseFailure', 'StopFailure'],
    'Agy': ['SessionStart', 'PreToolUse', 'PostToolUse', 'PreInvocation', 'PostInvocation', 'Stop'],
    'Grok': ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse',
             'PostToolUseFailure', 'PermissionDenied', 'Stop', 'StopFailure', 'StopCancelled',
             'Notification', 'SubagentStart', 'SubagentStop', 'SessionEnd'],
}
# Conservative release gate: unknown releases need revalidation, not optimistic support.
VERSIONS = {'Codex': ('0.163.0', '0.164.0'), 'Claude': ('2.1.310',)}
CAPABILITIES = {a: ['Activity', 'Approval', 'Response', 'Ended'] for a in EVENTS}
CAPABILITIES['Claude'] += ['Question', 'Error']
CAPABILITIES['Agy'] += ['Question', 'Error']
CAPABILITIES['Grok'] += ['Question', 'Error']
LIMITATIONS = {
    'Codex': '질문·일반 오류는 수집하지 않습니다. /hooks 신뢰 검토와 실제 수신은 별도입니다.',
    'Claude': '',
    'Agy': '승인은 ask_permission만 수집하며 일반 권한 팝업은 감지하지 않습니다. 새 실행의 SessionStart가 필요합니다.',
    'Grok': 'idle_prompt와 task_complete는 응답 종료 알림으로 수집하지 않습니다.',
}


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def run(args):
    return subprocess.check_output(args, stderr=subprocess.DEVNULL, timeout=3).decode('utf-8', 'replace').strip()


def processes():
    result = {}
    # lstart prevents attaching observations to a recycled PID. No command line leaves memory.
    arguments = {}
    for line in run(['ps', '-ww', '-axo', 'pid=,args=']).splitlines():
        entry = line.split(None, 1)
        if len(entry) == 2 and entry[0].isdigit():
            arguments[int(entry[0])] = entry[1]
    # comm must be the final column: macOS truncates non-final comm to 15 bytes.
    for line in run(['ps', '-ww', '-axo', 'pid=,ppid=,lstart=,comm=']).splitlines():
        parts = line.split(None, 7)
        if len(parts) < 8 or not parts[0].isdigit() or not parts[1].isdigit():
            continue
        pid, parent = int(parts[0]), int(parts[1])
        start, exe = ' '.join(parts[2:7]), parts[7]
        args = arguments.get(pid, '')
        agent = None
        name = Path(exe).name
        if name in ('codex', 'claude', 'agy', 'grok'):
            agent = {'codex': 'Codex', 'claude': 'Claude', 'agy': 'Agy', 'grok': 'Grok'}[name]
        elif name in ('node', 'nodejs'):
            try:
                argv = shlex.split(args)
            except ValueError:
                argv = []
            # Only the actual entrypoint, never a prompt/argument mentioning a product.
            if len(argv) > 1 and argv[1].startswith('/'):
                if argv[1].endswith('/@openai/codex/bin/codex.js'):
                    agent = 'Codex'
                elif argv[1].endswith('/@anthropic-ai/claude-code/cli.js'):
                    agent = 'Claude'
        result[pid] = {'parent': parent, 'token': digest(f'{pid}:{start}:{exe}'), 'agent': agent,
                       'launcher': bool(agent and name in ('node', 'nodejs'))}
    return result


def descendant(pid, root, procs):
    seen = set()
    while pid in procs and pid not in seen:
        if pid == root:
            return True
        seen.add(pid)
        pid = procs[pid]['parent']
    return False


def topology(tmux, session=None):
    procs = processes()
    ident = run([tmux, 'display-message', '-p', '#{socket_path}|#{pid}'])
    socket, pid = ident.rsplit('|', 1)
    server = digest(socket + ':' + procs[int(pid)]['token'])
    fields = '#{session_id}|#{window_id}|#{pane_id}|#{pane_pid}'
    args = [tmux, 'list-panes', '-a', '-F', fields]
    panes = []
    for line in run(args).splitlines():
        s, w, p, pid = line.split('|')
        if session and s != session:
            continue
        if not (re.fullmatch(r'\$\d+', s) and re.fullmatch(r'@\d+', w) and re.fullmatch(r'%\d+', p)):
            continue
        panes.append({'session': s, 'window': w, 'pane': p, 'pid': int(pid)})
    return server, panes, procs


def run_id(server, pane, token):
    return digest(server + ':' + pane + ':' + token)


def kind(agent, event, payload):
    return ADAPTERS[agent].kind(event, payload)


def common_kind(event):
    if event == 'PermissionRequest':
        return 'Approval'
    # Codex questions are deliberately not inferred from generic tools without a verified route.
    if event == 'Stop':
        return 'Response'
    if event in ('PostToolUseFailure', 'StopFailure'):
        return 'Error'
    if event in ('SessionEnd', 'Interrupt', 'SubagentStop', 'StopCancelled'):
        return 'Ended'
    return 'Activity'


def database():
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(ROOT, 0o700)
    db = sqlite3.connect(DB, timeout=0.8)
    os.chmod(DB, 0o600)
    db.execute('CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE, server TEXT, run TEXT, actor TEXT, data TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS latest (run TEXT, actor TEXT, data TEXT, PRIMARY KEY(run,actor))')
    db.execute('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS conversations (run TEXT PRIMARY KEY, conversation TEXT)')
    db.execute('INSERT OR IGNORE INTO meta VALUES (?, ?)', ('epoch', uuid.uuid4().hex))
    return db


def record(agent, tmux, explicit_event=None):
    # Fail-open: hooks must not approve, block, continue or otherwise control the CLI.
    raw = sys.stdin.buffer.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        return
    payload = json.loads(raw)
    if not isinstance(payload, dict) or agent not in EVENTS:
        return
    event = explicit_event or payload.get('hook_event_name')
    if agent == 'Grok' and not event:
        event = next((e for e in EVENTS[agent] if re.sub(r'(?<!^)(?=[A-Z])', '_', e).lower() == payload.get('hookEventName')), None)
    if explicit_event and payload.get('hook_event_name', explicit_event) != explicit_event:
        return explicit_event
    try:
        record_payload(agent, tmux, event, payload)
    except Exception:
        pass  # Preserve the event-specific neutral response even if observation fails.
    return event


def record_payload(agent, tmux, event, payload):
    if event not in EVENTS[agent] or not os.environ.get('TMUX'):
        return
    event_kind = kind(agent, event, payload)
    if event_kind is None:
        return
    # The global AGY file is shared with its App/IDE, which must not report into a CLI.
    if agent in ('Agy', 'Grok'):
        identity = payload.get('conversationId' if agent == 'Agy' else 'sessionId')
        if not isinstance(identity, str) or not identity:
            return
        if agent == 'Agy' and '/antigravity-cli/brain/' not in payload.get('transcriptPath', ''):
            return
    server, panes, procs = topology(tmux)
    pane = next((p for p in panes if p['pane'] == os.environ.get('TMUX_PANE')), None)
    if pane is None:
        return
    pid = os.getppid()
    seen = set()
    while pid in procs and pid not in seen:
        seen.add(pid)
        if procs[pid]['agent']:
            # Stop at the nearest identified CLI, including a different vendor's CLI.
            if procs[pid]['agent'] != agent or not descendant(pid, pane['pid'], procs):
                return
            break
        pid = procs[pid]['parent']
    else:
        return
    run_key = run_id(server, pane['pane'], procs[pid]['token'])
    child = payload.get('agent_id') or payload.get('subagentType')
    actor = digest(str(child)) if child else 'parent'
    if event.startswith('Subagent') and actor == 'parent':
        actor = 'unidentified-child'
    data = {k: pane[k] for k in ('session', 'window', 'pane')}
    data.update(id=uuid.uuid4().hex, server=server, run=run_key, actor=actor,
                agent=agent, kind=event_kind, event=event,
                at=int(time.time() * 1000))
    with database() as db:
        conversation = payload.get('conversationId') if agent == 'Agy' else payload.get('sessionId') if agent == 'Grok' else payload.get('session_id')
        if conversation:
            conversation = digest(str(conversation))
            primary = db.execute('SELECT conversation FROM conversations WHERE run=?', (run_key,)).fetchone()
            if primary is None and actor == 'parent':
                # AGY supplies no parent ID. Establish its root at SessionStart only.
                if agent == 'Agy' and event != 'SessionStart':
                    return
                db.execute('INSERT INTO conversations VALUES(?,?)', (run_key, conversation))
            elif primary and conversation != primary[0]:
                actor = digest('conversation:' + conversation)
                data['actor'] = actor
        if agent == 'Grok' and payload.get('subagentType'):
            data['actor'] = actor = digest('child:' + str(payload.get('sessionId', child)))
        # Official stable event IDs allow replay deduplication without keeping content.
        identity = (payload.get('toolUseId') if event.startswith(('PreTool', 'PostTool')) else
                    payload.get('promptId') if event in ('Stop', 'StopFailure', 'StopCancelled') else None)
        if agent == 'Grok' and identity:
            data['id'] = digest(':'.join([run_key, actor, event, str(identity)]))
            if db.execute('SELECT 1 FROM events WHERE id=?', (data['id'],)).fetchone():
                return
        db.execute('INSERT INTO events(id,server,run,actor,data) VALUES(?,?,?,?,?)',
                   (data['id'], server, run_key, actor, json.dumps(data)))
        seq = db.execute('SELECT last_insert_rowid()').fetchone()[0]
        data['seq'] = seq
        encoded = json.dumps(data)
        db.execute('UPDATE events SET data=? WHERE seq=?', (encoded, seq))
        db.execute('INSERT OR REPLACE INTO latest VALUES(?,?,?)', (run_key, actor, encoded))
        db.execute('DELETE FROM events WHERE seq <= ?', (seq - LIMIT,))
        db.execute('DELETE FROM latest WHERE (run,actor) NOT IN (SELECT run,actor FROM events)')
        db.execute('DELETE FROM conversations WHERE run NOT IN (SELECT run FROM events)')


def poll(tmux, session, after, epoch):
    server, panes, procs = topology(tmux, session)
    observations = []
    for pane in panes:
        for pid, proc in procs.items():
            if proc['agent'] and descendant(pid, pane['pid'], procs):
                # npm launcher + native executable represent one CLI. Prefer deepest process.
                if proc.get('launcher') and any(other != pid and p['agent'] == proc['agent'] and descendant(other, pid, procs)
                       for other, p in procs.items()):
                    continue
                observations.append(dict(session=pane['session'], window=pane['window'], pane=pane['pane'],
                                         run=run_id(server, pane['pane'], proc['token']), agent=proc['agent'],
                                         actor='parent', last=None, supported=[]))
    result = dict(server=server, epoch='', cursor=0, gap=False, observations=observations, events=[])
    if DB.exists():
        # Read-only polling never creates a helper directory or a database.
        with sqlite3.connect(DB.as_uri() + '?mode=ro', uri=True, timeout=0.8) as db:
            result['epoch'] = db.execute("SELECT value FROM meta WHERE key='epoch'").fetchone()[0]
            first, last = db.execute('SELECT min(seq),max(seq) FROM events').fetchone()
            result['cursor'] = last or 0
            result['gap'] = bool(epoch == result['epoch'] and after and first and after < first - 1)
            runs = {o['run']: o for o in observations}
            received = {}
            for run_key, encoded in db.execute('SELECT run,data FROM events WHERE server=?', (server,)):
                if run_key in runs:
                    data = json.loads(encoded)
                    received.setdefault((run_key, data['actor']), set()).add(data['kind'])
            for run_key, actor, encoded in db.execute('SELECT run,actor,data FROM latest'):
                if run_key not in runs:
                    continue
                data = json.loads(encoded)
                obs = runs[run_key] if actor == 'parent' else dict(runs[run_key], actor=actor)
                obs['last'] = data
                # Only show routes actually received for this process, not every configured hook.
                obs['supported'] = sorted(received.get((run_key, actor), set()))
                if actor != 'parent':
                    observations.append(obs)
            if epoch == result['epoch'] and not result['gap']:
                result['events'] = [json.loads(row[0]) for row in db.execute(
                    'SELECT data FROM events WHERE seq>? AND server=? ORDER BY seq', (after, server))
                    if json.loads(row[0])['run'] in runs]
    return result


def config_paths():
    # Honor the CLI's documented custom user-config directories.
    return {'Codex': Path(os.environ.get('CODEX_HOME', str(Path.home() / '.codex'))) / 'hooks.json',
            'Claude': Path(os.environ.get('CLAUDE_CONFIG_DIR', str(Path.home() / '.claude'))) / 'settings.json',
            'Agy': Path.home() / '.gemini/config/hooks.json',
            'Grok': Path(os.environ.get('GROK_HOME', str(Path.home() / '.grok'))) / 'hooks/dgl-observer-v1.json'}


def load_config(path):
    if path.is_symlink():
        raise ValueError('symlink configuration is not supported')
    raw = path.read_bytes() if path.exists() else b''
    if len(raw) > 4 * 1024 * 1024:
        raise ValueError('configuration is too large')
    data = json.loads(raw) if raw else {}
    if not isinstance(data, dict) or not isinstance(data.get('hooks', {}), dict):
        raise ValueError('invalid hooks configuration')
    return raw, data


def hook_command(agent, tmux, event=None):
    args = [sys.executable, str(HELPER), 'record', '--agent', agent, '--tmux', tmux]
    if event:
        args += ['--event', event]
    return ' '.join(shlex.quote(v) for v in args)


def owned(handler):
    # Stable full helper path and command structure, not a substring of a user's command.
    try:
        args = shlex.split(handler.get('command', ''))
        return (handler.get('type') == 'command' and len(args) in (7, 9) and
                args[1:4] == [str(HELPER), 'record', '--agent'] and args[4] in EVENTS and args[5] == '--tmux' and
                (len(args) == 7 or (args[7] == '--event' and args[8] in EVENTS[args[4]])))
    except (ValueError, AttributeError):
        return False


def standard_transform(data, agent, tmux, install):
    result = copy.deepcopy(data)
    hooks = result.setdefault('hooks', {})
    for event, groups in list(hooks.items()):
        if not isinstance(groups, list):
            raise ValueError('invalid hook matcher groups')
        cleaned = []
        for group in groups:
            if not isinstance(group, dict) or not isinstance(group.get('hooks'), list):
                raise ValueError('invalid hook handlers')
            remaining = [h for h in group['hooks'] if not owned(h)]
            if remaining or not group['hooks']:
                cleaned.append(dict(group, hooks=remaining))
        if cleaned:
            hooks[event] = cleaned
        else:
            del hooks[event]
    if install:
        for event in EVENTS[agent]:
            hooks.setdefault(event, []).append({'hooks': [{'type': 'command', 'command': hook_command(agent, tmux), 'timeout': 3}]})
    if not hooks:
        result.pop('hooks', None)
    return result


class HookAdapter:
    experimental = False

    def load(self, path):
        return load_config(path)

    def transform(self, data, agent, tmux, install):
        return standard_transform(data, agent, tmux, install)

    def handlers(self, data):
        return [h for groups in data.get('hooks', {}).values() for g in groups for h in g['hooks']]

    def neutral(self, event):
        return {}

    def kind(self, event, payload):
        return common_kind(event)


class CodexAdapter(HookAdapter):
    pass


class ClaudeAdapter(HookAdapter):
    def kind(self, event, payload):
        if event == 'PreToolUse' and payload.get('tool_name') == 'AskUserQuestion':
            return 'Question'
        return common_kind(event)


class AgyAdapter(HookAdapter):
    experimental = True
    namespace = 'dgl-observer-v1'

    def kind(self, event, payload):
        tool = payload.get('toolCall', {}).get('name')
        if event == 'PreToolUse' and tool in ('ask_question', 'ask_permission'):
            return 'Question' if tool == 'ask_question' else 'Approval'
        if event in ('PostToolUse', 'Stop') and payload.get('error'):
            return 'Error'
        if event == 'Stop':
            if payload.get('terminationReason') == 'error':
                return 'Error'
            return 'Response' if payload.get('terminationReason') == 'model_stop' else 'Ended'
        return common_kind(event)

    def transform(self, data, agent, tmux, install):
        result = copy.deepcopy(data)
        definition = result.get(self.namespace, {})
        if not isinstance(definition, dict):
            raise ValueError('invalid AGY hook definition')
        has_owned = any(owned(h) for h in self.handlers(data))
        # Refuse a name collision instead of claiming a user's named hook.
        if install and definition and not has_owned:
            raise ValueError('AGY hook name collision')
        for event, groups in list(definition.items()):
            if event == 'enabled':
                if not isinstance(groups, bool):
                    raise ValueError('invalid AGY enabled flag')
                continue
            if event not in EVENTS['Agy'] or not isinstance(groups, list):
                raise ValueError('invalid AGY hook event')
            cleaned = []
            for group in groups:
                if not isinstance(group, dict):
                    raise ValueError('invalid AGY handler')
                if event in ('PreToolUse', 'PostToolUse'):
                    if not isinstance(group.get('hooks'), list):
                        raise ValueError('invalid AGY matcher')
                    remaining = [h for h in group['hooks'] if not owned(h)]
                    if remaining or not group['hooks']:
                        cleaned.append(dict(group, hooks=remaining))
                elif not owned(group):
                    cleaned.append(group)
            if cleaned:
                definition[event] = cleaned
            else:
                definition.pop(event)
        if not install and not has_owned:
            return copy.deepcopy(data)
        if install:
            for event in EVENTS[agent]:
                handler = {'type': 'command', 'command': hook_command(agent, tmux, event), 'timeout': 3}
                entry = {'hooks': [handler]} if event in ('PreToolUse', 'PostToolUse') else handler
                definition.setdefault(event, []).append(entry)
        if any(k != 'enabled' for k in definition):
            result[self.namespace] = definition
        else:
            result.pop(self.namespace, None)
        return result

    def handlers(self, data):
        definition = data.get(self.namespace, {})
        if not isinstance(definition, dict):
            return []
        return [h for event, groups in definition.items() if event != 'enabled' and isinstance(groups, list)
                for g in groups if isinstance(g, dict)
                for h in (g.get('hooks', []) if event in ('PreToolUse', 'PostToolUse') else [g])]

    def neutral(self, event):
        # AGY's allow delegates to the remaining hooks/default policy; auto_approve would bypass it.
        # Stop requires a decision string; empty leaves the original stop unchanged.
        return {'decision': 'allow'} if event == 'PreToolUse' else {'decision': ''} if event == 'Stop' else {}


class GrokAdapter(HookAdapter):
    experimental = True

    def kind(self, event, payload):
        if event == 'Notification':
            # idle_prompt also follows failures/cancellations; task_complete duplicates Stop.
            return 'Approval' if payload.get('notificationType') == 'permission_prompt' else None
        if event == 'PreToolUse' and payload.get('toolName') == 'ask_user_question':
            return 'Question'
        return common_kind(event)


ADAPTERS = {'Codex': CodexAdapter(), 'Claude': ClaudeAdapter(), 'Agy': AgyAdapter(), 'Grok': GrokAdapter()}


def transformed(data, agent, tmux, install):
    return ADAPTERS[agent].transform(data, agent, tmux, install)


def inspect(tmux):
    if sys.version_info < (3, 9):
        raise ValueError('Python 3.9 or newer required')
    configs = []
    token_parts = []
    for agent, path in config_paths().items():
        adapter = ADAPTERS[agent]
        error, version, installed, supported = '', 'not installed', False, False
        try:
            raw, data = adapter.load(path)
            executable = shutil.which(agent.lower())
            version = run([executable, '--version']) if executable else 'not installed'
            supported = bool(executable) and (adapter.experimental or any(
                re.search(r'(?<![\w.])' + re.escape(v) + r'(?![\w.+-])', version) for v in VERSIONS[agent]))
            transformed(data, agent, tmux, supported)
            installed = any(owned(h) for h in adapter.handlers(data))
        except Exception:
            supported = False
            error = '설정 형식·경로 또는 CLI 버전 조회 실패 · 설치 제외'
            # Keep the conflict token sensitive to malformed-file changes too.
            try:
                raw = path.read_bytes() if path.is_file() and not path.is_symlink() else b''
            except OSError:
                raw = b''
        configs.append(dict(agent=agent, path=str(path), version=version, supported=supported,
                            installed=installed, experimental=adapter.experimental, error=error,
                            capabilities=CAPABILITIES[agent], limitations=LIMITATIONS[agent],
                            events=EVENTS[agent] if supported else []))
        token_parts.append(str(path) + ':' + digest(raw.hex()))
    helper_hash = digest(HELPER.read_bytes().hex()) if HELPER.exists() else ''
    token = digest('|'.join(token_parts) + helper_hash + str(configs))
    return dict(token=token, configs=configs, python=sys.version.split()[0], tmux=run([tmux, '-V']),
                message='설치됨 / 활성화 확인 필요. Codex /hooks에서 검토·신뢰 후 새 실행의 실제 이벤트 수신으로 확인합니다. 기존 CLI는 다시 시작하지 않습니다.',
                helper=str(HELPER), database=str(DB))


def atomic_write(path, raw, expected):
    if raw == expected and path.exists() and not path.is_symlink():
        return
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix='.dgl-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as f:
            f.write(raw)
            f.flush()
            os.fsync(f.fileno())
        current = path.read_bytes() if path.exists() else b''
        if path.is_symlink() or current != expected:
            raise ValueError('configuration changed; inspect again')
        if current:
            backup = path.with_name(path.name + '.dgl-backup-' + uuid.uuid4().hex)
            with backup.open('xb') as f:
                os.chmod(backup, 0o600)
                f.write(current)
        # Recheck immediately before replacement. Arbitrary non-cooperating writers cannot be locked.
        if (path.read_bytes() if path.exists() else b'') != expected:
            raise ValueError('configuration changed; inspect again')
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def configure(tmux, token, install, source):
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    lock = ROOT / 'install.lock'
    fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        preview = inspect(tmux)
        if token != preview['token']:
            raise ValueError('configuration changed; inspect again')
        if install and not any(c['supported'] for c in preview['configs']):
            raise ValueError('unsupported CLI version')
        changes = []
        for c in preview['configs']:
            if c['error'] or (install and not c['supported']):
                continue
            path = Path(c['path'])
            raw, data = ADAPTERS[c['agent']].load(path)
            new = transformed(data, c['agent'], tmux, install and c['supported'])
            if data != new:
                changes.append((path, raw, (json.dumps(new, ensure_ascii=False, indent=2) + '\n').encode()))
        # Recheck after preparing all changes; a writer may have changed a config
        # between the preview comparison and load_config above.
        if inspect(tmux)['token'] != token:
            raise ValueError('configuration changed; inspect again')
        if install:
            old = HELPER.read_bytes() if HELPER.exists() else b''
            atomic_write(HELPER, source.encode(), old)
        for path, old, new in changes:
            atomic_write(path, new, old)
        if not install and DB.exists():
            # Keep bounded history, but don't present a removed integration's old
            # observations as live. A later actual hook can establish a new observation.
            with database() as db:
                db.execute('DELETE FROM latest')
                db.execute("UPDATE meta SET value=? WHERE key='epoch'", (uuid.uuid4().hex,))
        return inspect(tmux)
    finally:
        os.close(fd)
        lock.unlink()


def main(source=''):
    os.umask(0o077)
    p = argparse.ArgumentParser()
    p.add_argument('action', choices=['poll', 'record', 'inspect', 'install', 'remove'])
    p.add_argument('--tmux', required=True)
    p.add_argument('--session')
    p.add_argument('--agent', choices=list(EVENTS))
    p.add_argument('--event')
    p.add_argument('--after', type=int, default=0)
    p.add_argument('--epoch', default='')
    p.add_argument('--token', default='')
    args = p.parse_args()
    try:
        if args.action == 'record':
            event = record(args.agent, args.tmux, args.event)
            print(json.dumps(ADAPTERS[args.agent].neutral(event if isinstance(event, str) else args.event)))
            return
        if args.action == 'poll':
            result = poll(args.tmux, args.session, args.after, args.epoch)
        elif args.action == 'inspect':
            result = inspect(args.tmux)
        else:
            result = configure(args.tmux, args.token, args.action == 'install', source or Path(__file__).read_text())
        print(json.dumps(result, ensure_ascii=False))
    except Exception:
        if args.action == 'record':
            print(json.dumps(ADAPTERS[args.agent].neutral(args.event)) if args.agent in ADAPTERS else '{}')
        else:
            # Never echo configuration contents, payloads, commands or secret-bearing exceptions.
            print(json.dumps({'error': '연동 조회/설정 실패: Python·tmux·CLI 버전·설정 형식 또는 외부 변경을 확인한 뒤 다시 검사하세요.'}))
            sys.exit(1)


if __name__ == '__main__':
    main()
