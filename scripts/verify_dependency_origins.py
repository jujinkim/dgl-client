#!/usr/bin/env python3
"""Re-fetch newly approved verification entries from the declared repository origins.

Use --baseline with the pre-update XML. CI uses Gradle strict verification, not this
maintenance command. A hash generated from a local cache alone is not provenance.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from refresh_licenses import ROOT, repository, artifact_source


def entries(path):
    result = {}
    for c in ET.parse(path).findall('.//{*}component'):
        for a in c.findall('{*}artifact'):
            hashes = {x.attrib['value'] for x in a.findall('{*}sha256')}
            if not hashes:
                raise ValueError('Artifact lacks SHA-256')
            result[(c.attrib['group'], c.attrib['name'], c.attrib['version'], a.attrib['name'])] = hashes
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--metadata', type=Path, default=ROOT / 'gradle/verification-metadata.xml')
    args = parser.parse_args()
    before, after = entries(args.baseline), entries(args.metadata)
    selected = [(key, value) for key, value in after.items() if before.get(key) != value]
    def verify(item):
        (group, name, version, artifact), hashes = item
        base = repository(group)
        url = base + '/'.join([group.replace('.', '/'), name, version, artifact])
        if artifact.endswith(('.jar', '.aar', '.klib')):
            url = artifact_source(group, name, version, artifact)
        candidates = [url]
        if name.endswith('.gradle.plugin') and not group.startswith('com.android'):
            candidates.append('https://plugins.gradle.org/m2/' + '/'.join([group.replace('.', '/'), name, version, artifact]))
        failures = []
        for url in candidates:
            try:
                with urllib.request.urlopen(url, timeout=60) as response:
                    digest = hashlib.sha256()
                    while block := response.read(1024 * 1024):
                        digest.update(block)
                if digest.hexdigest() in hashes:
                    break
                failures.append('hash differs: ' + url)
            except urllib.error.HTTPError as error:
                failures.append(f'HTTP {error.code}: {url}')
        else:
            raise ValueError('Origin verification failed: ' + '; '.join(failures))
        return {'coordinate': f'{group}:{name}:{version}', 'artifact': artifact, 'sha256': digest.hexdigest(), 'source': url}
    with ThreadPoolExecutor(max_workers=6) as pool:
        verified = list(pool.map(verify, selected))
    path = ROOT / 'security/artifact-origins.json'
    prior = json.loads(path.read_text(encoding="utf-8"))['artifacts'] if path.exists() else []
    merged = {(r['coordinate'], r['artifact']): r for r in prior + verified}
    path.write_text(json.dumps({'artifacts': sorted(merged.values(), key=lambda r: (r['coordinate'], r['artifact']))}, indent=2) + '\n', encoding="utf-8")
    print(f'Independently re-fetched and verified {len(verified)} new/changed artifacts.')


if __name__ == '__main__':
    main()
