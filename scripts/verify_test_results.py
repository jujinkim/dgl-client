#!/usr/bin/env python3
"""Require every named JVM test; only explicitly platform-scoped skips are allowed."""
import argparse
import xml.etree.ElementTree as ET
from evidence import ROOT, context, write_report

TESTS = {
    'core.TerminalKeyboardVisibilityTest': ['entryDismissalAndFocusLossRequestOnceAfterAnimations',
                                           'settingsBackgroundInactiveTargetsAndDisposalCancelDelayedRequests'],
    'core.TerminalPresentationTest': ['freshWindowWaitsForEveryVisiblePaneAndSnapshotCompletion',
                                       'rapidSwitchRebindAndConnectionChangesRejectOldAcknowledgements',
                                       'failureAndEmptyLayoutsNeverUncoverPartialScreensAndShellCanPresent'],
    'MobileChromeTest': ['notificationPositionRespectsAnchorScreenInsetsAndKeyboard'],
    'SnackbarStateTest': ['defaultPresetsAndCustomMillisecondsExpireAtTheirBoundary',
                          'sameTextRestartsAndOldExpiryOrDismissCannotRemoveReplacement',
                          'indefiniteHasNoGaugeOrExpiryAndCanBeClosedOrReplaced'],
    'core.TerminalImeAnimationTest': ['rendererGenerationsSurviveDetachAndRejectCancelledCallbacks'],
    'core.TerminalViewportTest': ['imeGeometryPreservesLogicalSizeAndRejectsDetachedUpdates'],
    'core.TerminalFontSizeTest': ['pinchAccumulatesSmallChangesClampsAndIgnoresInvalidFactors',
                                'overridesStayIndependentAndResetUsesTheLatestDefault'],
    'core.BackgroundResumeTest': ['backgroundStopKeepsTargetAndResumesOnlyOnce',
                                 'explicitStopsFailuresAndMissingSessionsNeverReserveResume'],
    'core.TerminalOutputTest': ['renderTimeoutReleasesBackpressureBeforeCleanupAndLateAck',
                                'cancelledReplacementsJoinOldCleanupBeforeUsingOrClosingSurface'],
    'core.TerminalRecoveryTest': ['healthyCheckDimsScreenWhileRecoveryWaitsForEveryCurrentPane',
                                  'checkResyncWaitsForChangedPanesAndAcceptsTheCurrentInFlightAck',
                                  'staleSubscriptionsAndCyclesCannotClearRecoveryOrReplayInput',
                                  'failuresStayDimmedAndResizeAloneDoesNotStartRecovery'],
    'WorkspaceTest': ['imeProjectionKeepsEveryGroupAndLogicalTerminalSize', 'splitMoveMergeSelectionAndEmptyGroups', 'minimumSpaceGroupLimitAndResizePreserveLayout',
                      'validatedDropsAndCancelledDragsDoNotMutateState', 'geometryOnlyPersistenceAndMalformedFilesStartEmpty',
                      'sessionsReserveBeforeConnectAndReuseAcrossGroups', 'staleConnectionGenerationsRejectDelayedTargetsAndModalGates',
                      'notificationsReuseSessionTabsAndObserveEveryVisibleConnection',
                      'titleUpdatesFollowForegroundAndVisibleGroupsWithoutSurfaceSubscriptions'],
    'ConnectionLifetimeTest': ['homeLockAndRecreationReuseConnectionsUntilTaskRemoval',
                               'onlyActualBackgroundTransitionsResumeOnce',
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
                       'liveTitlesFollowActivePanesWithoutChangingIdentity',
                       'tabTitlesFollowActiveWindowAndPaneAndIgnoreOtherSessions'],
    'TextComposerTest': ['changedTargetsBeforeAndDuringPreparationDoNotSend', 'emptyAndOversizedPastesNeverEnterQueue',
                         'failedSendPreservesDraftAndNeverAutomaticallyRetries', 'sizeGenerationIsNotAnInputDestinationChange',
                         'unicodeSelectionDraftAndCompletionAreConnectionOwned',
                         'enterWaitsForTextCompletionAndIsAcknowledgedSeparately',
                         'enterFailurePreservesDeliveredTextWithoutRetry', 'changedTargetsAfterTextDoNotSendEnter'],
    'core.TerminalResizeQueueTest': ['continuousAnimationSubmitsImmediatelyThenLatestEvery50Millis',
                                    'failedIntermediateSizeInvalidatesPreviouslyAppliedGeometry',
                                    'failedNativeRequestCanBeRetriedAfterConnectionRecovers',
                                    'newConnectionImmediatelyUsesTheLatestViewport',
                                    'onlyOneRequestAndOneLatestValueAreRetained',
                                    'rejectedSubmissionRetainsTheViewportWithoutBusyRetrying',
                                    'resizeAcceptsKeysButBlocksPointersWithoutConnectionProgress',
                                    'returningToTheSubmittedSizeNeedsNoRequest'],
    'CliLauncherTest': ['modifiersToggleUnionConsumeAndRejectUnsupportedWithoutReplay', 'targetChangesRecoveryAndModalClearsCannotLeakModifiers', 'exactCommandsOnceForActiveShellAndPane', 'targetChangesAndUnavailableInputNeverSendOrReplay',
                        'viewportRestorationRetainsRequestsButRecoveryAndChangedTargetsRejectThem'],
    'MigrationContractTest': ['controlStreamFixturesAtEveryBoundary', 'escapeFixtures',
                              'commandFixtures', 'inputFixtures', 'settingsFixtures', 'stateFixtures'],
    'TmuxDiscoveryTest': ['resolvesSearchPathsAndQuotesExecutable', 'recordsFailuresAndPreservesTransportFailures',
                          'shellFallbackExplainsFailureAndReconnectClearsDetails'],
    'core.TerminalInputTest': ['shortcutOrderPreviewCancelExclusionAndLegacyListsSurviveRestart', 'binaryMouseReportsPreserveEveryByteAndRejectInvalidInput', 'everySupportedKeyAndToolbarOrder', 'mixedStepsUnicodeNewlinesAndNoImplicitEnter',
                               'emptyInvalidAndUtf8SizeLimits', 'oldPreferencesAndIndependentBindingsRoundTrip'],
    'PreferencesTest': ['androidKeyboardModesSurviveRestartAndLegacyUnknownUseAuto', 'alwaysVisibleKeyboardPreservesModeAndSurvivesRestart',
                        'repositoryRestoresMacrosAndLegacySettings', 'accentValidationFallbackContrastAndStoredChoice',
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
    parser.add_argument('--mode', choices=['linux', 'ssh'], required=True)
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
