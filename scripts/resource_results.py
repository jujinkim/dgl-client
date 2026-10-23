"""Pure collectors/parsers and strict resource verdicts. Smoke never qualifies as soak."""
import re
import statistics
import math

MIB = 1024 * 1024
PROFILES = {'soak': dict(warmup=300, phase=600, observe=120, repeats=20, window=120),
            'smoke': dict(warmup=20, phase=30, observe=10, repeats=2, window=15)}
CRITERIA = {'memoryGrowthBytes': 32 * MIB, 'memoryGrowthFraction': .10,
            'fdGrowth': 4, 'threadGrowth': 4, 'scrollback': 10000,
            'sampleSeconds': 2, 'androidDetailSeconds': 10}


def parse_meminfo(output):
    """Android meminfo App Summary; missing is None, never zero."""
    def number(pattern):
        match = re.search(pattern, output)
        return int(match[1]) * 1024 if match else None
    return {'pssBytes': number(r'TOTAL PSS:\s+(\d+)'), 'rssBytes': number(r'TOTAL RSS:\s+(\d+)')}


def renderer_pids(output, package):
    """Require a bound service record referring to the host, not all WebViews on device."""
    result = set()
    for record in re.split(r'(?=\* ServiceRecord\{)', output):
        if 'SandboxedProcessService' not in record or package not in record:
            continue
        match = re.search(r'\bapp=ProcessRecord\{[^\n]*?\s(\d+):[^\n]*sandboxed_process', record, re.I)
        if match:
            result.add(int(match[1]))
    return sorted(result)


