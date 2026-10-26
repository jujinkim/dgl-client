package kim.jujin.dglc.core

import kotlin.test.*

class TerminalImeAnimationTest {
    @Test fun rendererGenerationsSurviveDetachAndRejectCancelledCallbacks() {
        val state = TerminalImeAnimation()
        val first = state.begin()
        val replacement = state.begin()
        assertFalse(state.end(first))
        val detached = state.cancel()
        assertTrue(detached > replacement)
        assertFalse(state.end(replacement))
        val reattached = state.begin()
        assertTrue(reattached > detached)
        assertFalse(state.end(replacement))
        assertTrue(state.end(reattached))
        assertFalse(state.end(reattached))
    }
}
