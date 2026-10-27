package kim.jujin.dglc.core

import kotlin.test.*

class TerminalRecoveryTest {
    @Test fun healthyCheckDimsScreenWhileRecoveryWaitsForEveryCurrentPane() {
        val gate = TerminalRecovery()
        gate.bind(1)
        val targets = mapOf("%1" to "new-1", "%2" to "new-2")
        gate.rendered(1, gate.cycle, "%1", "new-1")
        gate.rendered(1, gate.cycle, "%2", "new-2")
        assertFalse(gate.update(true, Phase.Ready, false, targets))
        val checking = ConnectionStatus(Phase.Ready, checking = true)
        assertFalse(checking.acceptsInput)
        assertEquals(t("연결 확인 중…"), checking.recoveryMessage)
        assertFalse(checking.showsConnectionProgress)
        assertFalse(checking.canRetryNow)
        assertFalse(checking.canStopRetrying)
        assertFalse(gate.update(false, Phase.Ready, false, targets))
        assertTrue(checking.copy(checking = false).acceptsInput)
        assertTrue(gate.update(false, Phase.Reconnecting, true, emptyMap()))
        val cycle = gate.cycle
        gate.begin()
        assertEquals(cycle, gate.cycle)
        val status = ConnectionStatus(Phase.Ready, checking = true, recovering = true)
        assertFalse(status.acceptsInput)
        assertEquals(t("연결 확인 중…"), status.recoveryMessage)
        assertEquals(t("연결 복구 중…"), status.copy(phase = Phase.Reconnecting).recoveryMessage)
        gate.rendered(1, cycle, "%1", "new-1")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%2", "old-2")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%2", "new-2")
        assertFalse(gate.update(false, Phase.Ready, false, targets))
        assertTrue(status.copy(checking = false, recovering = false).acceptsInput)
    }
    @Test fun checkResyncWaitsForChangedPanesAndAcceptsTheCurrentInFlightAck() {
        val gate = TerminalRecovery()
        gate.bind(1)
        val cycle = gate.cycle
        gate.rendered(1, cycle, "%1", "old")
        gate.rendered(1, cycle, "%2", "unchanged")
        assertFalse(gate.update(true, Phase.Ready, false, mapOf("%1" to "old", "%2" to "unchanged")))
        val targets = mapOf("%1" to "new", "%2" to "unchanged")
        // Polling may first observe the new target in the check-complete state.
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%1", "old")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%1", "new")
        assertFalse(gate.update(false, Phase.Ready, false, targets))
        // A snapshot started before resume may still be waiting on its ACK.
        gate.bind(2)
        assertTrue(gate.update(true, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%1", "new")
        gate.rendered(2, cycle, "%2", "unchanged")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(2, cycle, "%1", "new")
        assertFalse(gate.update(false, Phase.Ready, false, targets))
    }
    @Test fun staleSubscriptionsAndCyclesCannotClearRecoveryOrReplayInput() {
        val gate = TerminalRecovery()
        gate.bind(1); gate.begin()
        val oldCycle = gate.cycle
        gate.bind(2)
        gate.rendered(1, oldCycle, "%1", "current")
        assertTrue(gate.update(false, Phase.Ready, false, mapOf("%1" to "current")))
        gate.reset(); gate.bind(3); gate.begin()
        gate.rendered(3, oldCycle, "%1", "current")
        assertTrue(gate.update(false, Phase.Ready, false, mapOf("%1" to "current")))
        assertFalse(ConnectionStatus(Phase.Shell, recovering = true).acceptsInput)
        gate.rendered(3, gate.cycle)
        assertTrue(gate.update(false, Phase.Shell, false, mapOf("shell" to "current")))
        gate.rendered(3, gate.cycle, "shell", "old")
        assertTrue(gate.update(false, Phase.Shell, false, mapOf("shell" to "current")))
        gate.rendered(3, gate.cycle, "shell", "current")
        assertFalse(gate.update(false, Phase.Shell, false, mapOf("shell" to "current")))
    }
    @Test fun failuresStayDimmedAndResizeAloneDoesNotStartRecovery() {
        val gate = TerminalRecovery()
        gate.bind(1)
        assertFalse(gate.update(false, Phase.Restoring, false, emptyMap()))
        assertNull(ConnectionStatus(Phase.Restoring, resizing = true).recoveryMessage)
        assertTrue(gate.update(false, Phase.Reconnecting, true, emptyMap()))
        gate.rendered(1, gate.cycle)
        assertTrue(gate.update(false, Phase.Failed, false, emptyMap()))
        assertEquals("failure", ConnectionStatus(Phase.Failed, "failure", recovering = true).recoveryMessage)
        val outputFailure = ConnectionStatus(Phase.Ready, "screen failure", canRetryNow = true, recovering = true)
        assertEquals("screen failure", outputFailure.recoveryMessage)
        assertFalse(outputFailure.acceptsInput)
        gate.unbind()
        assertTrue(gate.update(false, Phase.Shell, false, emptyMap()))
    }
}
