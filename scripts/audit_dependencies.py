#!/usr/bin/env python3
"""Fail-closed OSV audit of selected Gradle graphs and npm lockfile packages."""
import argparse
from datetime import date
import hashlib
import json
from pathlib import Path
import re
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_GRAPHS = {(':androidApp', 'debugRuntimeClasspath'), (':androidApp', 'releaseRuntimeClasspath'),
                   (':app', 'desktopRuntimeClasspath'), (':app', 'desktopTestRuntimeClasspath'),
                   (':androidApp', 'debugAndroidTestRuntimeClasspath'), (':', 'classpath'),
                   (':androidApp', 'androidLintTool'), (':app', 'androidLintTool'), (':app', 'inventoryAapt2')}
PLATFORMS = {'windows-x64', 'linux-x64', 'macos-arm64'}


def load_inventory(path, root=ROOT):
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get('schema') != 1 or data.get('complete') is not True or not data.get('packages') or not data.get('inputs'):
        raise ValueError('Incomplete dependency inventory')
    for name, sha in data['inputs'].items():
        if hashlib.sha256((root / name).read_bytes()).hexdigest() != sha:
            raise ValueError('Stale inventory input: ' + name)
    graphs = data.get('graphs', [])
    if not REQUIRED_GRAPHS <= {(g['module'], g['configuration']) for g in graphs}:
        raise ValueError('Missing required dependency graphs')
    if not PLATFORMS <= {g['platform'] for g in graphs if g['scope'] == 'runtime'}:
        raise ValueError('Missing desktop platform graph')
    for graph in graphs:
        selected = {u.get('selected') for p in data['packages'] for u in p['usages']
                    if all(u[k] == graph[k] for k in ('module', 'configuration', 'platform', 'scope'))}
        required = {e['to'] for e in graph['edges'] if re.fullmatch(r'[^: ]+:[^: ]+:[^: ]+', e['to'])}
        if not required <= selected:
            raise ValueError('Selected graph components omitted from package inventory')
    for p in data['packages']:
        if not p.get('usages') or not p.get('name') or not p.get('version'):
            raise ValueError('Incomplete package record')
        for a in p['artifacts']:
            if not re.fullmatch(r'[a-f0-9]{64}', a.get('sha256', '')):
                raise ValueError('Missing artifact SHA-256')
        for u in p['usages']:
            if u['scope'] not in {'runtime', 'build', 'test'}:
                raise ValueError('Unknown dependency scope')
            if not any(all(u[k] == g[k] for k in ('module', 'configuration', 'platform', 'scope')) and
                       any(e['to'] == u['selected'] for e in g['edges']) for g in graphs):
                raise ValueError('Package absent from selected graph: ' + p['name'])
            if p['name'] == 'com.android.tools.build:aapt2' and u['scope'] == 'runtime':
                raise ValueError('Build tool in runtime inventory')
            if 'skiko-awt-runtime-' in p['name'] and u['platform'] in PLATFORMS:
                if not p['name'].endswith(u['platform']):
                    raise ValueError('Foreign native artifact in desktop graph')
    # Native packages vary by OS; the shipped Java artifacts must be identical.
    payloads = {platform: set() for platform in PLATFORMS | {'host-desktop'}}
    for p in data['packages']:
        if p['name'].startswith('org.jetbrains.compose.desktop:desktop-jvm-') or 'skiko-awt-runtime-' in p['name']:
            continue
        for u in p['usages']:
            if u['scope'] == 'runtime' and u['platform'] in payloads:
                for artifact in u.get('artifacts', []):
                    if artifact not in p['artifacts']:
                        raise ValueError('Usage artifact missing from package hashes')
                    payloads[u['platform']].add((p['name'], p['version'], artifact['name'], artifact['sha256']))
    if any(payloads[p] != payloads['host-desktop'] for p in PLATFORMS):
        raise ValueError('Desktop Java payload differs across OS inventories')
    return data['packages']


def npm_packages(lock):
    if lock.get('lockfileVersion') != 3 or 'packages' not in lock:
        raise ValueError('Expected complete npm v3 lockfile')
    result = []
    for path, data in lock['packages'].items():
        if not path:
            continue
        if 'node_modules/' not in path or not data.get('integrity') or not data.get('version'):
            raise ValueError('Unresolved npm package: ' + path)
        name = data.get('name') or path.rsplit('node_modules/', 1)[1]
        result.append({'ecosystem': 'npm', 'name': name, 'version': data['version'],
                       'artifacts': [{'name': path, 'integrity': data['integrity'], 'source': data['resolved']}],
                       'usages': [{'module': 'web', 'configuration': path, 'platform': 'android',
                                   'scope': 'build' if data.get('dev') else 'runtime'}]})
    if not result:
        raise ValueError('Empty npm inventory')
    return result


