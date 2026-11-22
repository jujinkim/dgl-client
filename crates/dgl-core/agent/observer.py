#!/usr/bin/env python3
"""dgl's opt-in observer. No transcripts, prompt text, tool inputs or outputs are persisted.

Status collection keeps whitelisted figures only: token counts, percentages, model ids
and names, effort levels, cost estimates, limit windows and hashed account keys. It never
keeps email addresses, titles, paths, prompts, messages or raw session/account ids, and
never reads transcripts, conversation databases or credential files.

Also runs from stdin/-c for read-only discovery before installation. Python 3.9+,
POSIX tmux hosts; the SSH client itself may run on Windows, macOS or Android.
"""
import argparse
import contextlib
from contextlib import closing
import copy
import datetime
import glob
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import re
import selectors
import shlex
import shutil
import signal
import sqlite3
import stat
import subprocess
import sys
import tempfile
import threading
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
# Status-only events update the status table, never events/latest (no notification, no
# `supported` kind). A gated one is installed only from that CLI release on.
STATUS_EVENTS = {'Claude': ['PostModelSwitch'], 'Codex': ['PostModelRequest']}
STATUS_GATE = {('Codex', 'PostModelRequest'): (0, 171, 0)}
# Release gate: the oldest release whose hooks were received for real. Later stable
# releases keep the same hook contract; pre-releases and malformed versions stay excluded.
MINIMUM = {'Codex': (0, 163, 0), 'Claude': (2, 1, 310)}
RELEASE = re.compile(r'(?<![\w.])(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?![\w.+-])')
CAPABILITIES = {a: ['Activity', 'Approval', 'Response', 'Ended'] for a in EVENTS}
CAPABILITIES['Claude'] += ['Question', 'Error']
CAPABILITIES['Agy'] += ['Question', 'Error']
CAPABILITIES['Grok'] += ['Question', 'Error']

INPUT_LIMIT = 1024 * 1024
TEE_TIMEOUT = 4.5  # seconds the saved statusline original may run
TEE_DRAIN = 0.2  # seconds to wait for stdout to close after `sh` exits
STATUS_RETENTION_MS = 24 * 3600 * 1000
MOD_STALE_MS = 6 * 3600 * 1000
MOD_NAME = 'dgl-agent-status'
PLUGIN_DIRS = 'CLAUDE_CODE_PLUGIN_DIRS'
STATUS_KIND = {'Claude': 'mod', 'Agy': 'statusline', 'Codex': 'hooks', 'Grok': 'files'}
SOURCES = ('mod', 'statusline', 'hook', 'file', 'command')
ALERTS = ('rate_limit', 'billing_error', 'authentication_failed')
ACCOUNT_KINDS = ('subscription', 'api_key', 'cloud', 'enterprise', 'other')
PART_KINDS = ('used', 'free', 'buffer', 'deferred')
PART_NAMES = {'System prompt': 'system_prompt', 'System tools': 'system_tools', 'MCP tools': 'mcp_tools',
              'Custom agents': 'custom_agents', 'Memory files': 'memory_files', 'Skills': 'skills',
              'Messages': 'messages'}
CATEGORIES = set(PART_NAMES.values()) | {'autocompact_buffer', 'free', 'deferred', 'other'}
CODEX_EFFORT_EVENTS = ('SessionStart', 'UserPromptSubmit', 'PostModelRequest', 'Stop')
GROK_FILE_EVENTS = ('SessionStart', 'Stop', 'StopFailure')
MISSING = object()


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def now_ms():
    return int(time.time() * 1000)


def release_at_least(version, minimum):
    return any(tuple(map(int, m.groups())) >= minimum for m in RELEASE.finditer(version))


def release_supported(agent, version):
    return release_at_least(version, MINIMUM[agent])


def hook_events(agent, version=''):
    """Configured hook routes; a gated status-only event joins only from its release on."""
    events = list(EVENTS[agent])
    for event in STATUS_EVENTS.get(agent, []):
        gate = STATUS_GATE.get((agent, event))
        if gate is None or release_at_least(version, gate):
            events.append(event)
    return events


def run(args):
    return subprocess.check_output(args, stderr=subprocess.DEVNULL, timeout=3).decode('utf-8', 'replace').strip()


APP_SERVER = re.compile(r'(?:^|/)codex(?:\.js)? app-server(?:\s|$)')


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
        launcher = name in ('node', 'nodejs')
        argv = []
        if launcher:
            try:
                argv = shlex.split(args)
            except ValueError:
                pass
        if name in ('codex', 'claude', 'agy', 'grok'):
            agent = {'codex': 'Codex', 'claude': 'Claude', 'agy': 'Agy', 'grok': 'Grok'}[name]
        elif launcher:
            # Only the actual entrypoint, never a prompt/argument mentioning a product.
            if len(argv) > 1 and argv[1].startswith('/'):
                if argv[1].endswith('/@openai/codex/bin/codex.js'):
                    agent = 'Codex'
                elif argv[1].endswith('/@anthropic-ai/claude-code/cli.js'):
                    agent = 'Claude'
        daemon = managed = None
        # Codex 0.171+ serves every TUI of a CODEX_HOME from one `codex app-server` daemon,
        # which also runs their hooks. No app-server is a CLI in the pane it descends from.
        # Matched on the raw text so an install path with spaces still counts.
        if agent == 'Codex' and APP_SERVER.search(args):
            agent, daemon = None, 'Codex'
            # Only the TUIs' shared, managed daemon is attributed to a pane. An IDE's or other
            # client's own app-server is not, even if a TUI shares its directory.
            managed = '--managed-daemon' in args.split()
        result[pid] = {'parent': parent, 'token': digest(f'{pid}:{start}:{exe}'), 'agent': agent,
                       'launcher': bool(agent and launcher), 'daemon': daemon, 'managed': bool(managed),
                       'started': started(start)}
    return result


def started(lstart):
    # Best effort: ps prints lstart in the C locale form on the supported hosts.
    try:
        return int(time.mktime(time.strptime(lstart, '%a %b %d %H:%M:%S %Y')))
    except (ValueError, OverflowError):
        return None


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


def database(timeout=0.8):
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(ROOT, 0o700)
    db = sqlite3.connect(DB, timeout=timeout)
    os.chmod(DB, 0o600)
    db.execute('CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE, server TEXT, run TEXT, actor TEXT, data TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS latest (run TEXT, actor TEXT, data TEXT, PRIMARY KEY(run,actor))')
    db.execute('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS conversations (run TEXT PRIMARY KEY, conversation TEXT)')
    db.execute('CREATE TABLE IF NOT EXISTS status (run TEXT, actor TEXT, agent TEXT, data TEXT, at INTEGER, PRIMARY KEY(run,actor))')
    # Codex thread -> TUI run bindings for daemon-run hooks; only digests are stored.
    db.execute('CREATE TABLE IF NOT EXISTS threads (thread TEXT PRIMARY KEY, run TEXT, at INTEGER)')
    db.execute('INSERT OR IGNORE INTO meta VALUES (?, ?)', ('epoch', uuid.uuid4().hex))
    return db


# ---------------------------------------------------------------------------
# Status whitelist. Every collector builds a candidate from named fields only, and
# clean_status() bounds it to the wire contract. Unknown stays absent, never 0.

def text(value, limit=128):
    if isinstance(value, str) and 0 < len(value) <= limit and value.isprintable():
        return value
    return None


def number(value, low=0.0, high=None):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    if not math.isfinite(value) or value < low or (high is not None and value > high):
        return None
    return value


def count(value):
    value = number(value, 0, 10 ** 15)
    if value is None or not float(value).is_integer():
        return None
    return int(value)


def compact(values):
    return {k: v for k, v in values.items() if v is not None and v != [] and v != {}}


