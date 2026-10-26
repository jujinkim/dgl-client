"""Validate named instrumentation results, including ignored/assumption-failed cases."""
import re

EXPECTED_ANDROID = {
    'kim.jujin.rovessh.AndroidConnectionLifetimeTest.imeBackgroundIntentAndTaskRemovalPreserveOnlyRemoteWork',
    'kim.jujin.rovessh.AgentNotificationTest.channelPermissionAndClickIntentRetainOnlyEventId',
    'kim.jujin.rovessh.AgentNotificationTest.nativeInboxDeduplicatesVisibleReadAndDoesNotResolveObservation',
    'kim.jujin.rovessh.AndroidHierarchyTest.compactRowsExpansionPanelDetailsAndSelectedHostActions',
    'kim.jujin.rovessh.AndroidMobileInputTest.mobileLayoutImeComposerAndSharedPreference',
    'kim.jujin.rovessh.AndroidPlatformTest.windowPanesKeepIndependentScreensAndOnlyTheContainerRequestsSize',
    'kim.jujin.rovessh.FileSyncTest.persistedDocumentTransportMergesProtectsSecretsAndRetriesFailedWrites',
    'kim.jujin.rovessh.AndroidVolumeKeyTest.cliLauncherCommandsLayoutDismissalAndTargetChanges',
    'kim.jujin.rovessh.AndroidSshTest.sshKeysControlAndReconnectionKeepRemoteProcess',
    'kim.jujin.rovessh.AndroidPlatformTest.keystoreRoundTripTamperingAndBackupExclusion',
    'kim.jujin.rovessh.AndroidPlatformTest.bundledRendererImeBridgeResizeAndNavigationIsolation',
    'kim.jujin.rovessh.AndroidAppFlowTest.composeTerminalKeyboardToolbarRotationAndForegroundRestore',
    'kim.jujin.rovessh.AndroidVolumeKeyTest.independentKeysConsumeOnlyTheInitialPress',
    'kim.jujin.rovessh.AndroidVolumeKeyTest.defaultHandlingAndLifecycleNeverReplayHeldKeys',
    'kim.jujin.rovessh.AndroidVolumeKeyTest.visibleShellDialogsSelectionRecoveryAndRecreation',
    'kim.jujin.rovessh.AndroidVolumeKeyTest.editorSaveCancelValidationAndStepOrdering',
}


def expected_for_selection(selection):
    """Resolve am instrument's comma-separated classes or Class#method names."""
    expected = set()
    for item in selection.split(','):
        matches = {name for name in EXPECTED_ANDROID
                   if name.rsplit('.', 1)[0] == item or name.rsplit('.', 1)[0] + '#' + name.rsplit('.', 1)[1] == item}
        if not matches:
            raise ValueError('Unknown Android test selection: ' + item)
        expected.update(matches)
    return expected


def verify(output, expected=EXPECTED_ANDROID):
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
    count = len(expected)
    if set(cases) != set(expected) or not re.search(rf'OK \({count} tests?\)', output) or any(
            token in output for token in ('FAILURES', 'INSTRUMENTATION_FAILED', 'shortMsg=')):
        raise ValueError('Incomplete or failed Android instrumentation')
    return {'tests': count, 'passed': count, 'failed': 0, 'skipped': 0, 'cases': cases}
