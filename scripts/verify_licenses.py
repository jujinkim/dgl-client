#!/usr/bin/env python3
"""Offline, fail-closed coverage check for selected runtime license materials."""
import hashlib
import json
from pathlib import Path
import sys
from audit_dependencies import ROOT, combine, load_inventory, npm_packages


def check_file(record, root):
    path = root / record['path']
    if not path.is_file() or not path.stat().st_size or hashlib.sha256(path.read_bytes()).hexdigest() != record['sha256']:
        raise ValueError('Missing or modified license evidence: ' + record['path'])


def verify(packages, registry, root=ROOT):
    if registry.get('schema') != 1:
        raise ValueError('Invalid license registry')
    records = {(p['ecosystem'], p['name'], p['version']): p for p in registry['packages']}
    checked = 0
    for package in packages:
        if not any(u['scope'] == 'runtime' for u in package['usages']):
            continue
        key = tuple(package[k] for k in ('ecosystem', 'name', 'version'))
        record = records.get(key)
        if not record or not record.get('licenses') or not record.get('evidence'):
            raise ValueError('Missing runtime license: ' + str(key))
        if not record.get('texts') and not any(e['kind'] == 'license' for e in record['evidence']):
            raise ValueError('Missing license text: ' + str(key))
        for artifact in package['artifacts']:
            if artifact not in record['artifacts']:
                raise ValueError('Unreviewed runtime artifact: ' + str(key))
        for evidence in record['evidence']:
            if not evidence.get('source', '').startswith('https://'):
                raise ValueError('Missing evidence provenance')
            check_file(evidence, root)
        for text in record['texts']:
            check_file(text, root)
        checked += 1
    # The APK ships JNA's libjnidispatch.so with libffi linked in.
    required = {'JNA native dispatch and libffi'}
    native = registry.get('native', [])
    if not required <= {n['name'] for n in native}:
        raise ValueError('Missing native component notices')
    for component in native:
        if not component.get('version') or not component.get('evidence'):
            raise ValueError('Incomplete native component evidence')
        for evidence in component['evidence']:
            check_file(evidence, root)
    return checked


def main():
    packages = combine(load_inventory(ROOT / 'artifacts/dependency-inventory.json') +
                       npm_packages(json.loads((ROOT / 'web/package-lock.json').read_text(encoding="utf-8"))))
    count = verify(packages, json.loads((ROOT / 'licenses/dependencies.json').read_text(encoding="utf-8")))
    for name in ['README.md', 'settings.gradle.kts', 'build.gradle.kts']:
        if not (ROOT / 'licenses/jediterm-rebuild' / name).is_file():
            raise ValueError('Missing JediTerm rebuild material: ' + name)
    print(f'License coverage passed: {count} selected runtime package versions and native notices')


if __name__ == '__main__':
    try:
        main()
    except (OSError, KeyError, ValueError) as error:
        print('License verification FAILED: ' + str(error), file=sys.stderr)
        sys.exit(1)
