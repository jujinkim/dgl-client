package kim.jujin.dglc.core

import kotlin.test.*

class TerminalPresentationTest {
    @Test fun freshWindowWaitsForEveryVisiblePaneAndSnapshotCompletion() {
        val gate = TerminalPresentation()
        gate.bind(1); gate.observe("connection:session:window")
        val cycle = gate.cycle
        val visible = setOf("%1", "%2")
        val targets = mapOf("%1" to "fresh1", "%2" to "fresh2", "%3" to "hidden")
        fun ready(phase: Phase = Phase.Ready, blocked: Boolean = false) = gate.ready(1, cycle, phase, visible, targets, blocked)
        assertFalse(ready())
        gate.rendered(1, cycle, "%1", "fresh1")
        assertFalse(ready())
        gate.rendered(1, cycle, "%2", "old2")
        assertFalse(ready())
        gate.rendered(1, cycle, "%2", "fresh2")
        assertFalse(ready(Phase.Restoring))
        assertFalse(ready(blocked = true))
        assertFalse(gate.ready(1, cycle, Phase.Ready, visible, mapOf("%1" to "fresh1"), false))
        assertTrue(ready())
        assertTrue(gate.pending, "Readiness is not presentation until the renderer uncovers the window")
        gate.presented(1, cycle)
        assertFalse(gate.pending)
        gate.observe("connection:session:window")
        assertFalse(gate.pending, "Focus and size changes in the same window do not start a switch")
        assertFalse(ConnectionStatus(Phase.Ready, presenting = true).acceptsInput)
        assertFalse(ConnectionStatus(Phase.Restoring, resizing = true, presenting = true).acceptsPointer)
        assertTrue(ConnectionStatus(Phase.Restoring, resizing = true).acceptsInput)
    }

    @Test fun rapidSwitchRebindAndConnectionChangesRejectOldAcknowledgements() {
        val gate = TerminalPresentation()
        gate.bind(1); gate.observe("1:windowA")
        val first = gate.cycle
        gate.observe("1:windowB"); gate.observe("1:windowA")
        val second = gate.cycle
        val visible = setOf("%1"); val targets = mapOf("%1" to "fresh")
        gate.rendered(1, first, "%1", "fresh"); gate.presented(1, first)
        assertTrue(gate.pending)
        assertFalse(gate.ready(1, second, Phase.Ready, visible, targets, false))
        gate.rendered(1, second, "%1", "fresh")
        assertTrue(gate.ready(1, second, Phase.Ready, visible, targets, false))
        gate.bind(2); gate.observe("1:windowA")
        gate.rendered(1, second, "%1", "fresh"); gate.presented(1, second)
        assertFalse(gate.ready(2, gate.cycle, Phase.Ready, visible, targets, false))
        val previousConnection = gate.cycle
        gate.observe("2:windowA")
        gate.rendered(2, previousConnection, "%1", "fresh")
        assertFalse(gate.ready(2, gate.cycle, Phase.Ready, visible, targets, false))
        gate.unbind()
        gate.rendered(2, gate.cycle, "%1", "fresh"); gate.observe("ignored")
        assertFalse(gate.pending)
    }

    @Test fun failureAndEmptyLayoutsNeverUncoverPartialScreensAndShellCanPresent() {
        val gate = TerminalPresentation()
        gate.bind(1); gate.observe("shell")
        gate.rendered(1, gate.cycle, "shell", "fresh")
        assertFalse(gate.ready(1, gate.cycle, Phase.Failed, setOf("shell"), mapOf("shell" to "fresh"), false))
        assertFalse(gate.ready(1, gate.cycle, Phase.Ready, emptySet(), emptyMap(), false))
        assertFalse(gate.ready(1, gate.cycle, Phase.Shell, setOf("shell"), mapOf("shell" to "fresh"), true))
        assertTrue(gate.pending)
        assertTrue(gate.ready(1, gate.cycle, Phase.Shell, setOf("shell"), mapOf("shell" to "fresh"), false))
        gate.presented(1, gate.cycle)
        assertFalse(gate.pending)
    }

    @Test fun selectingCurtainAsksForASessionInsteadOfRestoring() {
        val selecting = ConnectionStatus(Phase.Selecting, t("tmux 세션을 선택하거나 만드세요."), presenting = true)
        assertTrue(selecting.presentationNotice)
        assertEquals(t("tmux 세션을 선택하거나 만드세요."), selecting.presentationMessage)
        val restoring = ConnectionStatus(Phase.Restoring, t("터미널 복원 중…"), presenting = true)
        assertFalse(restoring.presentationNotice)
        assertEquals(t("터미널 복원 중…"), restoring.presentationMessage)
        assertEquals(t("터미널 복원 중…"), ConnectionStatus(Phase.Ready, "connected", presenting = true).presentationMessage)
        assertEquals("failure", ConnectionStatus(Phase.Failed, "failure", presenting = true).presentationMessage)
        assertEquals("retry", ConnectionStatus(Phase.Restoring, "retry", canRetryNow = true, presenting = true).presentationMessage)
    }

    @Test fun reattachToLastPresentedWindowPreviewsUntilPresented() {
        val gate = TerminalPresentation()
        val visible = setOf("%1"); val targets = mapOf("%1" to "fresh")
        fun present(id: Long) { gate.rendered(id, gate.cycle, "%1", "fresh"); gate.presented(id, gate.cycle) }
        gate.bind(1); gate.observe("1:s:A")
        assertFalse(gate.preview, "A first presentation has no earlier frame")
        present(1)
        assertEquals("1:s:A", gate.lastPresented)
        gate.unbind()
        assertEquals("1:s:A", gate.lastPresented, "A hidden tab keeps the frame it presented last")

        gate.bind(2); gate.observe("1:s:A")
        assertTrue(gate.pending); assertTrue(gate.preview)
        assertFalse(gate.ready(2, gate.cycle, Phase.Ready, visible, targets, false), "Preview still waits for fresh panes")
        assertFalse(ConnectionStatus(Phase.Ready, presenting = true, presentingPreview = true).acceptsInput)
        present(2)
        assertFalse(gate.pending); assertFalse(gate.preview)

        gate.observe("1:s:B")
        assertFalse(gate.preview, "A tmux window switch keeps the curtain")
        present(2); gate.unbind()
        gate.bind(3); gate.observe("2:s:B")
        assertFalse(gate.preview, "A new connection generation keeps the curtain")
        present(3); gate.unbind()

        gate.bind(4); gate.observe("2:s:B")
        assertTrue(gate.preview)
        gate.unbind()
        assertNull(gate.lastPresented, "An interrupted restoration never previews a partial frame")
        gate.bind(5); gate.observe("2:s:B")
        assertFalse(gate.preview)
        present(5)
        gate.observe("2:s:C"); gate.observe("2:s:B")
        assertFalse(gate.preview, "Leaving a window mid-restoration forgets the frame")
    }
}