def evaluate(samples, final, profile, platform, completed):
    spec = PROFILES[profile]
    errors = []
    def require(condition, reason):
        if not condition and reason not in errors:
            errors.append(reason)
    require(completed and final.get('complete') is True and not final.get('failure'), 'incomplete_or_interrupted')
    require(final.get('recoveries') == spec['repeats'], 'recovery_count')
    require(final.get('recreations') == spec['repeats'], 'recreation_count')
    require(final.get('blockedInputs') == spec['repeats'], 'blocked_input_count')
    require(final.get('slowConsumerChecked') is True, 'slow_consumer_not_checked')
    require(58982 <= final.get('outputBytesPerSecond', 0) <= 72090, 'output_rate')
    require(final.get('controlClients') == 0, 'control_client_not_reclaimed')
    require(bool(samples), 'no_samples')
    pids = {s.get('appPid') for s in samples}
    require(len(pids) == 1 and None not in pids, 'app_pid_changed_or_missing')
    remote = {s.get('remotePid') for s in samples if s.get('remotePid', 0)}
    require(len(remote) == 1, 'remote_pid_changed_or_missing')
    required = ('rssBytes', 'heapBytes', 'fd', 'threads', 'gcCount', 'gcMillis', 'liveTerminals', 'scrollback', 'queuedBytes')
    for key in required:
        require(all(type(s.get(key)) in (int, float) and math.isfinite(s[key]) and s[key] >= 0 for s in samples), 'missing_' + key)
    for a, b in zip(samples, samples[1:]):
        require(0 < b['elapsed'] - a['elapsed'] <= 6, 'sample_gap')
    require(all(s.get('scrollback', 10001) <= 10000 for s in samples), 'scrollback_limit')
    require(all(s.get('queuedBytes', 999999) <= (262144 if platform == 'desktop' else 22000) for s in samples), 'output_buffer_limit')
    trends = {}
    for phase in ('output', 'recovery', 'recreate'):
        rows = [s for s in samples if s['phase'] == phase]
        require(bool(rows) and max((s['phaseElapsed'] for s in rows), default=0) >= spec['phase'] - 6, phase + '_duration')
        require(max((s['phaseElapsed'] for s in rows), default=0) <= spec['phase'] + 6, phase + '_overrun')
        rendering = [s for s in rows if s.get('modelPhase') == 'Ready' and s.get('renderSequence', 0) > 0]
        require(bool(rendering), phase + '_renderer_no_progress')
        if rendering:
            require(rendering[-1]['renderSequence'] > rendering[0]['renderSequence'], phase + '_renderer_stalled')
        trend = {}
        for metric in ('rssBytes', 'pssBytes', 'rendererRssBytes', 'rendererPssBytes', 'heapBytes'):
            if platform == 'desktop' and metric not in ('rssBytes', 'heapBytes'):
                continue
            values = [s for s in rows if s.get(metric) is not None and
                      (metric in ('rssBytes', 'heapBytes') or s.get('memoryPhase', phase) == phase)]
            first = [s[metric] for s in values if s['phaseElapsed'] <= spec['window']]
            last = [s[metric] for s in values if s['phaseElapsed'] >= spec['phase'] - spec['window']]
            require(bool(first) and bool(last), phase + '_missing_windows_' + metric)
            if profile == 'soak':
                minimum = 9 if metric in ('pssBytes', 'rendererRssBytes', 'rendererPssBytes') else 45
                require(len(first) >= minimum and len(last) >= minimum, phase + '_insufficient_samples_' + metric)
            if first and last:
                begin, end = statistics.median(first), statistics.median(last)
                limit = max(32 * MIB, begin * .1)
                trend[metric] = {'firstMedian': begin, 'lastMedian': end, 'growth': end - begin, 'limit': limit,
                                 'sampleMax': max(s[metric] for s in values)}
                # Smoke cannot supply the specified two-minute comparison windows.
                # It exercises collection/lifecycle only; never a memory acceptance.
                if profile == 'soak' and metric != 'heapBytes':
                    require(end - begin <= limit, phase + '_growth_' + metric)
        trends[phase] = trend
    for phase, duration in [('warmup', spec['warmup']), ('observe', spec['observe'])]:
        require(max((s['phaseElapsed'] for s in samples + [final] if s.get('phase') == phase), default=0) >= duration - 3, phase + '_duration')
    baseline = [s for s in samples if s['phase'] == 'baseline']
    after = [s for s in samples if s['phase'] == 'observe' and s['phaseElapsed'] >= spec['observe'] / 2]
    idle = {}
    require(bool(baseline) and bool(after), 'missing_idle_comparison')
    if baseline and after:
        for metric, limit in [('fd', 4), ('threads', 4), ('liveTerminals', 0), ('sshSockets', 0)]:
            a = [s[metric] for s in baseline if s.get(metric) is not None]
            b = [s[metric] for s in after if s.get(metric) is not None]
            require(bool(a) and bool(b), 'missing_idle_' + metric)
            if a and b:
                growth = statistics.median(b) - statistics.median(a)
                idle[metric] = dict(before=statistics.median(a), after=statistics.median(b), growth=growth, limit=limit)
                require(growth <= limit, 'idle_growth_' + metric)
                if metric in ('liveTerminals', 'sshSockets'):
                    # One admin SSH socket lives through both idle observations.
                    require(statistics.median(b) <= (1 if metric == 'sshSockets' else 0), 'retained_' + metric)
    if platform == 'android':
        details = [s for s in samples if s.get('memoryDetail') and s.get('modelPhase') == 'Ready' and
                   s.get('memoryPhase', s['phase']) == s['phase']]
        require(bool(details), 'missing_renderer_detail')
        require(all(s.get('anrCount') == 0 and s.get('crashCount') == 0 for s in details), 'android_anr_crash_or_missing')
        require(all(s.get('rendererPids') and s.get('pssBytes') is not None and s.get('rendererPssBytes') is not None for s in details), 'missing_android_memory')
        for phase in ('output', 'recovery'):
            identities = {tuple(s.get('rendererPids', [])) for s in details if s['phase'] == phase}
            require(len(identities) == 1, phase + '_renderer_pid_changed')
    maxima = {key: max((s[key] for s in samples if isinstance(s.get(key), (int, float))), default=None)
              for key in required + ('pssBytes', 'rendererRssBytes', 'rendererPssBytes', 'osPeakRssBytes')}
    return dict(status='failed' if errors else ('passed' if profile == 'soak' else 'smoke_passed'),
                memoryAcceptanceEvaluated=profile == 'soak',
                qualifiesAsSoak=profile == 'soak' and not errors, errors=errors, criteria=CRITERIA,
                phases=trends, idle=idle, sampleMaxima=maxima, counts={k: final.get(k) for k in ('recoveries', 'recreations', 'blockedInputs', 'checks')})