def epoch_seconds(value):
    """Epoch seconds from epoch seconds/ms or an ISO 8601 text; None when unreadable."""
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        if not math.isfinite(value) or value <= 0:
            return None
        return value / 1000 if value > 1e12 else float(value)
    if not isinstance(value, str) or len(value) > 64:
        return None
    m = re.fullmatch(r'(\d{4}-\d{2}-\d{2})[T ](\d{2}:\d{2}(?::\d{2})?)(?:[.,](\d+))?\s*(Z|z|[+-]\d{2}:?\d{2})?',
                     value.strip())
    if not m:
        return None
    zone = m.group(4) or 'Z'
    zone = '+00:00' if zone in ('Z', 'z') else zone if ':' in zone else zone[:3] + ':' + zone[3:]
    clock = m.group(2) if m.group(2).count(':') == 2 else m.group(2) + ':00'
    try:
        moment = datetime.datetime.fromisoformat(m.group(1) + 'T' + clock + zone)
    except ValueError:
        return None
    return moment.timestamp() + (float('0.' + m.group(3)) if m.group(3) else 0.0)


def account_key(agent, kind, plan):
    return digest('account:%s:%s:%s' % (agent, kind or '', plan or ''))


def part(row):
    if not isinstance(row, dict):
        return None
    tokens = count(row.get('tokens'))
    name = text(row.get('name'), 64)
    row_kind = row.get('kind') if row.get('kind') in PART_KINDS else None
    if tokens is None or (name is None and row.get('category') not in CATEGORIES):
        return None
    category = row.get('category') if row.get('category') in CATEGORIES else (
        'free' if row_kind == 'free' else 'autocompact_buffer' if row_kind == 'buffer' else
        'deferred' if row_kind == 'deferred' else PART_NAMES.get(name, 'other'))
    return compact({'category': category, 'name': name, 'tokens': tokens, 'kind': row_kind})


def clean_status(status):
    """Bounds a candidate to the wire contract. `alert: None` survives: it clears an alert."""
    if not isinstance(status, dict):
        return {}
    out = {}
    model = status.get('model')
    if isinstance(model, dict):
        out['model'] = compact({'id': text(model.get('id')), 'name': text(model.get('name'), 64)})
    out['effort'] = text(status.get('effort'), 32)
    if isinstance(status.get('thinking'), bool):
        out['thinking'] = status['thinking']
    context = status.get('context')
    if isinstance(context, dict):
        rows = context.get('breakdown') if isinstance(context.get('breakdown'), list) else []
        out['context'] = compact({
            'used': count(context.get('used')), 'size': count(context.get('size')),
            'percent': number(context.get('percent'), 0, 100),
            'compact_percent': number(context.get('compact_percent'), 0, 100),
            'breakdown': [r for r in (part(r) for r in rows[:24]) if r],
            'breakdown_total': count(context.get('breakdown_total')),
            'breakdown_window': count(context.get('breakdown_window'))})
    cache = status.get('cache')
    if isinstance(cache, dict):
        values = compact({k: count(cache.get(k)) for k in ('input', 'read', 'write', 'output', 'last_at')})
        values['ttl'] = cache.get('ttl') if cache.get('ttl') in ('5m', '1h') else None
        ratio = number(cache.get('hit_ratio'), 0, 1)
        if ratio is None and 'input' in values and 'read' in values:
            total = values['input'] + values['read'] + values.get('write', 0)
            ratio = values['read'] / total if total else None
        values['hit_ratio'] = ratio
        out['cache'] = compact(values)
    out['cost_usd'] = number(status.get('cost_usd'), 0, 1e7)
    account = status.get('account')
    if isinstance(account, dict):
        key = account.get('key')
        out['account'] = compact({
            'kind': account.get('kind') if account.get('kind') in ACCOUNT_KINDS else None,
            'plan': text(account.get('plan'), 64),
            'key': key if isinstance(key, str) and re.fullmatch(r'[0-9a-f]{64}', key) else None})
    limits = []
    for limit in (status.get('limits') if isinstance(status.get('limits'), list) else [])[:16]:
        if not isinstance(limit, dict) or text(limit.get('id'), 64) is None:
            continue
        resets = number(limit.get('resets_at'), 1, 1e11)
        limits.append(compact({'id': limit['id'], 'label': text(limit.get('label'), 64),
                               'used_percent': number(limit.get('used_percent'), 0, 1000),
                               'remaining_fraction': number(limit.get('remaining_fraction'), 0, 1),
                               'resets_at': int(resets) if resets is not None else None}))
    out['limits'] = limits
    out['alert'] = status.get('alert') if status.get('alert') in ALERTS else None
    out = compact(out)
    if 'alert' in status and status['alert'] is None:
        out['alert'] = None
    if not out:
        return {}
    sources = status.get('sources') if isinstance(status.get('sources'), list) else []
    out['sources'] = [s for s in SOURCES if s in sources][:8]
    at = count(status.get('at'))
    if at is not None:
        out['at'] = at
    limits_at = count(status.get('limits_at'))
    if limits_at is not None and out.get('limits'):
        out['limits_at'] = limits_at
    return out


def status_from_hook(agent, event, payload, now=None):
    """Whitelisted status from one hook payload (no I/O)."""
    if not isinstance(payload, dict):
        return {}
    now = now_ms() if now is None else now
    status = {'sources': ['hook'], 'at': now}
    if agent == 'Claude':
        effort = payload.get('effort')
        if isinstance(effort, dict):
            status['effort'] = effort.get('level')
        model = payload.get('to_model') if event == 'PostModelSwitch' else payload.get('model') if event == 'SessionStart' else None
        if isinstance(model, str):
            status['model'] = {'id': model}
        if event == 'StopFailure' and payload.get('error') in ALERTS:
            status['alert'] = payload['error']
        elif event == 'Stop':
            status['alert'] = None
    elif agent == 'Codex':
        if isinstance(payload.get('model'), str):
            status['model'] = {'id': payload['model']}
        usage = payload.get('token_usage')
        if event == 'PostModelRequest' and payload.get('kind') == 'sampling' and isinstance(usage, dict):
            given = {k: count(usage.get(k)) for k in ('input_tokens', 'cached_input_tokens', 'cache_write_input_tokens',
                                                       'output_tokens', 'total_tokens')}
            write = given['cache_write_input_tokens'] if given['cache_write_input_tokens'] is not None else 0
            if given['input_tokens'] is not None and given['cached_input_tokens'] is not None:
                # Codex input_tokens includes the cached part.
                status['cache'] = {'input': max(0, given['input_tokens'] - given['cached_input_tokens'] - write),
                                   'read': given['cached_input_tokens'], 'write': write,
                                   'output': given['output_tokens'], 'last_at': now}
            if given['total_tokens'] is not None:
                status['context'] = {'used': given['total_tokens']}
    elif agent == 'Agy':
        if isinstance(payload.get('modelName'), str):
            status['model'] = {'id': payload['modelName']}
    elif agent == 'Grok':
        if event == 'StopFailure':
            error = payload.get('errorType') if payload.get('errorType') in ALERTS else payload.get('error')
            if error in ALERTS:
                status['alert'] = error
        elif event == 'Stop':
            status['alert'] = None
    return clean_status(status)


