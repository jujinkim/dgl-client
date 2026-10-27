"""Validate named instrumentation results, including ignored/assumption-failed cases."""
import re

EXPECTED_ANDROID = {
    'kim.jujin.dglc.AndroidMobileInputTest.imeTransitionsRecordFrameDurations',
    'kim.jujin.dglc.AndroidMobileInputTest.cliLauncherKeepsImeAndSurvivesResizeWithoutLeakingInput',
    'kim.jujin.dglc.AndroidMobileInputTest.compactTabsAndNotificationPopupPreserveInputAndFocus',
    'kim.jujin.dglc.AndroidMobileInputTest.exhaustedRetryIsCenteredAndRetainsTheTerminal',
    'kim.jujin.dglc.AndroidWorkspaceTest.adaptiveGroupsKeepRenderersImeAndActiveInput',
    'kim.jujin.dglc.AndroidWorkspaceTest.pinchFontAndScrollBoundsStayWithTheSession',
    'kim.jujin.dglc.AndroidConnectionLifetimeTest.imeBackgroundIntentAndTaskRemovalPreserveOnlyRemoteWork',
    'kim.jujin.dglc.AndroidOutputRecoveryTest.timedOutOutputResumesWithoutSshReconnectAndFailedRetryNeedsManualAction',
    'kim.jujin.dglc.AgentNotificationTest.channelPermissionAndClickIntentRetainOnlyEventId',
    'kim.jujin.dglc.AgentNotificationTest.nativeInboxDeduplicatesVisibleReadAndDoesNotResolveObservation',
    'kim.jujin.dglc.AndroidHierarchyTest.compactRowsExpansionPanelDetailsAndSelectedHostActions',
    'kim.jujin.dglc.AndroidMobileInputTest.mobileLayoutImeComposerAndSharedPreference',
    'kim.jujin.dglc.AndroidPlatformTest.windowPanesKeepIndependentScreensAndOnlyTheContainerRequestsSize',
    'kim.jujin.dglc.FileSyncTest.persistedDocumentTransportMergesProtectsSecretsAndRetriesFailedWrites',
    'kim.jujin.dglc.AndroidVolumeKeyTest.cliLauncherCommandsLayoutDismissalAndTargetChanges',
    'kim.jujin.dglc.AndroidSshTest.sshKeysControlAndReconnectionKeepRemoteProcess',
    'kim.jujin.dglc.AndroidPlatformTest.keystoreRoundTripTamperingAndBackupExclusion',
    'kim.jujin.dglc.AndroidPlatformTest.bundledRendererImeBridgeResizeAndNavigationIsolation',
    'kim.jujin.dglc.AndroidPlatformTest.bundledRendererRemoteColumnsAndTouchScrolling',
    'kim.jujin.dglc.AndroidAppFlowTest.composeTerminalKeyboardToolbarRotationAndForegroundRestore',
    'kim.jujin.dglc.AndroidVolumeKeyTest.independentKeysConsumeOnlyTheInitialPress',
    'kim.jujin.dglc.AndroidVolumeKeyTest.defaultHandlingAndLifecycleNeverReplayHeldKeys',
    'kim.jujin.dglc.AndroidVolumeKeyTest.visibleShellDialogsSelectionRecoveryAndRecreation',
    'kim.jujin.dglc.AndroidVolumeKeyTest.editorSaveCancelValidationAndStepOrdering',
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
