#!/usr/bin/env python3
"""R1 measurement primitives. Missing/partial data is never an acceptance pass.

Trace producers must timestamp actual input-ready/present events; SSH callbacks,
write acknowledgements, am start TotalTime and headless parsing are different metrics.
See docs/PERFORMANCE.md for the input format and collection procedure.
"""
import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import time

from evidence import ROOT, context, sha256, write_report

CONTRACT = ROOT / 'fixtures/migration/performance-v1.json'
RULES = json.loads(CONTRACT.read_text())
MATCH = ('device', 'os', 'cpu', 'gpu', 'ramBytes', 'displayHz', 'displayScale', 'powerMode',
         'scenario', 'buildMode', 'memoryMetric', 'webviewVersion', 'renderer')


def number(value, name, minimum=0):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < minimum:
        raise ValueError('Invalid numeric field: ' + name)
    return value


def p95(values):
    if not values:
        raise ValueError('Missing samples')
    values = sorted(number(v, 'sample') for v in values)
    return values[math.ceil(len(values) * .95) - 1]


def durations(rows, minimum, start, end):
    if not isinstance(rows, list) or len(rows) < minimum:
        raise ValueError('Insufficient samples: ' + start)
    seen = set()
    values = []
    previous_start = -1
    for row in rows:
        ident = row['id']
        if ident in seen:
            raise ValueError('Duplicate sample')
        seen.add(ident)
        a, b = number(row[start], start), number(row[end], end)
        if b <= a or a <= previous_start:
            raise ValueError('Reversed/repeated clock or zero duration')
        previous_start = a
        values.append((b - a) / 1e6)
    return p95(values)


def idle_metrics(idle):
    if idle['connections'] != 1 or idle['terminals'] != 1 or idle['outputBytes'] != 0:
        raise ValueError('Idle scenario mismatch')
    if number(idle['stabilizeSeconds'], 'stabilization') < RULES['idle']['stabilizeSeconds']:
        raise ValueError('Insufficient stabilization')
    if idle['completeProcessInventory'] is not True:
        raise ValueError('Incomplete process inventory')
    gpu = idle['gpuMemory']
    if gpu.get('bytes') is None:
        if not gpu.get('unavailableReason'):
            raise ValueError('GPU memory must be separately recorded')
    else:
        number(gpu['bytes'], 'GPU memory')
    rows = idle['samples']
    if len(rows) < 61:
        raise ValueError('Insufficient idle samples')
    identities = None
    cpu = []
    rss = []
    times = []
    for row in rows:
        t = number(row['elapsedSeconds'], 'elapsed')
        if times and not 0 < t - times[-1] <= 2:
            raise ValueError('Idle time gap/reversal')
        times.append(t)
        processes = row['processes']
        ids = {(p['pid'], p['identity'], p['role']) for p in processes}
        if (not ids or len({p['pid'] for p in processes}) != len(processes)
                or sum(p['role'] == 'app' for p in processes) != 1
                or any(p['role'] not in ('app', 'child', 'webview') for p in processes)):
            raise ValueError('Invalid process inventory')
        if identities is not None and ids != identities:
            raise ValueError('Process replaced during idle')
        identities = ids
        current = {(p['pid'], p['identity']): number(p['cpuSeconds'], 'CPU') for p in processes}
        if cpu and any(current[k] < cpu[-1][k] for k in current):
            raise ValueError('CPU counter reset')
        cpu.append(current)
        rss.append(sum(number(p['rssBytes'], 'RSS', 1) for p in processes))
    elapsed = times[-1] - times[0]
    if elapsed < RULES['idle']['measureSeconds']:
        raise ValueError('Idle window shorter than 60s')
    # Trapezoidal time weighting, not an unweighted mean of jittered polls.
    average = sum((rss[i] + rss[i - 1]) / 2 * (times[i] - times[i - 1]) for i in range(1, len(rows))) / elapsed
    usage = 100 * (sum(cpu[-1].values()) - sum(cpu[0].values())) / elapsed
    return {'idleCpuOneCorePercent': usage, 'idleRssBytes': average}


