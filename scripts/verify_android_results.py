"""Validate named instrumentation results, including ignored/assumption-failed cases."""
import re

EXPECTED_ANDROID = {
    'kim.jujin.rovessh.AndroidSshTest.sshKeysControlAndReconnectionKeepRemoteProcess',
    'kim.jujin.rovessh.AndroidPlatformTest.keystoreRoundTripTamperingAndBackupExclusion',
    'kim.jujin.rovessh.AndroidPlatformTest.bundledRendererImeBridgeResizeAndNavigationIsolation',
    'kim.jujin.rovessh.AndroidAppFlowTest.composeTerminalKeyboardToolbarRotationAndForegroundRestore',
}


def verify(output):
    fields, cases = {}, {}
    for line in output.splitlines():
        if line.startswith('INSTRUMENTATION_STATUS: '):
            key, separator, value = line.removeprefix('INSTRUMENTATION_STATUS: ').partition('=')
            if separator:
                fields[key] = value
        elif line.startswith('INSTRUMENTATION_STATUS_CODE: '):
            code = int(line.split(': ', 1)[1])
            if code != 1:
                name = fields.get('class', '') + '.' + fields.get('test', '')
                if name in cases or code != 0:
                    raise ValueError('Duplicate, failed or skipped Android test')
                cases[name] = 'passed'
            fields = {}
    if set(cases) != EXPECTED_ANDROID or not re.search(r'OK \(4 tests\)', output) or any(
            token in output for token in ('FAILURES', 'INSTRUMENTATION_FAILED', 'shortMsg=')):
        raise ValueError('Incomplete or failed Android instrumentation')
    return {'tests': 4, 'passed': 4, 'failed': 0, 'skipped': 0, 'cases': cases}