def status_from_statusline(agent, payload, now=None):
    """Whitelisted status from a statusline payload (AGY shape; Claude/Grok shapes are alike)."""
    if not isinstance(payload, dict):
        return {}
    now = now_ms() if now is None else now
    status = {'sources': ['statusline'], 'at': now}
    model = payload.get('model')
    if isinstance(model, dict):
        status['model'] = {'id': model.get('id'), 'name': model.get('display_name')}
        effort = model.get('effort')
        status['effort'] = effort.get('level') if isinstance(effort, dict) else effort
    if isinstance(payload.get('effort'), dict):
        status['effort'] = payload['effort'].get('level')
    if isinstance(payload.get('thinking'), dict):
        status['thinking'] = payload['thinking'].get('enabled')
    window = payload.get('context_window')
    if isinstance(window, dict):
        context = {'percent': window.get('used_percentage'), 'size': window.get('context_window_size')}
        current = window.get('current_usage')
        if isinstance(current, dict):
            values = {k: count(current.get(k)) for k in ('input_tokens', 'output_tokens',
                                                         'cache_creation_input_tokens', 'cache_read_input_tokens')}
            # current_usage.input_tokens is the uncached part only.
            status['cache'] = {'input': values['input_tokens'], 'read': values['cache_read_input_tokens'],
                               'write': values['cache_creation_input_tokens'], 'output': values['output_tokens']}
            parts = [values[k] for k in ('input_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens')]
            if all(v is not None for v in parts):
                context['used'] = sum(parts)
        status['context'] = context
    quota = payload.get('quota')
    if isinstance(quota, dict):
        limits = []
        for bucket, value in itertools.islice(quota.items(), 16):
            if not isinstance(value, dict) or value.get('disabled') is True or text(bucket, 64) is None:
                continue
            fraction = number(value.get('remaining_fraction'), 0, 1)
            resets = epoch_seconds(value.get('reset_time'))
            if resets is None and number(value.get('reset_in_seconds'), 0, 1e9) is not None:
                resets = now / 1000 + value['reset_in_seconds']
            limits.append({'id': bucket, 'label': bucket, 'remaining_fraction': fraction,
                           'used_percent': round(100 * (1 - fraction), 1) if fraction is not None else None,
                           'resets_at': resets})
        status['limits'] = limits
    plan = text(payload.get('plan_tier'), 64)
    if plan:
        status['account'] = {'plan': plan, 'key': account_key(agent, None, plan)}
    cost = payload.get('cost')
    if isinstance(cost, dict):
        status['cost_usd'] = cost.get('total_usd') if 'total_usd' in cost else cost.get('total_cost_usd')
    return clean_status(status)


def status_from_claude_mod(doc, pid=None, now=None):
    """Status from a file the Claude Code mod wrote; {} unless fresh and for this pid."""
    if not isinstance(doc, dict) or doc.get('v') != 1 or not isinstance(doc.get('status'), dict):
        return {}
    if pid is not None and doc.get('pid') != pid:
        return {}
    now = now_ms() if now is None else now
    at = count(doc.get('at'))
    if at is None or at < now - MOD_STALE_MS or at > now + 60000:
        return {}
    status = clean_status(dict(doc['status'], at=at, sources=['mod']))
    account = status.get('account')
    if account and 'key' not in account and ('kind' in account or 'plan' in account):
        account['key'] = account_key('Claude', account.get('kind'), account.get('plan'))
    # The model is sampled at turn ends only, while every measure rewrites the file.
    model_at = count(doc.get('model_at'))
    if status and model_at is not None and model_at <= at:
        status['_ts'] = {'model': model_at}
    return status


def status_from_grok_files(signals, usage, now=None):
    """Status from Grok's signals.json and usage.json values (already parsed)."""
    now = now_ms() if now is None else now
    status = {'sources': ['file'], 'at': now}
    if isinstance(signals, dict):
        size = count(signals.get('contextWindowTokens'))
        # serde defaults write 0 before a measurement; 0 is unknown, never a figure.
        if size:
            status['context'] = {'size': size, 'used': count(signals.get('contextTokensUsed')),
                                 'percent': signals.get('contextWindowUsage')}
        if isinstance(signals.get('primaryModelId'), str):
            status['model'] = {'id': signals['primaryModelId']}
    if isinstance(usage, dict):
        session = usage.get('session')
        if isinstance(session, dict):
            ticks = session.get('costUsdTicks')
            if count(ticks) is not None:
                status['cost_usd'] = ticks / 1e10
            if 'model' not in status and isinstance(session.get('primaryModelId'), str):
                status['model'] = {'id': session['primaryModelId']}
        turns = usage.get('turns')
        if isinstance(turns, list) and turns and isinstance(turns[-1], dict):
            last = turns[-1]
            values = {k: count(last.get(k)) for k in ('inputTokens', 'cachedReadTokens', 'cacheCreationTokens', 'outputTokens')}
            if values['inputTokens'] is not None and values['cachedReadTokens'] is not None:
                write = values['cacheCreationTokens'] or 0
                ended = epoch_seconds(last.get('endedAt'))
                # Grok inputTokens includes the cached part.
                status['cache'] = {'input': max(0, values['inputTokens'] - values['cachedReadTokens'] - write),
                                   'read': values['cachedReadTokens'], 'write': values['cacheCreationTokens'],
                                   'output': values['outputTokens'],
                                   'last_at': int(ended * 1000) if ended is not None else None}
    return clean_status(status)


def merge_status(old, new, now=None):
    """A collector replaces only the top-level keys it reports; None removes one."""
    now = now_ms() if now is None else now
    result = dict(old) if isinstance(old, dict) else {}
    stamps = dict(result.pop('_ts', None) or {})
    for key, value in new.items():
        if key in ('at', 'sources'):
            continue
        if value is None:
            result.pop(key, None)
        else:
            if key == 'model' and isinstance(result.get('model'), dict) and result['model'].get('id') == value.get('id'):
                value = dict(result['model'], **value)
            result[key] = value
        stamps[key] = now
    sources = set(result.get('sources') or []) | set(new.get('sources') or [])
    result['sources'] = [s for s in SOURCES if s in sources]
    result['at'] = max(count(result.get('at')) or 0, count(new.get('at')) or now)
    result['_ts'] = stamps
    return result


def wire_status(row, mod):
    """Merges a status row and a Claude mod sample: the newer sample of each key wins,
    the mod on a tie."""
    row = dict(row) if isinstance(row, dict) else {}
    stamps = row.pop('_ts', None) or {}
    mod = dict(mod or {})
    mod_stamps = mod.pop('_ts', None) or {}
    out = {}
    limits_at = None
    for key in set(row) | set(mod):
        if key in ('at', 'sources', 'limits_at'):
            continue
        mod_at = mod_stamps.get(key, mod.get('at', 0))
        if key in mod and (key not in row or stamps.get(key, 0) <= mod_at):
            value, other, sampled = mod[key], row.get(key), mod_at
        else:
            value, other, sampled = row[key], mod.get(key), stamps.get(key) or row.get('at')
        if key == 'model' and isinstance(other, dict) and isinstance(value, dict) and other.get('id') == value.get('id'):
            value = dict(other, **value)
        if key == 'limits':
            limits_at = count(sampled)
        out[key] = value
    out['sources'] = list(set(row.get('sources') or []) | set(mod.get('sources') or []))
    out['at'] = max(count(row.get('at')) or 0, count(mod.get('at')) or 0) or None
    if limits_at and out.get('limits'):
        out['limits_at'] = min(limits_at, out['at'] or limits_at)  # when the limits were sampled
    status = clean_status(out)
    if status.get('alert') is None:
        status.pop('alert', None)  # a clearing marker never reaches the wire
    return status if any(k not in ('at', 'sources') for k in status) else {}


def upsert_status(db, run_key, actor, agent, status, now=None):
    now = now_ms() if now is None else now
    if not status:
        return
    found = db.execute('SELECT data FROM status WHERE run=? AND actor=?', (run_key, actor)).fetchone()
    try:
        old = json.loads(found[0]) if found else {}
    except ValueError:
        old = {}
    merged = merge_status(old, status, now)
    if not found and not any(k not in ('at', 'sources', '_ts') for k in merged):
        return
    db.execute('INSERT OR REPLACE INTO status VALUES(?,?,?,?,?)', (run_key, actor, agent, json.dumps(merged), now))
    db.execute('DELETE FROM status WHERE at < ?', (now - STATUS_RETENTION_MS,))


def read_limited(path, limit):
    """Bytes of a regular, non-symlink file of at most `limit` bytes; None otherwise."""
    try:
        info = os.lstat(path)
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            return None
        with open(path, 'rb') as f:
            raw = f.read(limit + 1)
        return raw if len(raw) <= limit else None
    except OSError:
        return None


def read_json(path, limit):
    raw = read_limited(path, limit)
    if raw is None:
        return None
    try:
        return json.loads(raw)
    except ValueError:
        return None


def codex_home():
    return Path(os.environ.get('CODEX_HOME', str(Path.home() / '.codex')))


_WINDOWS = {}


