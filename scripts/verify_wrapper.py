#!/usr/bin/env python3
"""Reject an unexpected Gradle bootstrap JAR before running Java."""
import hashlib
from pathlib import Path

root = Path(__file__).resolve().parents[1]
expected = '7a9ce74cff467ca1bf60a4fcd9f05185acceda4d0f382434d393e17864262c5d'
actual = hashlib.sha256((root / 'gradle/wrapper/gradle-wrapper.jar').read_bytes()).hexdigest()
if actual != expected:
    raise SystemExit('Gradle 9.7.0 wrapper checksum mismatch')
properties = dict(line.split('=', 1) for line in (root / 'gradle/wrapper/gradle-wrapper.properties').read_text(encoding="utf-8").splitlines() if '=' in line)
if properties.get('distributionUrl') != r'https\://services.gradle.org/distributions/gradle-9.7.0-bin.zip':
    raise SystemExit('Unexpected Gradle distribution URL/version')
if properties.get('distributionSha256Sum') != '84fbba45c7f4c64abc77460e1c00f541e9f960e3c7ed2538f1ede19eacd873ae':
    raise SystemExit('Gradle distribution checksum pin mismatch')
print('Gradle 9.7.0 wrapper checksum passed')