def combine(packages):
    selected = {}
    for p in packages:
        key = (p['ecosystem'], p['name'], p['version'])
        item = selected.setdefault(key, dict(p, usages=[], artifacts=[]))
        for field in ('usages', 'artifacts'):
            for value in p[field]:
                if value not in item[field]:
                    item[field].append(value)
    return list(selected.values())


def request_json(url, payload=None):
    request = urllib.request.Request(url, data=None if payload is None else json.dumps(payload).encode(),
                                     headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=45) as response:
        return json.load(response)


def query_osv(packages, request=request_json):
    findings = {}
    for start in range(0, len(packages), 100):
        batch = packages[start:start + 100]
        response = request('https://api.osv.dev/v1/querybatch', {'queries': [
            {'package': {'ecosystem': p['ecosystem'], 'name': p['name']}, 'version': p['version']} for p in batch]})
        results = response.get('results')
        if not isinstance(results, list) or len(results) != len(batch):
            raise ValueError('OSV omitted batch results')
        for p, result in zip(batch, results):
            if not isinstance(result, dict) or set(result) - {'vulns'}:
                raise ValueError('Incomplete/error OSV response')
            if not isinstance(result.get('vulns', []), list):
                raise ValueError('Invalid OSV vulnerabilities')
            for vuln in result.get('vulns', []):
                if not isinstance(vuln, dict) or not vuln.get('id'):
                    raise ValueError('OSV advisory missing ID')
                key = (p['ecosystem'], p['name'], p['version'], vuln['id'])
                findings[key] = dict(p, id=vuln['id'])
    return list(findings.values())


def apply_exceptions(findings, exceptions, today=None):
    today = today or date.today()
    keys = set()
    for e in exceptions:
        key = tuple(e.get(k) for k in ('ecosystem', 'name', 'version', 'id'))
        if not all(key) or key in keys:
            raise ValueError('Incomplete or duplicate exception')
        keys.add(key)
        reviewed, expires, recheck = [date.fromisoformat(e[k]) for k in ('reviewed', 'expires', 'recheck')]
        if not reviewed <= today <= recheck <= expires or not 0 < (expires - reviewed).days <= 90:
            raise ValueError('Expired, future, overdue or over-90-day exception: ' + e['id'])
        if not e.get('reason') or not e.get('evidence') or not e.get('conditions') or not e.get('usages'):
            raise ValueError('Exception lacks evidence/conditions/scope')
        if any(u['scope'] not in {'build', 'test'} for u in e['usages']):
            raise ValueError('Runtime exception forbidden')
    for f in findings:
        f['status'] = 'blocked'
        for e in exceptions:
            if all(f[k] == e[k] for k in ('ecosystem', 'name', 'version', 'id')):
                usages = [{k: u[k] for k in ('module', 'configuration', 'platform', 'scope')} for u in f['usages']]
                actual = {json.dumps(u, sort_keys=True) for u in usages}
                allowed = {json.dumps(u, sort_keys=True) for u in e['usages']}
                if any(u['scope'] == 'runtime' for u in usages) or actual != allowed:
                    raise ValueError('Exception cannot cover runtime or additional usage: ' + f['id'])
                f['status'] = 'excepted'
                f['exception'] = e
    return findings


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--inventory', type=Path, default=ROOT / 'artifacts/dependency-inventory.json')
    parser.add_argument('--exceptions', type=Path, default=ROOT / 'security/dependency-exceptions.json')
    args = parser.parse_args()
    report = ROOT / 'artifacts/dependency-audit.json'
    report.parent.mkdir(exist_ok=True)
    # Invalidate a prior successful report before any network or inventory operation.
    report.write_text(json.dumps({'complete': False, 'status': 'failed'}) + '\n', encoding="utf-8")
    try:
        packages = combine(load_inventory(args.inventory) + npm_packages(json.loads((ROOT / 'web/package-lock.json').read_text(encoding="utf-8"))))
        findings = apply_exceptions(query_osv(packages), json.loads(args.exceptions.read_text(encoding="utf-8"))['exceptions'])
        blocked = sum(f['status'] == 'blocked' for f in findings)
        runtime = sum(any(u['scope'] == 'runtime' for u in f['usages']) for f in findings)
        report.write_text(json.dumps({'complete': True, 'date': date.today().isoformat(), 'packages': len(packages),
                                     'runtime': runtime, 'blocked': blocked, 'findings': findings}, indent=2) + '\n', encoding="utf-8")
        print(f'OSV: {len(packages)} selected versions; {runtime} runtime, {len(findings)-runtime} build/test, {blocked} blocked findings')
        return 1 if blocked else 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('Dependency audit FAILED: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