def codex_window(model):
    """Usable context window of `model` from Codex's model catalog cache."""
    path = codex_home() / 'models_cache.json'
    try:
        info = os.lstat(path)
    except OSError:
        return None
    key = (str(path), info.st_mtime_ns, info.st_size)
    if key not in _WINDOWS:
        _WINDOWS.clear()
        data = read_json(path, 4 * 1024 * 1024)
        windows = {}
        for entry in (data.get('models') if isinstance(data, dict) and isinstance(data.get('models'), list) else []):
            if not isinstance(entry, dict) or not isinstance(entry.get('slug'), str):
                continue
            window = count(entry.get('context_window')) or count(entry.get('max_context_window'))
            percent = number(entry.get('effective_context_window_percent'), 1, 100)
            if window:
                windows[entry['slug']] = int(window * percent // 100) if percent is not None else window
        _WINDOWS[key] = windows
    return _WINDOWS[key].get(model)


def codex_thread(thread):
    """(reasoning_effort, model) of one Codex thread, read-only, from the newest state DB."""
    if text(thread) is None:
        return None
    best = None
    for path in itertools.islice(codex_home().glob('state_*.sqlite'), 64):
        m = re.fullmatch(r'state_(\d+)\.sqlite', path.name)
        if m and (best is None or int(m.group(1)) > best[0]):
            best = (int(m.group(1)), path)
    if best is None:
        return None
    with closing(sqlite3.connect(best[1].as_uri() + '?mode=ro', uri=True, timeout=0.2)) as db:
        return db.execute('SELECT reasoning_effort, model FROM threads WHERE id=?', (thread,)).fetchone()


def grok_files(session_id):
    """Newest signals.json/usage.json of one Grok session, or (None, None)."""
    if not isinstance(session_id, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]{0,127}', session_id):
        return None, None
    home = Path(os.environ.get('GROK_HOME', str(Path.home() / '.grok')))
    pattern = os.path.join(glob.escape(str(home / 'sessions')), '*', glob.escape(session_id), 'signals.json')
    newest = None
    for match in itertools.islice(glob.iglob(pattern), 64):
        try:
            mtime = os.lstat(match).st_mtime
        except OSError:
            continue
        if newest is None or mtime > newest[0]:
            newest = (mtime, Path(match))
    if newest is None:
        return None, None
    return read_json(newest[1], 256 * 1024), read_json(newest[1].parent / 'usage.json', INPUT_LIMIT)


def collect_hook_status(agent, event, payload, now=None):
    """status_from_hook plus the passive local sources the hook points at. Fail-open."""
    now = now_ms() if now is None else now
    status = status_from_hook(agent, event, payload, now)
    try:
        if agent == 'Codex':
            model = payload.get('model')
            if event == 'PostModelRequest' and 'used' in status.get('context', {}) and isinstance(model, str):
                size = codex_window(model)
                if size:
                    status['context']['size'] = size
                    status['context']['percent'] = round(min(100.0, 100.0 * status['context']['used'] / size), 1)
            if event in CODEX_EFFORT_EVENTS:
                row = codex_thread(payload.get('thread_id') or payload.get('session_id'))
                if row:
                    status = clean_status(dict(status, sources=['hook'], effort=row[0],
                                               model=status.get('model') or {'id': row[1]}))
        elif agent == 'Grok' and event in GROK_FILE_EVENTS:
            files = status_from_grok_files(*grok_files(payload.get('sessionId')), now=now)
            if files:
                merged = dict(files, **{k: v for k, v in status.items() if k not in ('sources', 'at')})
                merged['sources'] = ['hook', 'file']
                status = clean_status(merged)
    except Exception:
        pass  # A passive source is optional; the hook's own fields still count.
    return status


def prune_mod_files(now):
    folder = ROOT / 'claude'
    try:
        entries = list(itertools.islice(os.scandir(folder), 512))
    except OSError:
        return
    for entry in entries:
        try:
            if entry.name.endswith('.json') and entry.is_file(follow_symlinks=False) and \
                    entry.stat(follow_symlinks=False).st_mtime * 1000 < now - STATUS_RETENTION_MS:
                os.unlink(entry.path)
        except OSError:
            pass


def claude_mod_file(pid, now, proc=None):
    if not isinstance(pid, int) or pid <= 0:
        return {}
    path = ROOT / 'claude' / ('%d.json' % pid)
    try:
        if os.lstat(path).st_mtime * 1000 < now - MOD_STALE_MS:
            return {}
    except OSError:
        return {}
    # A partial or foreign file is ignored, never repaired.
    status = status_from_claude_mod(read_json(path, 256 * 1024), pid, now)
    # A sample from before this process started belongs to a recycled PID.
    begun = (proc or {}).get('started')
    if status and isinstance(begun, int) and status.get('at', 0) < (begun - 60) * 1000:
        return {}
    return status


# ---------------------------------------------------------------------------
# Hooks and statusline recording.

def record(agent, tmux, explicit_event=None):
    # Fail-open: hooks must not approve, block, continue or otherwise control the CLI.
    raw = sys.stdin.buffer.read(INPUT_LIMIT + 1)
    if len(raw) > INPUT_LIMIT:
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


def pane_clis(server, panes, procs):
    """(pane, pid, run) of each identified CLI process under a pane; one per npm launcher + binary."""
    found = []
    for pane in panes:
        for pid, proc in procs.items():
            if proc['agent'] and descendant(pid, pane['pid'], procs):
                # npm launcher + native executable represent one CLI. Prefer deepest process.
                if proc.get('launcher') and any(other != pid and p['agent'] == proc['agent'] and descendant(other, pid, procs)
                       for other, p in procs.items()):
                    continue
                found.append((pane, pid, run_id(server, pane['pane'], proc['token'])))
    return found


def process_cwd(pid):
    """A process's working directory, or None. Linux /proc first, then lsof (macOS)."""
    try:
        return os.path.realpath(os.readlink('/proc/%d/cwd' % pid))
    except (OSError, ValueError):
        pass
    try:
        out = run(['lsof', '-b', '-w', '-a', '-d', 'cwd', '-p', str(pid), '-Fn'])
    except Exception:
        return None
    line = next((l for l in out.splitlines() if l.startswith('n/')), None)
    return os.path.realpath(line[1:]) if line else None


THREAD_TTL = 7 * 86400 * 1000


def daemon_target(agent, server, panes, procs, payload):
    """The TUI a daemon-run hook belongs to, or None when it cannot be told apart.

    The daemon's own environment names whichever pane started it, so TMUX_PANE and the
    process tree say nothing about the thread. A thread keeps the TUI it was first bound to.
    An unbound thread binds only to the single Codex TUI whose working directory is the
    thread's `cwd`. Several TUIs in that directory, none, or no thread ID: the hook is
    dropped rather than shown in the wrong pane.
    """
    thread = payload.get('session_id')
    if not isinstance(thread, str) or not thread or len(thread) > 256:
        return None
    candidates = [c for c in pane_clis(server, panes, procs) if procs[c[1]]['agent'] == agent]
    if not candidates:
        return None
    runs = {c[2]: c for c in candidates}
    key = digest('thread:' + thread)
    now = now_ms()
    with closing(database()) as db, db:
        db.execute('DELETE FROM threads WHERE at < ?', (now - THREAD_TTL,))
        row = db.execute('SELECT run FROM threads WHERE thread=?', (key,)).fetchone()
    chosen = runs.get(row[0]) if row else None
    if chosen is None:
        cwd = payload.get('cwd')
        if not isinstance(cwd, str) or not cwd.startswith('/'):
            return None
        want = os.path.realpath(cwd)
        # Outside any transaction: lsof may be slow and other hooks must not wait on the lock.
        matches = [c for c in candidates if process_cwd(c[1]) == want]
        if len(matches) != 1:
            return None
        chosen = matches[0]
    with closing(database()) as db, db:
        db.execute('INSERT OR REPLACE INTO threads VALUES(?,?,?)', (key, chosen[2], now))
    pane, pid, _ = chosen
    return server, pane, procs, pid


def resolve(agent, tmux, payload=None):
    """(server, pane, procs, pid) of the CLI process that ran this hook or statusline, or None."""
    server, panes, procs = topology(tmux)
    pane = next((p for p in panes if p['pane'] == os.environ.get('TMUX_PANE')), None)
    pid = os.getppid()
    seen = set()
    while pid in procs and pid not in seen:
        seen.add(pid)
        if procs[pid].get('daemon') == agent:
            if not procs[pid].get('managed') or payload is None:
                return None
            return daemon_target(agent, server, panes, procs, payload)
        if procs[pid]['agent']:
            # Stop at the nearest identified CLI, including a different vendor's CLI.
            if pane is None or procs[pid]['agent'] != agent or not descendant(pid, pane['pid'], procs):
                return None
            return server, pane, procs, pid
        pid = procs[pid]['parent']
    return None


def record_payload(agent, tmux, event, payload):
    status_only = event in STATUS_EVENTS.get(agent, [])
    # A daemon-run Codex hook may lack TMUX; resolve() still requires TMUX_PANE for every other route.
    if (event not in EVENTS[agent] and not status_only) or (not os.environ.get('TMUX') and agent != 'Codex'):
        return
    event_kind = None if status_only else kind(agent, event, payload)
    if event_kind is None and not status_only:
        return
    # The global AGY file is shared with its App/IDE, which must not report into a CLI.
    if agent in ('Agy', 'Grok'):
        identity = payload.get('conversationId' if agent == 'Agy' else 'sessionId')
        if not isinstance(identity, str) or not identity:
            return
        if agent == 'Agy' and '/antigravity-cli/brain/' not in payload.get('transcriptPath', ''):
            return
    resolved = resolve(agent, tmux, payload)
    if resolved is None:
        return
    server, pane, procs, pid = resolved
    run_key = run_id(server, pane['pane'], procs[pid]['token'])
    child = payload.get('agent_id') or payload.get('subagentType')
    actor = digest(str(child)) if child else 'parent'
    if event.startswith('Subagent') and actor == 'parent':
        actor = 'unidentified-child'
    now = now_ms()
    status = collect_hook_status(agent, event, payload, now)
    data = {k: pane[k] for k in ('session', 'window', 'pane')}
    data.update(id=uuid.uuid4().hex, server=server, run=run_key, actor=actor,
                agent=agent, kind=event_kind, event=event, at=now)
    if agent == 'Claude' and event == 'SessionStart':
        prune_mod_files(now)
    with database() as db:
        conversation = payload.get('conversationId') if agent == 'Agy' else payload.get('sessionId') if agent == 'Grok' else payload.get('session_id')
        if conversation:
            conversation = digest(str(conversation))
            primary = db.execute('SELECT conversation FROM conversations WHERE run=?', (run_key,)).fetchone()
            if primary is None and actor == 'parent' and not status_only:
                # AGY supplies no parent ID. Establish its root at SessionStart only.
                if agent == 'Agy' and event != 'SessionStart':
                    return
                db.execute('INSERT INTO conversations VALUES(?,?)', (run_key, conversation))
            elif primary and conversation != primary[0]:
                actor = digest('conversation:' + conversation)
                data['actor'] = actor
        if agent == 'Grok' and payload.get('subagentType'):
            data['actor'] = actor = digest('child:' + str(payload.get('sessionId', child)))
        if status_only:
            # Status only: no event, no latest row, no notification, no `supported` change.
            upsert_status(db, run_key, actor, agent, status, now)
            return
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
        upsert_status(db, run_key, actor, agent, status, now)
        db.execute('DELETE FROM events WHERE seq <= ?', (seq - LIMIT,))
        db.execute('DELETE FROM latest WHERE (run,actor) NOT IN (SELECT run,actor FROM events)')
        db.execute('DELETE FROM conversations WHERE run NOT IN (SELECT run FROM events)')


def record_statusline(agent, tmux, payload):
    if not os.environ.get('TMUX'):
        return
    status = status_from_statusline(agent, payload)
    if not status:
        return
    resolved = resolve(agent, tmux)
    if resolved is None:
        return
    server, pane, procs, pid = resolved
    with database(timeout=0.2) as db:
        upsert_status(db, run_id(server, pane['pane'], procs[pid]['token']), 'parent', agent, status)


def relayable(original):
    """The user's own statusline command, when it is one that would have run."""
    if not isinstance(original, dict) or owned_tee(original) or original.get('enabled') is False:
        return None
    command = original.get('command')
    if original.get('type', 'command') != 'command' or not isinstance(command, str) or not command.strip():
        return None
    return command


def statusline(agent, tmux, umask=None):
    """Tee: record a whitelist of the statusline input, then run the saved original command
    with the same stdin bytes, environment and directory. Returns (exit code, stdout bytes)."""
    raw = sys.stdin.buffer.read(INPUT_LIMIT + 1)
    if len(raw) > INPUT_LIMIT:
        raw += sys.stdin.buffer.read(16 * INPUT_LIMIT)  # relayed unread, never recorded
    if agent != 'Agy':
        return 0, b''  # only AGY's statusline is wrapped
    try:
        command = relayable(read_saved(agent))
    except Exception:
        command = None
    child = None
    if command:
        previous = os.umask(umask) if umask is not None else None
        try:
            child = subprocess.Popen(['/bin/sh', '-c', command], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     start_new_session=True)
        except OSError:
            child = None
        finally:
            if previous is not None:
                os.umask(previous)
    if len(raw) <= INPUT_LIMIT:
        try:
            record_statusline(agent, tmux, json.loads(raw))
        except Exception:
            pass  # Never block or fail the user's statusline.
    if child is None:
        return 0, b''
    return relay(child, raw, TEE_TIMEOUT)


def relay(child, raw, timeout):
    """Feed stdin, collect stdout and the exit code of `sh` within one deadline. A background job
    of the original that keeps stdout open neither delays the exit code nor outlives the timeout."""
    def feed():
        try:
            child.stdin.write(raw)
        except (OSError, ValueError):
            pass  # The original need not read its input.
        finally:
            try:
                child.stdin.close()
            except (OSError, ValueError):
                pass

    threading.Thread(target=feed, daemon=True).start()
    deadline = time.monotonic() + timeout
    chunks, code, exited_at = [], None, None
    with selectors.DefaultSelector() as selector:
        selector.register(child.stdout, selectors.EVENT_READ)
        while True:
            now = time.monotonic()
            if code is None:
                code = child.poll()
                if code is not None:
                    exited_at = now
            if code is not None and now - exited_at >= TEE_DRAIN:
                break  # `sh` finished; a lingering background job keeps its own output.
            if now >= deadline:
                break
            limit = deadline - now if code is None else TEE_DRAIN - (now - exited_at)
            if not selector.select(min(limit, 0.05)):
                continue
            chunk = os.read(child.stdout.fileno(), 65536)
            if not chunk:
                if code is None:
                    try:
                        code = child.wait(max(0.0, deadline - time.monotonic()))
                    except subprocess.TimeoutExpired:
                        pass
                break
            chunks.append(chunk)
    child.stdout.close()
    if code is None:
        try:
            os.killpg(child.pid, signal.SIGKILL)  # the whole session of the original, grandchildren too
        except OSError:
            pass
        try:
            child.wait(0.5)
        except subprocess.TimeoutExpired:
            pass
        return 124, b''.join(chunks)
    return code, b''.join(chunks)


def poll(tmux, session, after, epoch):
    server, panes, procs = topology(tmux, session)
    observations = []
    pids = {}
    for pane, pid, run_key in pane_clis(server, panes, procs):
        # The pid stays internal: it only maps a Claude mod file to its pane.
        pids[run_key] = pid
        observations.append(dict(session=pane['session'], window=pane['window'], pane=pane['pane'],
                                 run=run_key, agent=procs[pid]['agent'],
                                 actor='parent', last=None, supported=[]))
    result = dict(server=server, epoch='', cursor=0, gap=False, observations=observations, events=[])
    statuses = {}
    if DB.exists():
        # Read-only polling never creates a helper directory or a database.
        with closing(sqlite3.connect(DB.as_uri() + '?mode=ro', uri=True, timeout=0.8)) as db:
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
            try:
                # A database written by an older helper has no status table yet.
                for run_key, actor, encoded in db.execute('SELECT run,actor,data FROM status'):
                    if run_key in runs:
                        try:
                            statuses[(run_key, actor)] = json.loads(encoded)
                        except ValueError:
                            pass
            except sqlite3.OperationalError:
                pass
    now = now_ms()
    for obs in observations:
        mod = {}
        if obs['agent'] == 'Claude' and obs['actor'] == 'parent':
            pid = pids.get(obs['run'])
            mod = claude_mod_file(pid, now, procs.get(pid))
        status = wire_status(statuses.get((obs['run'], obs['actor'])), mod)
        if status:
            obs['status'] = status
    return result


# ---------------------------------------------------------------------------
# Configuration.

def config_paths():
    # Honor the CLI's documented custom user-config directories.
    return {'Codex': Path(os.environ.get('CODEX_HOME', str(Path.home() / '.codex'))) / 'hooks.json',
            'Claude': Path(os.environ.get('CLAUDE_CONFIG_DIR', str(Path.home() / '.claude'))) / 'settings.json',
            'Agy': Path.home() / '.gemini/config/hooks.json',
            'Grok': Path(os.environ.get('GROK_HOME', str(Path.home() / '.grok'))) / 'hooks/dgl-observer-v1.json'}


def status_paths():
    # Files changed by status collection beyond config_paths(). Claude's mod entry lives
    # in the same settings.json as its hooks.
    return {'Agy': Path.home() / '.gemini/antigravity-cli/settings.json'}


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


def load_settings(path):
    if path.is_symlink():
        raise ValueError('symlink configuration is not supported')
    raw = path.read_bytes() if path.exists() else b''
    if len(raw) > 4 * 1024 * 1024:
        raise ValueError('configuration is too large')
    data = json.loads(raw) if raw.strip() else {}
    if not isinstance(data, dict):
        raise ValueError('invalid settings')
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
                (len(args) == 7 or (args[7] == '--event' and args[8] in EVENTS[args[4]] + STATUS_EVENTS.get(args[4], []))))
    except (ValueError, AttributeError):
        return False


