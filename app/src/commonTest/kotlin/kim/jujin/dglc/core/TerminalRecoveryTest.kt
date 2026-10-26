package kim.jujin.dglc.core

import kotlin.test.*

class TerminalRecoveryTest {
    @Test fun resumeCoalescesAndBlocksInputUntilCurrentSnapshotsAreDisplayed() {
        val gate = TerminalRecovery()
        gate.bind(1)
        val targets = mapOf("%1" to "new-1", "%2" to "new-2")
        assertTrue(gate.update(true, Phase.Ready, false, targets))
        val cycle = gate.cycle
        gate.begin()
        assertEquals(cycle, gate.cycle)
        val status = ConnectionStatus(Phase.Ready, checking = true, recovering = true)
        assertFalse(status.acceptsInput)
        assertEquals(t("연결 확인 중…"), status.recoveryMessage)
        gate.rendered(1, cycle, "%1", "new-1")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%2", "old-2")
        assertTrue(gate.update(false, Phase.Ready, false, targets))
        gate.rendered(1, cycle, "%2", "new-2")
        assertFalse(gate.update(false, Phase.Ready, false, targets))
        assertTrue(status.copy(checking = false, recovering = false).acceptsInput)
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
        assertFalse(gate.update(false, Phase.Shell, false, emptyMap()))
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
        gate.unbind()
        assertTrue(gate.update(false, Phase.Shell, false, emptyMap()))
    }
}
