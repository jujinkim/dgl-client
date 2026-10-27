package kim.jujin.dglc.core

import kotlin.test.*

class TerminalViewportTest {
    @Test fun imeGeometryPreservesLogicalSizeAndRejectsDetachedUpdates() {
        val state = TerminalViewportState()
        state.attach()
        for (visible in listOf(600, 540, 360, 300, 400, 600, 300)) {
            assertTrue(state.update(TerminalViewport(1000, 600, visible, visible < 600)))
            assertEquals(600, state.viewport?.logicalHeight)
            assertTrue(state.accepts(state.version))
        }
        val previous = state.version
        state.detach()
        assertFalse(state.accepts(previous))
        assertFalse(state.accepts(state.version))
        state.attach()
        assertFalse(state.accepts(previous))
        assertTrue(state.accepts(state.version))
        assertFalse(state.update(TerminalViewport(0, 0, 0, false)))
        assertFalse(state.update(TerminalViewport(800, 400, 500, true)))
        assertTrue(state.update(TerminalViewport(800, 400, 200, true)))
        assertEquals(400, state.viewport?.logicalHeight)
    }
}
