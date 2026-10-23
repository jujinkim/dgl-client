"""Small, explicit report format shared by CI verifiers (no raw logs/secrets)."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def sha256(path):
    with Path(path).open('rb') as stream:
        digest = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
        return digest.hexdigest()


def context():
    return {'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT,
                                             text=True, encoding='utf-8').strip(),
            'runId': os.environ.get('GITHUB_RUN_ID'), 'runAttempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
            'os': platform.system(), 'osVersion': platform.version(), 'machine': platform.machine()}


def write_report(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def safe_relative(name):
    # Reject Windows paths even when a report is consumed on Unix.
    if not isinstance(name, str) or not name or '\\' in name or ':' in name or name.startswith('/') or any(
            part in {'', '.', '..'} for part in name.split('/')):
        raise ValueError('Unsafe manifest path')
    return name


def require_passed(report, expected_context=None):
    if report.get('schema') != 1 or report.get('complete') is not True or report.get('status') != 'passed':
        raise ValueError('Incomplete or failed verification report')
    ctx = report.get('context', {})
    if not re.fullmatch('[0-9a-f]{40}', ctx.get('commit', '')):
        raise ValueError('Missing report commit')
    if expected_context and any(ctx.get(k) != expected_context.get(k) for k in ('commit', 'runId', 'runAttempt')):
        raise ValueError('Verification report is from a different commit/run')
