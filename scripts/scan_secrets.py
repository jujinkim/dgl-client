#!/usr/bin/env python3
"""Scan the public working tree, full index and all refs without logging secret values."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
if not shutil.which('gitleaks'):
    raise SystemExit('Install Gitleaks 8.30.1 before running this check.')

def scan(args, data=None):
    with tempfile.TemporaryDirectory(prefix='dgl-scan-report-') as reports:
        report = Path(reports) / 'redacted.json'
        result = subprocess.run(['gitleaks', *args, '--redact=100', '--no-banner', '--report-format=json', '--report-path', str(report), '--config', str(root / '.gitleaks.toml')],
                                input=data, cwd=root, capture_output=True)
        # Only rule and location are emitted, never values or surrounding source.
        if result.returncode:
            if report.exists():
                for finding in json.loads(report.read_text(encoding="utf-8")):
                    print(f"{finding['RuleID']}: {finding['File']}:{finding['StartLine']}")
            raise SystemExit('Secret scan failed (' + args[0] + '). Findings are redacted.')

paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root).split(b'\0')
with tempfile.TemporaryDirectory(prefix='dgl-public-scan-') as directory:
    for encoded in paths:
        if not encoded:
            continue
        relative = Path(encoded.decode())
        source = root / relative
        if source.is_file() and not source.is_symlink():
            target = Path(directory) / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
    scan(['dir', directory])
# Preserve index paths so narrowly scoped upstream false-positive rules still apply.
# Scanning the complete staged snapshot is stronger than scanning only a diff.
with tempfile.TemporaryDirectory(prefix='dgl-index-scan-') as directory:
    subprocess.run(['git', 'checkout-index', '--all', '--prefix=' + directory + os.sep], cwd=root, check=True)
    scan(['dir', directory])
scan(['git', '.', '--log-opts=--all'])
print('Gitleaks: public working tree, full staged snapshot, and all Git refs passed (redacted).')