def summarize(trace):
    if trace['schema'] != 1 or trace['contractSha256'] != sha256(CONTRACT):
        raise ValueError('Wrong measurement contract')
    if trace['scope'] not in RULES['scope']:
        raise ValueError('Outside current validation scope')
    env = trace['environment']
    if any(k not in env for k in MATCH) or any(env[k] in (None, '') for k in MATCH if k != 'webviewVersion'):
        raise ValueError('Missing environment metadata')
    if env['displayHz'] != RULES['displayHz']:
        raise ValueError('Display is not 60Hz')
    if not re.fullmatch('[0-9a-f]{40}', trace['sourceCommit']):
        raise ValueError('Missing source commit')
    if not re.fullmatch('[0-9a-f]{64}', trace['binarySha256']):
        raise ValueError('Missing measured binary hash')
    if trace['clock'] != 'monotonic-nanoseconds' or trace['inputEnd'] != 'presented' or trace['synthetic'] is not False:
        raise ValueError('Synthetic or non-presentation trace')
    metrics = {}
    metrics['localInputP95Ms'] = durations(trace['localInput'], RULES['samples']['localInput'], 'inputNs', 'presentedNs')
    metrics['frameP95Ms'] = durations(trace['frames'], RULES['samples']['frames'], 'previousPresentNs', 'presentedNs')
    if any(current['previousPresentNs'] != previous['presentedNs'] for previous, current in zip(trace['frames'], trace['frames'][1:])):
        raise ValueError('Non-contiguous frame trace')
    metrics.update(idle_metrics(trace['idle']))
    if trace['scope'].startswith('macos'):
        if env['buildMode'] != 'release' or env['memoryMetric'] != RULES['memoryMetric']['macos']:
            raise ValueError('Desktop requires release/RSS')
        for condition in ('cold', 'warm'):
            metrics[condition + 'StartupP95Ms'] = durations(trace['startup'][condition], RULES['samples'][condition + 'Startup'], 'launchNs', 'inputReadyNs')
        for key in ('downloadBytes', 'installedBytes'):
            metrics[key] = number(trace['package'][key], key, 1)
    else:
        if env['memoryMetric'] != RULES['memoryMetric']['android'] or not env['webviewVersion']:
            raise ValueError('Android RSS/WebView metadata missing')
        if not all(any(p['role'] == 'webview' for p in r['processes']) for r in trace['idle']['samples']):
            raise ValueError('WebView process omitted')
    return metrics


def evaluate(trace, baseline=None):
    result = {'schema': 1, 'complete': False, 'status': 'incomplete', 'errors': [], 'metrics': {}}
    try:
        result['metrics'] = metrics = summarize(trace)
        if trace['scope'].startswith('macos'):
            limits = RULES['desktop']
            for name, value in metrics.items():
                limit = limits['startupP95Ms'] if name.endswith('StartupP95Ms') else limits[name]
                if value > limit:
                    result['errors'].append('over_budget:' + name)
        else:
            if baseline is None:
                raise ValueError('Android requires measured legacy baseline')
            if baseline['scope'] != trace['scope'] or any(trace['environment'][k] != baseline['environment'][k] for k in MATCH):
                raise ValueError('Android baseline environment/scenario differs')
            reference = summarize(baseline)
            for name, rule in (('localInputP95Ms', 'maximumLatencyRatio'), ('frameP95Ms', 'maximumLatencyRatio'), ('idleRssBytes', 'maximumMemoryRatio')):
                if reference[name] <= 0:
                    raise ValueError('Invalid zero baseline')
                if metrics[name] > reference[name] * RULES['android'][rule]:
                    result['errors'].append('regression:' + name)
        result.update(complete=True, status='failed' if result['errors'] else 'passed')
    except (KeyError, TypeError, ValueError) as error:
        result['errors'].append(str(error))
    return result


def package_sizes(download, installed):
    download, installed = Path(download), Path(installed)
    if not download.is_file() or download.is_symlink() or not installed.is_dir() or installed.is_symlink():
        raise ValueError('Expected a package file and installed directory')
    # Installed tree logical size; each hardlinked inode counted once. Internal
    # symlinks count their link bytes, never follow links out of the installation.
    total, seen = 0, set()
    for root, directories, files in os.walk(installed, followlinks=False):
        for name in directories + files:
            stat = (Path(root) / name).lstat()
            ident = (stat.st_dev, stat.st_ino)
            if ident not in seen:
                seen.add(ident)
                if not (Path(root) / name).is_dir() or (Path(root) / name).is_symlink():
                    total += stat.st_size
    return {'downloadBytes': download.stat().st_size, 'installedBytes': total, 'sha256': sha256(download), 'installedMetric': 'logical-bytes-hardlinks-once-symlinks-not-followed'}