def tee_command(tmux):
    args = [sys.executable, str(HELPER), 'statusline', '--agent', 'Agy', '--tmux', tmux]
    return ' '.join(shlex.quote(v) for v in args)


def owned_tee(value):
    try:
        args = shlex.split(value.get('command', ''))
        return (len(args) == 7 and args[1:5] == [str(HELPER), 'statusline', '--agent', 'Agy'] and
                args[5] == '--tmux')
    except (ValueError, AttributeError):
        return False


def standard_transform(data, agent, tmux, install, events=None):
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
        for event in (hook_events(agent) if events is None else events):
            hooks.setdefault(event, []).append({'hooks': [{'type': 'command', 'command': hook_command(agent, tmux), 'timeout': 3}]})
    if not hooks:
        result.pop('hooks', None)
    return result


class HookAdapter:
    experimental = False

    def load(self, path):
        return load_config(path)

    def transform(self, data, agent, tmux, install, events=None):
        return standard_transform(data, agent, tmux, install, events)

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

    def transform(self, data, agent, tmux, install, events=None):
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
            for event in (EVENTS[agent] if events is None else events):
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


def transformed(data, agent, tmux, install, events=None):
    return ADAPTERS[agent].transform(data, agent, tmux, install, events)


# ---------------------------------------------------------------------------
# Detailed status collection: the Claude Code mod and the AGY statusline tee.

