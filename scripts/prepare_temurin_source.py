#!/usr/bin/env python3
"""Fetch the exact build JDK's vendor source archive with its published digest."""
import json
import os
from pathlib import Path
import re
import shutil
import urllib.parse
import urllib.request
from evidence import ROOT, sha256, write_report


def release_values(path):
    return dict(re.findall(r'^(\w+)="(.*)"$', path.read_text(encoding='utf-8'), re.M))


def main():
    release = release_values(Path(os.environ['JAVA_HOME']) / 'release')
    if release.get('IMPLEMENTOR') != 'Eclipse Adoptium':
        raise ValueError('Windows packages require Eclipse Temurin')
    version = release['JAVA_RUNTIME_VERSION']
    url = ('https://api.adoptium.net/v3/assets/version/' + urllib.parse.quote(version, safe='') +
           '?architecture=x64&os=windows&image_type=jdk&vendor=eclipse&release_type=ga')
    def request(url, timeout=60):
        return urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'dgl-client-verification'}), timeout=timeout)
    with request(url) as response:
        releases = json.load(response)
    matches = [item for item in releases if item['version_data']['semver'].split('+')[0] == version.split('+')[0]
               and item['release_name'] == 'jdk-' + version]
    if len(matches) != 1:
        raise ValueError('Cannot identify exact Temurin source release')
    item = matches[0]
    source = item['source']
    if not source['link'].startswith('https://github.com/adoptium/temurin17-binaries/releases/download/'):
        raise ValueError('Unexpected vendor source provenance')
    # Adoptium's top-level source object omits checksum; the release ships a sidecar.
    checksum_url = source['link'] + '.sha256.txt'
    with request(checksum_url) as response:
        checksum = response.read().decode('ascii').split()[0]
    if not re.fullmatch('[a-f0-9]{64}', checksum):
        raise ValueError('Invalid vendor source checksum')
    directory = ROOT / 'app/build/temurin-source'
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / 'temurin-sources.tar.gz'
    with request(source['link'], timeout=120) as response, target.open('wb') as output:
        shutil.copyfileobj(response, output)
    if sha256(target) != checksum:
        target.unlink()
        raise ValueError('Vendor source hash mismatch')
    write_report(directory / 'source.json', {'vendor': release['IMPLEMENTOR'], 'runtimeVersion': version,
                 'releaseName': item['release_name'], 'releaseUrl': item['release_link'],
                 'sourceUrl': source['link'], 'checksumUrl': checksum_url, 'sourceSha256': checksum, 'file': target.name})
    print('Verified Temurin source: ' + item['release_name'])


if __name__ == '__main__':
    main()
