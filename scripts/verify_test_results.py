#!/usr/bin/env python3
"""Require every named JVM test; only explicitly platform-scoped skips are allowed."""
import argparse
import xml.etree.ElementTree as ET
from evidence import ROOT, context, write_report

TESTS = {
    'core.TerminalFontSizeTest': ['pinchAccumulatesSmallChangesClampsAndIgnoresInvalidFactors',
                                'overridesStayIndependentAndResetUsesTheLatestDefault'],
    'core.BackgroundResumeTest': ['backgroundStopKeepsTargetAndResumesOnlyOnce',
                                 'explicitStopsFailuresAndMissingSessionsNeverReserveResume'],
    'core.TerminalRecoveryTest': ['healthyCheckKeepsScreenWhileRecoveryWaitsForEveryCurrentPane',
                                  'staleSubscriptionsAndCyclesCannotClearRecoveryOrReplayInput',
                                  'failuresStayDimmedAndResizeAloneDoesNotStartRecovery'],
    'WorkspaceTest': ['splitMoveMergeSelectionAndEmptyGroups', 'minimumSpaceGroupLimitAndResizePreserveLayout',
                      'validatedDropsAndCancelledDragsDoNotMutateState', 'geometryOnlyPersistenceAndMalformedFilesStartEmpty',
                      'sessionsReserveBeforeConnectAndReuseAcrossGroups', 'staleConnectionGenerationsRejectDelayedTargetsAndModalGates',
                      'notificationsReuseSessionTabsAndObserveEveryVisibleConnection'],
    'ConnectionLifetimeTest': ['homeLockAndRecreationReuseConnectionsUntilTaskRemoval',
                               'disablingMaintenanceAndRetryExhaustionReleaseResources',
                               'rejectedOrLostServiceDoesNotReconnectOrLoop',
                               'maintenanceContinuesWhileAnyConnectionIsActive'],
    'InputRejectionsTest': ['nativeCodesDistinguishPreparationQueueAndChangedTargets',
                            'repeatedRejectionsCoalesceWithinEachRecoveryCycle'],
    'core.I18nTest': ['systemLanguageOverridesAndUnsupportedFallback',
                      'translationsReorderArgumentsWithoutInterpretingUserData',
                      'switchingLanguageRefreshesLabelsAndKeepsInputLiteral'],
    'AgentNavigationTest': ['detailsSelectTheConnectionAndRejectARecreatedPane'],
    'HierarchyTest': ['panelStateSeparatesRunsOtherPanesAndOtherConnections', 'sharedHierarchyProjection',
                       'liveTitlesFollowActivePanesWithoutChangingIdentity'],
    'TextComposerTest': ['changedTargetsBeforeAndDuringPreparationDoNotSend', 'emptyAndOversizedPastesNeverEnterQueue',
                         'failedSendPreservesDraftAndNeverAutomaticallyRetries', 'sizeGenerationIsNotAnInputDestinationChange',
                         'unicodeSelectionDraftAndCompletionAreConnectionOwned'],
    'core.TerminalResizeQueueTest': ['continuousAnimationWaitsForTheLastChange',
                                    'failedNativeRequestCanBeRetriedAfterConnectionRecovers',
                                    'newConnectionImmediatelyUsesTheLatestViewport',
                                    'onlyOneRequestAndOneLatestValueAreRetained',
                                    'rejectedSubmissionRetainsTheViewportWithoutBusyRetrying',
                                    'resizeKeepsInputBlockedWithoutShowingConnectionProgress',
                                    'returningToTheSubmittedSizeNeedsNoRequest'],
    'CliLauncherTest': ['exactCommandsOnceForActiveShellAndPane', 'targetChangesAndUnavailableInputNeverSendOrReplay'],
    'MigrationContractTest': ['controlStreamFixturesAtEveryBoundary', 'escapeFixtures',
                              'commandFixtures', 'inputFixtures', 'settingsFixtures', 'stateFixtures'],
    'TmuxDiscoveryTest': ['resolvesSearchPathsAndQuotesExecutable', 'recordsFailuresAndPreservesTransportFailures',
                          'shellFallbackExplainsFailureAndReconnectClearsDetails'],
    'core.TerminalInputTest': ['binaryMouseReportsPreserveEveryByteAndRejectInvalidInput', 'everySupportedKeyAndToolbarOrder', 'mixedStepsUnicodeNewlinesAndNoImplicitEnter',
                               'emptyInvalidAndUtf8SizeLimits', 'oldPreferencesAndIndependentBindingsRoundTrip'],
    'PreferencesTest': ['repositoryRestoresMacrosAndLegacySettings', 'accentValidationFallbackContrastAndStoredChoice',
                        'languageChoiceSurvivesRestartAndLegacySettingsUseSystem'],
    'core.ProtocolTest': '''everyByteBoundaryAndUtf8Preserved commandBodyCannotForgeNotification
        unicodeMaySpanOutputEvents extendedOutputAndEmptyOutput malformedAndOversizedMessagesFailClosed
        inputIsHexAndNamesAreQuoted captureEscapingDiffersFromLiveOutput retryAndInputGates
        supportedVersionsAndAuthoritativeState'''.split(),
    'RendererTest': ['unicodeColorsCursorAlternateScreenAndResize',
                     'snapshotRestoresRowsWithoutNewlineDuplicationAndCursor', 'scrollbackIsBoundedAndLargeOutputFinishes'],
    'AuthenticationTest': ['passwordSuccessFailureAndGeneratedEd25519Key'],
    'IntegrationTest': ['plainSshShellAndHostileNamesAndErrorRecovery', 'hostTrustMismatchAndKeyAuthentication',
                        'tmuxDiscoveryUsesLoginPathForCreateListAttachAndReconnect',
                        'macrosUseSelectedPaneAndPlainShellOnce',
                        'actualSshControlInputExternalChangesAndProcessPreservation',
                        'simultaneousClientsSnapshotDuringOutputAndSessionDeletion'],
    'RecoveryTest': ['repeatedDisconnectBlocksInputPreservesPidAndDoesNotReplaceMissingSession'],
    'PerformanceTest': ['loopbackInputLatencyAndSustainedOutput'],
    'WindowsSecurityTest': ['currentUserDpapiRoundTripTamperAndPathValidation'],
}
EXPECTED = {'kim.jujin.dglc.' + cls + '.' + name for cls, names in TESTS.items() for name in names}


