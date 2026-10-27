package kim.jujin.dglc

import kotlin.test.*

class SnackbarStateTest {
    @Test fun defaultPresetsAndCustomMillisecondsExpireAtTheirBoundary() {
        var nanos = 0L
        val state = SnackbarState { nanos }
        state.show("default")
        assertEquals(SnackbarDuration.Normal, state.current.value!!.duration)
        for (duration in listOf(SnackbarDuration.Short, SnackbarDuration.Normal, SnackbarDuration.Long, SnackbarDuration.Millis(1_234))) {
            nanos = 0
            state.show("timed", duration)
            val id = state.current.value!!.id
            val ms = duration.millis!!
            assertEquals(1f, state.tick(id))
            nanos = ms * 500_000
            assertEquals(0.5f, state.tick(id))
            nanos = ms * 1_000_000 - 1
            state.tick(id)
            assertNotNull(state.current.value)
            nanos++
            assertEquals(0f, state.tick(id))
            assertNull(state.current.value)
        }
        state.show("direct", durationMs = 37)
        assertEquals(37L, state.current.value!!.duration.millis)
        state.show("zero", durationMs = 0)
        assertEquals(0f, state.tick(state.current.value!!.id))
        assertNull(state.current.value)
        state.show("kept")
        assertFailsWith<IllegalArgumentException> { state.show("invalid", durationMs = -1) }
        assertEquals("kept", state.current.value!!.message)
    }

    @Test fun sameTextRestartsAndOldExpiryOrDismissCannotRemoveReplacement() {
        var nanos = 0L
        val state = SnackbarState { nanos }
        state.show("same")
        val old = state.current.value!!.id
        nanos = 4_000_000_000
        state.show("same")
        val replacement = state.current.value!!.id
        assertNotEquals(old, replacement)
        assertEquals(1f, state.tick(replacement))
        nanos = 5_000_000_000
        assertNull(state.tick(old))
        state.dismiss(old)
        assertEquals(replacement, state.current.value!!.id)
        assertEquals(0.8f, state.tick(replacement))
        state.dismiss(replacement)
        assertNull(state.current.value)
        assertNull(state.tick(replacement))
    }

    @Test fun indefiniteHasNoGaugeOrExpiryAndCanBeClosedOrReplaced() {
        var nanos = 0L
        val state = SnackbarState { nanos }
        state.show("permanent", SnackbarDuration.Indefinite)
        val id = state.current.value!!.id
        nanos = 1_000_000_000_000
        assertNull(state.tick(id))
        assertNotNull(state.current.value)
        state.dismiss(id)
        assertNull(state.current.value)
        state.show("permanent", SnackbarDuration.Indefinite)
        state.show("timed")
        assertEquals(SnackbarDuration.Normal, state.current.value!!.duration)
        assertEquals(1f, state.tick(state.current.value!!.id))
    }
}
