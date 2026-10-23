#!/usr/bin/env python3
"""Explicit maintenance command: collect selected runtime license evidence for review.

Normal builds use verify_licenses.py offline. This command never approves unknown licenses.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import sys
import urllib.error
import urllib.request
from urllib.parse import urljoin
import xml.etree.ElementTree as ET
import zipfile
from audit_dependencies import ROOT, combine, load_inventory, npm_packages

CACHE = Path(os.environ.get('GRADLE_USER_HOME', Path.home() / '.gradle')) / 'caches/modules-2/files-2.1'
DEST = ROOT / 'licenses/evidence'


def save(data, source, kind):
    sha = hashlib.sha256(data).hexdigest()
    path = DEST / (sha + '.txt')
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {'path': path.relative_to(ROOT).as_posix(), 'sha256': sha, 'source': source, 'kind': kind}


def repository(group):
    if group.startswith(('androidx.', 'com.android', 'com.google.testing.platform')):
        return 'https://dl.google.com/dl/android/maven2/'
    if group == 'org.jetbrains.jediterm':
        return 'https://packages.jetbrains.team/maven/p/ij/intellij-dependencies/'
    return 'https://repo.maven.apache.org/maven2/'


def artifact_source(group, name, version, filename):
    base = repository(group) + '/'.join([group.replace('.', '/'), name, version]) + '/'
    for path in (CACHE / group / name / version).glob('*/' + name + '-' + version + '.module'):
        for variant in json.loads(path.read_text(encoding="utf-8")).get('variants', []):
            for f in variant.get('files', []):
                if f['name'] == filename:
                    url = urljoin(base, f['url'])
                    if not url.startswith(repository(group)):
                        raise ValueError('Artifact outside declared repository')
                    return url
    return base + filename


def pom(group, name, version, seen=None):
    seen = set() if seen is None else seen
    key = (group, name, version)
    if key in seen or len(seen) > 12:
        raise ValueError('Invalid POM parent chain')
    seen.add(key)
    url = repository(group) + '/'.join([group.replace('.', '/'), name, version, f'{name}-{version}.pom'])
    data = urllib.request.urlopen(url, timeout=30).read()
    root = ET.fromstring(data)
    evidence = [save(data, url, 'pom')]
    licenses = [{'name': x.findtext('{*}name'), 'url': x.findtext('{*}url')} for x in root.findall('./{*}licenses/{*}license')]
    if not licenses:
        parent = root.find('{*}parent')
        if parent is not None:
            licenses, extra = pom(*(parent.findtext('{*}' + k) for k in ('groupId', 'artifactId', 'version')), seen)
            evidence += extra
    return licenses, evidence


def embedded(data, source):
    evidence = []
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        for name in archive.namelist():
            if name.endswith('/'):
                continue
            if re.search(r'(^|/)(license[^/]*|notice[^/]*|copying[^/]*)$', name, re.I):
                evidence.append(save(archive.read(name), source + '!/' + name, 'embedded-notice'))
            elif name == 'classes.jar' or (name.startswith('libs/') and name.endswith('.jar')):
                evidence += embedded(archive.read(name), source + '!/' + name)
    return evidence


def license_texts(licenses):
    names = ' '.join(x['name'] or '' for x in licenses).lower()
    # Choose the Apache option for dual licensed JNA.
    if 'apache' in names:
        return ['licenses/Apache-2.0.txt']
    if 'bouncy castle' in names:
        return ['licenses/Bouncy-Castle.txt']
    if 'lgpl 3' in names or 'lgpl-3' in names:
        return ['licenses/JediTerm-LGPLv3.txt', 'licenses/GPL-3.0-or-later.txt']
    if names in ('mit license', 'mit'):
        return ['licenses/SLF4J.txt']
    if 'cc0' in names:
        return ['licenses/CC0-1.0.txt']
    raise ValueError('Unreviewed license: ' + repr(licenses))


def main():
    packages = combine(load_inventory(ROOT / 'artifacts/dependency-inventory.json') +
                       npm_packages(json.loads((ROOT / 'web/package-lock.json').read_text(encoding="utf-8"))))
    records = []
    for package in packages:
        if not any(u['scope'] == 'runtime' for u in package['usages']):
            continue
        p = {k: package[k] for k in ('ecosystem', 'name', 'version', 'artifacts')}
        if p['ecosystem'] == 'npm':
            path = ROOT / 'web/node_modules' / p['name']
            text = path / 'LICENSE'
            metadata = json.loads((path / 'package.json').read_text(encoding="utf-8"))
            if metadata['version'] != p['version'] or metadata['name'] != p['name'] or metadata['license'] != 'MIT':
                raise ValueError('npm license/version mismatch')
            p.update(licenses=[{'name': 'MIT'}], evidence=[save(text.read_bytes(), package['artifacts'][0]['source'] + '!/package/LICENSE', 'license')], texts=[])
        else:
            group, name = p['name'].split(':')
            licenses, evidence = pom(group, name, p['version'])
            texts = license_texts(licenses)
            for artifact in p['artifacts']:
                paths = list((CACHE / group / name / p['version']).glob('*/' + artifact['name']))
                path = next((f for f in paths if hashlib.sha256(f.read_bytes()).hexdigest() == artifact['sha256']), None)
                if path is None:
                    raise ValueError('Selected artifact absent from verified cache: ' + str(artifact))
                source = artifact_source(group, name, p['version'], path.name)
                if path.suffix in {'.jar', '.aar'}:
                    evidence += embedded(path.read_bytes(), source)
            p.update(licenses=licenses, evidence=evidence, texts=[{'path': t, 'sha256': hashlib.sha256((ROOT / t).read_bytes()).hexdigest()} for t in texts])
        records.append(p)
    native = json.loads((ROOT / 'licenses/native-components.json').read_text(encoding="utf-8"))
    output = {'schema': 1, 'packages': records, 'native': native}
    (ROOT / 'licenses/dependencies.json').write_text(json.dumps(output, indent=2) + '\n', encoding="utf-8")
    print(f'Collected {len(records)} runtime package license records; review evidence before release.')


if __name__ == '__main__':
    main()