def cpu_seconds(value):
    days, _, clock = value.rpartition('-')
    parts = clock.split(':')
    if len(parts) not in (2, 3):
        raise ValueError('Unexpected ps CPU format')
    return (int(days) * 86400 if days else 0) + sum(float(v) * 60 ** i for i, v in enumerate(reversed(parts)))


def mac_processes(root_pid):
    # No command lines, process environment, usernames or terminal content captured.
    output = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,time=,lstart='], text=True)
    rows = {}
    for line in output.splitlines():
        parts = line.split(None, 4)
        if len(parts) == 5:
            rows[int(parts[0])] = dict(pid=int(parts[0]), parent=int(parts[1]), rssBytes=int(parts[2]) * 1024,
                                     cpuSeconds=cpu_seconds(parts[3]), identity=parts[4])
    if root_pid not in rows:
        raise ValueError('Measured app exited')
    pids = {root_pid}
    while True:
        children = {pid for pid, row in rows.items() if row['parent'] in pids}
        if children <= pids:
            break
        pids |= children
    return [dict(rows[pid], role='app' if pid == root_pid else 'child') for pid in sorted(pids)]


def collect_idle(pid, output):
    if platform.system() != 'Darwin':
        raise ValueError('This collector currently supports macOS; Android uses resource probe evidence')
    report = {'schema': 1, 'context': context(), 'complete': False, 'status': 'incomplete',
              'metric': RULES['memoryMetric']['macos'], 'stabilizeSeconds': 30, 'samples': []}
    write_report(output, report)
    try:
        # A process sample does not prove one connected terminal or no output. It
        # remains partial evidence until combined with the app's scenario trace.
        for _ in range(30):
            mac_processes(pid)
            time.sleep(1)
        begin = time.monotonic()
        for index in range(62):
            time.sleep(max(0, begin + index - time.monotonic()))
            report['samples'].append(dict(elapsedSeconds=time.monotonic() - begin, processes=mac_processes(pid)))
            write_report(output, report)
        report.update(complete=True, status='collected')
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report['errorType'] = type(error).__name__
        raise
    finally:
        write_report(output, report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    evaluate_cmd = commands.add_parser('evaluate')
    evaluate_cmd.add_argument('trace', type=Path)
    evaluate_cmd.add_argument('--baseline', type=Path)
    package = commands.add_parser('package')
    package.add_argument('--download', required=True)
    package.add_argument('--installed', required=True)
    idle = commands.add_parser('idle')
    idle.add_argument('--pid', type=int, required=True)
    for command in (evaluate_cmd, package, idle):
        command.add_argument('--output', required=True)
    args = parser.parse_args()
    destination = Path(args.output).resolve()
    if args.command == 'evaluate' and destination in {p.resolve() for p in (args.trace, args.baseline) if p}:
        parser.error('Output must not overwrite trace or baseline input')
    if args.command == 'package' and (destination == Path(args.download).resolve() or
                                    Path(args.installed).resolve() in destination.parents):
        parser.error('Output must be outside the measured package/installation')
    if args.command == 'idle':
        collect_idle(args.pid, args.output)
        return 0
    # Invalidate an old success even when parsing or collection fails.
    write_report(args.output, {'schema': 1, 'complete': False, 'status': 'incomplete'})
    if args.command == 'package':
        report = dict(schema=1, context=context(), complete=True, status='collected', **package_sizes(args.download, args.installed))
    else:
        report = evaluate(json.loads(args.trace.read_text()), json.loads(args.baseline.read_text()) if args.baseline else None)
        report.update(context=context(), traceSha256=sha256(args.trace), contractSha256=sha256(CONTRACT))
        if args.baseline:
            report['baselineSha256'] = sha256(args.baseline)
    write_report(args.output, report)
    print(report['status'])
    return 0 if report['status'] in ('collected', 'passed') else 1


if __name__ == '__main__':
    raise SystemExit(main())