def mod_dir():
    return ROOT / 'claude-mod'


def saved_path(agent):
    return ROOT / ('claude-mod.json' if agent == 'Claude' else 'statusline-%s.json' % agent)


def read_saved(agent):
    """What install --status found before changing a setting; MISSING when nothing is saved."""
    path = saved_path(agent)
    if path.is_symlink():
        raise ValueError('symlink state is not supported')
    if not path.exists():
        return MISSING
    raw = read_limited(path, 4 * 1024 * 1024)
    if raw is None:
        raise ValueError('invalid saved state')
    return json.loads(raw)


def plugin_dirs(data):
    env = data.get('env', {})
    if not isinstance(env, dict):
        raise ValueError('invalid env')
    value = env.get(PLUGIN_DIRS)
    if value is None:
        return []
    if not isinstance(value, str):
        raise ValueError('invalid plugin directories')
    return value.split(':') if value else []


def claude_status(data, want):
    """Adds or removes the mod's folder in env.CLAUDE_CODE_PLUGIN_DIRS, keeping other entries.
    Returns (new data, saved-state action)."""
    folder = str(mod_dir())
    entries = plugin_dirs(data)
    installed = folder in entries
    saved = read_saved('Claude')
    if want:
        if installed or saved is not MISSING:
            return data, None  # installed already, or replaced by the user: never re-added
        result = copy.deepcopy(data)
        env = result.setdefault('env', {})
        original = env.get(PLUGIN_DIRS)
        env[PLUGIN_DIRS] = original + ':' + folder if original else folder
        return result, ('write', {'env': 'env' in data, 'value': original})
    if not installed:
        return data, (('delete',) if saved is not MISSING else None)
    result = copy.deepcopy(data)
    env = result['env']
    remaining = [e for e in entries if e != folder]
    prior = saved if isinstance(saved, dict) else {}
    if remaining:
        env[PLUGIN_DIRS] = ':'.join(remaining)
    elif prior.get('value') == '':
        env[PLUGIN_DIRS] = ''
    else:
        env.pop(PLUGIN_DIRS)
    if not env and not prior.get('env'):
        result.pop('env')
    return result, (('delete',) if saved is not MISSING else None)


def agy_status(data, tmux, want):
    """Wraps or restores AGY's statusLine. Returns (new data, saved-state action)."""
    current = data.get('statusLine', MISSING)
    installed = isinstance(current, dict) and owned_tee(current)
    saved = read_saved('Agy')
    if want:
        if installed:
            line = dict(current, command=tee_command(tmux))
            return (data, None) if line == current else (dict(copy.deepcopy(data), statusLine=line), None)
        if saved is not MISSING:
            return data, None  # replaced by the user: never re-wrapped automatically
        original = None if current is MISSING else current
        line = {'type': 'command', 'command': tee_command(tmux)}
        if relayable(original):
            line.update({k: original[k] for k in ('padding', 'stack_with_default') if k in original})
        else:
            line['stack_with_default'] = True
        return dict(copy.deepcopy(data), statusLine=line), ('write', original)
    if not installed:
        return data, (('delete',) if saved is not MISSING else None)
    result = copy.deepcopy(data)
    if saved is MISSING or saved is None:
        result.pop('statusLine')
    else:
        result['statusLine'] = saved
    return result, (('delete',) if saved is not MISSING else None)


def status_collection(agent, data, supported):
    result = dict(supported=False, installed=False, replaced=False, kind=STATUS_KIND[agent])
    try:
        if agent == 'Claude' and data is not None:
            installed = str(mod_dir()) in plugin_dirs(data)
            result.update(supported=supported, installed=installed,
                          replaced=read_saved('Claude') is not MISSING and not installed)
        elif agent == 'Agy':
            _, settings = load_settings(status_paths()['Agy'])
            current = settings.get('statusLine')
            installed = isinstance(current, dict) and owned_tee(current)
            result.update(supported=supported, installed=installed,
                          replaced=read_saved('Agy') is not MISSING and not installed)
    except Exception:
        result.update(supported=False)
    return result


