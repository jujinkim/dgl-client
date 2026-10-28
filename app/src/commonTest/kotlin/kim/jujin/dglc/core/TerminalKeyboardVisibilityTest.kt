package kim.jujin.dglc.core

import kotlin.test.*

class TerminalKeyboardVisibilityTest {
    private val active = TerminalKeyboardVisibility.State(enabled = true, eligible = true)

    @Test fun entryDismissalAndFocusLossRequestOnceAfterAnimations() {
        val policy = TerminalKeyboardVisibility()
        val first = assertNotNull(policy.update(active))
        assertTrue(policy.isCurrent(first))
        repeat(100) { assertNull(policy.update(active)) }
        val shown = active.copy(visible = true, focused = true)
        assertNull(policy.update(shown))
        assertFalse(policy.isCurrent(first))
        assertNotNull(policy.update(shown.copy(focused = false)))
        assertNull(policy.update(shown.copy(animating = true)))
        assertNull(policy.update(active.copy(animating = true)))
        val reopened = assertNotNull(policy.update(active.copy(focused = true)))
        assertTrue(policy.isCurrent(reopened))
        assertFalse(policy.isCurrent(first))
    }

    @Test fun settingsBackgroundInactiveTargetsAndDisposalCancelDelayedRequests() {
        val policy = TerminalKeyboardVisibility()
        // Eligibility excludes modal editors, background windows and inactive/hidden terminals.
        repeat(4) {
            val request = assertNotNull(policy.update(active))
            assertNull(policy.update(active.copy(eligible = false)))
            assertFalse(policy.isCurrent(request))
            assertNull(policy.update(active.copy(eligible = false)))
        }
        val request = assertNotNull(policy.update(active))
        assertNull(policy.update(active.copy(enabled = false)))
        assertFalse(policy.isCurrent(request))
        val replacement = assertNotNull(policy.update(active))
        assertNull(policy.update(TerminalKeyboardVisibility.State()))
        assertFalse(policy.isCurrent(replacement))
        assertNotNull(policy.update(active))
        assertFalse(policy.isCurrent(replacement))
    }
}