def verify(directory, mode):
    cases = {}
    for path in sorted(directory.glob('TEST-*.xml')):
        tree = ET.parse(path)
        for case in tree.findall('.//testcase'):
            name = case.attrib['classname'] + '.' + case.attrib['name'].removesuffix('[desktop]')
            if name in cases:
                raise ValueError('Duplicate test: ' + name)
            cases[name] = ('failed' if case.find('failure') is not None or case.find('error') is not None else
                           'skipped' if case.find('skipped') is not None else 'passed')
    if set(cases) != EXPECTED:
        raise ValueError('Missing or unexpected JVM tests: ' + str(sorted(set(cases) ^ EXPECTED)))
    allowed = set()
    if mode != 'windows':
        allowed |= {n for n in EXPECTED if '.WindowsSecurityTest.' in n}
    if mode != 'ssh':
        allowed |= {n for n in EXPECTED if any('.' + cls + '.' in n for cls in
                    ('IntegrationTest', 'RecoveryTest', 'PerformanceTest'))}
    for name, status in cases.items():
        if status != ('skipped' if name in allowed else 'passed'):
            raise ValueError('Unexpected test outcome: ' + name + ': ' + status)
    return {'tests': len(cases), 'passed': list(cases.values()).count('passed'),
            'failed': 0, 'skipped': len(allowed), 'expectedSkips': sorted(allowed), 'cases': cases}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--mode', choices=['linux', 'windows', 'ssh'], required=True)
    parser.add_argument('--output', type=str, required=True)
    args = parser.parse_args()
    report = {'schema': 1, 'complete': False, 'status': 'failed', 'context': context(), 'mode': args.mode}
    write_report(args.output, report)
    report.update(verify(ROOT / 'app/build/test-results/desktopTest', args.mode))
    report.update(complete=True, status='passed')
    write_report(args.output, report)
    print(f"JVM {args.mode}: {report['passed']} passed, {report['skipped']} expected skips")


if __name__ == '__main__':
    main()