def write_private(path, raw):
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.is_symlink():
        raise ValueError('symlink state is not supported')
    if path.exists() and path.read_bytes() == raw:
        return
    fd, name = tempfile.mkstemp(prefix='.dgl-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as f:
            f.write(raw)
            f.flush()
            os.fsync(f.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def remove_mod():
    folder = mod_dir()
    if folder.is_symlink() or not folder.is_dir():
        return
    manifest = read_json(folder / '.claude-plugin/plugin.json', 64 * 1024)
    # Only a folder this helper generated, including types the engine laid beside it.
    if isinstance(manifest, dict) and manifest.get('name') == MOD_NAME:
        shutil.rmtree(folder)


MOD_SOURCE = r'''import type { EngineInterface, Register, SessionRateLimit } from 'claude-code'

// Generated by the dgl agent observer at installation; a reinstall rewrites it.
// It writes token counts, percentages, model ids, limit windows and hashes,
// never prompts, paths, names or addresses, to one file per Claude Code
// process. Every step is wrapped so that a failure here never reaches the
// session.
const DIR: string = __DGL_DIR__

type Usage = { used?: number; size?: number; percent?: number }
type Part = { name?: string; tokens: number; kind: string }
type Parts = { breakdown: Part[]; breakdown_total?: number; breakdown_window?: number; compact_percent?: number }
type Cache = { input?: number; read?: number; write?: number; output?: number; ttl?: string; last_at?: number }
type Model = { id?: string; name?: string }
type Limit = { id: string; used_percent?: number; resets_at?: number }
type Account = { kind?: string; plan?: string; key?: string }
type State = {
  context?: Usage; parts?: Parts; cache?: Cache; model?: Model
  limits?: Limit[]; cost?: number; account?: Account
  modelAt?: number
}

const KINDS: Record<string, string> = {
  'claude.ai': 'subscription', 'oauth-token': 'subscription', console: 'api_key',
  'api-key': 'api_key', bedrock: 'cloud', vertex: 'cloud', foundry: 'cloud',
}

const finite = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const bounded = (value: unknown, length: number): string | undefined =>
  typeof value === 'string' && value.length > 0 && value.length <= length ? value : undefined

const seconds = (iso: string | undefined): number | undefined => {
  const ms = iso === undefined ? NaN : Date.parse(iso)
  return Number.isFinite(ms) ? Math.floor(ms / 1000) : undefined
}

const sha256 = async (value: string): Promise<string> => {
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))
  return Array.from(new Uint8Array(hash), b => b.toString(16).padStart(2, '0')).join('')
}

const limitsOf = (rows: readonly SessionRateLimit[]): Limit[] =>
  rows.slice(0, 16).flatMap(row => {
    const id = bounded(row.kind, 64)
    return id === undefined ? [] : [{ id, used_percent: finite(row.percentUsed), resets_at: seconds(row.resetsAt) }]
  })

// One process, one session at a time: a module-level record of what was measured.
const state: State = {}
const memo = { pid: 0, identified: false, written: 0, pending: false }

async function identify($: EngineInterface): Promise<void> {
  if (memo.identified) return
  memo.identified = true
  try {
    const who = await $.session.identity()
    // A refusal is an answer, not a rejection: it leaves the account to the fallback below.
    if (who.deny !== undefined) return
    const org = bounded(who.organizationType, 64)
    const sign = typeof who.kind === 'string' ? who.kind : 'none'
    let kind = KINDS[sign] ?? (sign === 'none' ? undefined : 'other')
    if (org === 'claude_enterprise') kind = 'enterprise'
    const plan = org === undefined ? undefined : org.replace(/^claude_/, '')
    const id = bounded(who.organizationId, 256)
    const key = id === undefined ? undefined : await sha256(`account:Claude:org:${id}`)
    state.account = { kind, plan, key }
  } catch {
    // A user-tier mod may be refused the identity; the limits imply a subscription.
  }
}

async function flush($: EngineInterface): Promise<void> {
  if (memo.pid === 0) memo.pid = Number((await $.process.hookEnv()).CLAUDE_PID) || -1
  if (memo.pid < 0) return
  const now = await $.clock.now()
  const wait = memo.written + 1000 - now
  if (wait > 0) {
    if (!memo.pending) {
      memo.pending = true
      $.clock.after(wait, () => {
        memo.pending = false
        flush($).catch(() => undefined)
      })
    }
    return
  }
  memo.written = now
  const session = await sha256(await $.session.id())
  const context = state.context || state.parts ? { ...state.context, ...state.parts } : undefined
  // Without a known kind (refused, signed out or unknown), rate limits imply a subscription.
  const account = state.account?.kind === undefined && state.limits?.length
    ? { ...state.account, kind: 'subscription' } : state.account
  const status = {
    model: state.model, context, cache: state.cache, cost_usd: state.cost, account, limits: state.limits,
  }
  const pid = memo.pid
  await $.fs.write(`${DIR}/${pid}.json`, JSON.stringify({ v: 1, pid, session, at: now, model_at: state.modelAt, status }))
}

export const register: Register = on => {
  on('session.measure', async ($, e, next) => {
    const result = await next(e)
    try {
      state.context = { used: finite(e.context.tokens), size: finite(e.context.window), percent: finite(e.context.percent) }
      state.limits = limitsOf(e.rateLimits)
      state.cost = finite(e.cost?.usd)
      await identify($)
      await flush($)
    } catch {
      // Never disturb the session.
    }
    return result
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.agentId !== undefined) return result
    try {
      const now = await $.clock.now()
      const turn = e.usage
      if (turn !== undefined) {
        const id = bounded(turn.model, 128)
        state.model = { ...(state.model?.id === id ? state.model : {}), id }
        // Measures flush between turns: the model's own sample time keeps a later /model switch.
        state.modelAt = now
        const long = turn.ephemeral_1h_input_tokens ?? 0
        const short = turn.ephemeral_5m_input_tokens ?? 0
        const ttl = long > 0 ? '1h' : short > 0 ? '5m' : state.cache?.ttl
        state.cache = {
          input: turn.input_tokens, read: turn.cache_read_input_tokens,
          write: turn.cache_creation_input_tokens, output: turn.output_tokens, ttl, last_at: now,
        }
      }
      const usage = await $.session.usage({ breakdown: 'summary' })
      state.context = {
        used: finite(usage.context.tokens), size: finite(usage.context.window), percent: finite(usage.context.percent),
      }
      state.limits = limitsOf(usage.rateLimits)
      state.cost = finite(usage.cost?.usd)
      const b = usage.context.breakdown
      if (b !== undefined) {
        const window = finite(b.rawMaxTokens)
        const threshold = finite(b.autoCompactThreshold)
        state.parts = {
          // Category rows only: the memory file, MCP tool, agent and skill lists stay behind.
          breakdown: b.categories.slice(0, 24).map(c => ({ name: bounded(c.name, 64), tokens: c.tokens, kind: c.kind })),
          breakdown_total: finite(b.totalTokens),
          breakdown_window: window,
          compact_percent: b.isAutoCompactEnabled && threshold !== undefined && window
            ? Math.min(100, (100 * threshold) / window) : undefined,
        }
        const name = bounded(b.model, 64)
        if (name !== undefined) state.model = { ...state.model, name }
        // The live window's last response, rather than the turn's summed requests.
        const last = b.apiUsage
        if (last !== null && state.cache !== undefined) {
          state.cache = {
            ...state.cache, input: last.input_tokens, read: last.cache_read_input_tokens,
            write: last.cache_creation_input_tokens, output: last.output_tokens,
          }
        }
      }
      await identify($)
      await flush($)
    } catch {
      // Never disturb the session.
    }
    return result
  })

  on('session.end', async ($, e, next) => {
    const result = await next(e)
    try {
      if (e.reason === 'clear') {
        state.context = undefined
        state.parts = undefined
        state.cache = undefined
        await flush($)
      }
    } catch {
      // Never disturb the session.
    }
    return result
  })
}
'''


def mod_files():
    """The generated Claude Code mod: a plugin folder whose module writes to ROOT/claude."""
    manifest = {'name': MOD_NAME, 'version': '1.0.0', 'author': {'name': 'dgl'},
                'description': 'Reports context, usage, cache and model figures of this Claude Code process '
                               'to the dgl agent observer. Writes no prompts, paths or addresses.'}
    source = MOD_SOURCE.replace('__DGL_DIR__', json.dumps(str(ROOT / 'claude')))
    return {'.claude-plugin/plugin.json': (json.dumps(manifest, indent=2) + '\n').encode(),
            'hooks/hooks.json': (json.dumps({'modules': ['./register.ts']}, indent=2) + '\n').encode(),
            'hooks/register.ts': source.encode()}


def inspect(tmux):
    if sys.version_info < (3, 9):
        raise ValueError('Python 3.9 or newer required')
    configs = []
    token_parts = []
    for agent, path in config_paths().items():
        adapter = ADAPTERS[agent]
        error, version, installed, supported, data = '', 'not installed', False, False, None
        try:
            raw, data = adapter.load(path)
            executable = shutil.which(agent.lower())
            version = run([executable, '--version']) if executable else 'not installed'
            supported = bool(executable) and (adapter.experimental or release_supported(agent, version))
            transformed(data, agent, tmux, supported, hook_events(agent, version))
            installed = any(owned(h) for h in adapter.handlers(data))
        except Exception:
            supported, data = False, None
            # A code: the app owns the wording and its translations.
            error = 'config'
            # Keep the conflict token sensitive to malformed-file changes too.
            try:
                raw = path.read_bytes() if path.is_file() and not path.is_symlink() else b''
            except OSError:
                raw = b''
        configs.append(dict(agent=agent, path=str(path), version=version, supported=supported,
                            installed=installed, experimental=adapter.experimental, error=error,
                            capabilities=CAPABILITIES[agent],
                            events=hook_events(agent, version) if supported else [],
                            status_collection=status_collection(agent, data, supported)))
        token_parts.append(str(path) + ':' + digest(raw.hex()))
    for path in list(status_paths().values()) + [saved_path('Claude'), saved_path('Agy')]:
        try:
            raw = path.read_bytes() if path.is_file() and not path.is_symlink() else b''
        except OSError:
            raw = b''
        token_parts.append(str(path) + ':' + digest(raw.hex()))
    helper_hash = digest(HELPER.read_bytes().hex()) if HELPER.exists() else ''
    token = digest('|'.join(token_parts) + helper_hash + str(configs))
    return dict(token=token, configs=configs, python=sys.version.split()[0], tmux=run([tmux, '-V']),
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


def encode(data):
    return (json.dumps(data, ensure_ascii=False, indent=2) + '\n').encode()


def configure(tmux, token, install, source, status=False):
    ROOT.mkdir(mode=0o700, parents=True, exist_ok=True)
    lock = ROOT / 'install.lock'
    fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        preview = inspect(tmux)
        if token != preview['token']:
            raise ValueError('configuration changed; inspect again')
        if install and not any(c['supported'] for c in preview['configs']):
            raise ValueError('unsupported CLI version')
        changes, saved, write_mod, drop_mod = [], [], False, False
        configs = {c['agent']: c for c in preview['configs']}
        for c in preview['configs']:
            if c['error'] or (install and not c['supported']):
                continue
            path = Path(c['path'])
            raw, data = ADAPTERS[c['agent']].load(path)
            new = transformed(data, c['agent'], tmux, install and c['supported'], c['events'])
            if c['agent'] == 'Claude':
                want = install and status and c['status_collection']['supported']
                try:
                    new, action = claude_status(new, want)
                except ValueError:
                    pass  # An unreadable env leaves the mod entry as it is.
                else:
                    if action:
                        saved.append(('Claude', action, path))
                    listed = str(mod_dir()) in plugin_dirs(new)
                    write_mod, drop_mod = want and listed, not listed
            if data != new:
                changes.append((path, raw, encode(new)))
        agy = configs.get('Agy')
        if agy and 'Agy' in status_paths():
            want = install and status and agy['supported'] and agy['status_collection']['supported']
            path = status_paths()['Agy']
            try:
                raw, data = load_settings(path)
                new, action = agy_status(data, tmux, want)
            except ValueError:
                pass  # Unreadable settings are never rewritten.
            else:
                if action:
                    saved.append(('Agy', action, path))
                if data != new:
                    changes.append((path, raw, encode(new)))
        # Recheck after preparing all changes; a writer may have changed a config
        # between the preview comparison and load_config above.
        if inspect(tmux)['token'] != token:
            raise ValueError('configuration changed; inspect again')
        if install:
            old = HELPER.read_bytes() if HELPER.exists() else b''
            atomic_write(HELPER, source.encode(), old)
        landed = set()
        try:
            if write_mod:
                for name, raw in mod_files().items():
                    write_private(mod_dir() / name, raw)
            # The original is kept before a setting changes, and dropped once it is restored.
            for agent, action, _ in saved:
                if action[0] == 'write':
                    write_private(saved_path(agent), (json.dumps(action[1], ensure_ascii=False) + '\n').encode())
            for path, old, new in changes:
                atomic_write(path, new, old)
                landed.add(path)
        except BaseException:
            # A saved original whose setting never changed would read as "replaced by the user"
            # and block every later wrap, so it goes with the failed change.
            for agent, action, path in saved:
                if action[0] == 'write' and path not in landed:
                    with contextlib.suppress(OSError):
                        saved_path(agent).unlink()
                    if agent == 'Claude':
                        with contextlib.suppress(OSError):
                            remove_mod()
            raise
        for agent, action, _ in saved:
            if action[0] == 'delete' and saved_path(agent).exists():
                saved_path(agent).unlink()
        if drop_mod:
            remove_mod()
        if not install and DB.exists():
            # Keep bounded history, but don't present a removed integration's old
            # observations as live. A later actual hook can establish a new observation.
            with database() as db:
                db.execute('DELETE FROM latest')
                db.execute('DELETE FROM status')
                db.execute('DELETE FROM threads')
                db.execute("UPDATE meta SET value=? WHERE key='epoch'", (uuid.uuid4().hex,))
        return inspect(tmux)
    finally:
        os.close(fd)
        lock.unlink()


def serve(tmux):
    # One process per connection answers every poll, so the source and the login shell are
    # sent once. Replies start with RS so shell start-up output is never taken for one.
    while True:
        line = sys.stdin.readline()
        if not line:
            return
        try:
            request = json.loads(line)
            result = poll(tmux, request['session'], int(request['after']), request['epoch'])
        except Exception:
            result = {'error': 'failed'}
        sys.stdout.write('\x1e' + json.dumps(result, ensure_ascii=False) + '\n')
        sys.stdout.flush()


def main(source=''):
    umask = os.umask(0o077)
    p = argparse.ArgumentParser()
    p.add_argument('action', choices=['serve', 'record', 'statusline', 'inspect', 'install', 'remove'])
    p.add_argument('--tmux', required=True)
    p.add_argument('--agent', choices=list(EVENTS))
    p.add_argument('--event')
    p.add_argument('--token', default='')
    p.add_argument('--status', action='store_true')
    args = p.parse_args()
    try:
        if args.action == 'record':
            event = record(args.agent, args.tmux, args.event)
            print(json.dumps(ADAPTERS[args.agent].neutral(event if isinstance(event, str) else args.event)))
            return
        if args.action == 'statusline':
            code, out = statusline(args.agent, args.tmux, umask)
            sys.stdout.flush()
            sys.stdout.buffer.write(out)
            sys.stdout.buffer.flush()
            if code:
                sys.exit(code)
            return
        if args.action == 'serve':
            serve(args.tmux)
            return
        if args.action == 'inspect':
            result = inspect(args.tmux)
        else:
            result = configure(args.tmux, args.token, args.action == 'install',
                               source or Path(__file__).read_text(), args.status)
        print(json.dumps(result, ensure_ascii=False))
    except SystemExit:
        raise
    except Exception:
        if args.action == 'record':
            print(json.dumps(ADAPTERS[args.agent].neutral(args.event)) if args.agent in ADAPTERS else '{}')
        elif args.action == 'statusline':
            return  # A statusline shows nothing rather than an error object.
        else:
            # Never echo configuration contents, payloads, commands or secret-bearing exceptions.
            print(json.dumps({'error': 'failed'}))
            sys.exit(1)


if __name__ == '__main__':
    main()
