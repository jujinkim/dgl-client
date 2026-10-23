#!/usr/bin/env python3
import hashlib
from pathlib import Path
root = Path(__file__).resolve().parents[1]
expected = (root / 'KICKOFF_PROMPT.sha256').read_text(encoding="utf-8").split()[0]
actual = hashlib.sha256((root / 'KICKOFF_PROMPT.md').read_bytes()).hexdigest()
if actual != expected:
    raise SystemExit('Original kickoff prompt checksum changed.')
print('Kickoff prompt SHA-256 unchanged.')
