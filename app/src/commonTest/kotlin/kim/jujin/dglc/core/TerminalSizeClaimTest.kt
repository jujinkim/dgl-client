package kim.jujin.dglc.core

import kotlin.test.*

class TerminalSizeClaimTest {
    private class Tab(val name: String) { override fun toString() = name }

    @Test fun activationAndFocusMovesClaimShownTabsOnceWithTheFocusedLast() {
        val (a, b, hidden) = listOf(Tab("a"), Tab("b"), Tab("hidden"))
        val claims = TerminalFocusClaims<Tab>()
        assertEquals(emptyList(), claims.update(false, a, listOf(a, b)), "a background app never claims")
        assertEquals(listOf(b, a), claims.update(true, a, listOf(a, b)), "the focused tab wins a window both show")
        assertEquals(emptyList(), claims.update(true, a, listOf(a, b)), "an unchanged focus is not a new claim")
        assertEquals(emptyList(), claims.update(true, a, listOf(b, a)))
        // Losing and regaining OS focus is an activation edge.
        assertEquals(emptyList(), claims.update(false, a, listOf(a, b)))
        assertEquals(listOf(b, a), claims.update(true, a, listOf(a, b)))
        // An in-app move to another shown tab or group.
        assertEquals(listOf(a, b), claims.update(true, b, listOf(a, b)))
        assertFalse(hidden in claims.update(true, a, listOf(a, b)), "hidden tabs never claim")
    }

    @Test fun aNewSelectionWaitsForItsLayoutAndNeverClaimsTheOldOne() {
        val (old, next) = listOf(Tab("old"), Tab("next"))
        val claims = TerminalFocusClaims<Tab>()
        assertEquals(listOf(old), claims.update(true, old, listOf(old)))
        // The selection changes before the screen shows it.
        assertEquals(emptyList(), claims.update(true, next, listOf(old)))
        assertEquals(listOf(next), claims.update(true, next, listOf(next)))
        // A hidden terminal (explorer, modal) keeps the claim until it is shown again.
        assertEquals(emptyList(), claims.update(false, next, emptyList()))
        assertEquals(emptyList(), claims.update(true, next, emptyList()))
        assertEquals(listOf(next), claims.update(true, next, listOf(next, next)), "duplicates claim once")
        assertEquals(emptyList(), claims.update(true, next, listOf(next)))
        // No selected tab: every shown terminal is claimed once.
        assertEquals(listOf(old), claims.update(true, null, listOf(old)))
    }

    @Test fun inputClaimsOnlyAfterATakeoverAndOneAtATime() {
        val claims = TerminalSizeClaims()
        var reads = 0
        fun displaced(value: Boolean): () -> Boolean = { reads++; value }
        assertFalse(claims.shouldClaim(false, displaced(false)), "ordinary keys add no request between inputs")
        assertEquals(1, reads)
        assertTrue(claims.shouldClaim(true, displaced(false)), "focus claims do not depend on a takeover")
        assertEquals(1, reads, "focus claims do not read the state")
        assertTrue(claims.shouldClaim(false, displaced(true)))
        claims.submitted(7u)
        assertTrue(claims.pending)
        reads = 0
        assertFalse(claims.shouldClaim(false, displaced(true)), "a claim in flight is not repeated per key")
        assertEquals(0, reads)
        assertTrue(claims.shouldClaim(true, displaced(true)))
        claims.submitted(8u)
        // Completions are recognized so their errors stay silent; other ids are not claims.
        assertFalse(claims.complete(9u))
        assertTrue(claims.complete(7u)); assertFalse(claims.complete(7u))
        assertTrue(claims.pending)
        claims.reset()
        assertFalse(claims.pending)
        assertFalse(claims.complete(8u), "a replaced native connection never reports old requests")
        assertTrue(claims.shouldClaim(false, displaced(true)))
    }

    @Test fun aShownTerminalOnTheFocusedServerWindowNeverClaims() {
        val (a, b, c) = listOf(Tab("a"), Tab("b"), Tab("c"))
        val windows = mutableMapOf(a to "@1", b to "@1", c to "@2")
        val same: (Tab, Tab) -> Boolean = { x, y -> windows[x] != null && windows[x] == windows[y] }
        val claims = TerminalFocusClaims<Tab>()
        // Independent native actors: "focused last" would not decide the shared window.
        assertEquals(listOf(c, a), claims.update(true, a, listOf(a, b, c), same))
        windows[b] = "@3"
        assertEquals(listOf(a, c, b), claims.update(true, b, listOf(a, b, c), same), "other windows still claim")
        assertTrue(claims.stillClaims(a) && claims.stillClaims(b))
        // The filter only ever drops non-focused terminals.
        assertEquals(listOf(c), claims.update(true, c, listOf(c), { _, _ -> true }))

        val host = HostProfile("one", "alias", "Example.test", username = "u")
        val alias = HostProfile("two", "other", "example.TEST", username = "u")
        assertTrue(TerminalFocusClaims.sameServerWindow(host, "@1", host, "@1"))
        assertTrue(TerminalFocusClaims.sameServerWindow(host, "@1", alias, "@1"), "another profile of the same account")
        assertFalse(TerminalFocusClaims.sameServerWindow(host, "@1", alias, "@2"))
        assertFalse(TerminalFocusClaims.sameServerWindow(host, null, host, null), "an unknown window is never shared")
        assertFalse(TerminalFocusClaims.sameServerWindow(host, "@1", alias.copy(username = "v"), "@1"), "another account is another server")
        assertFalse(TerminalFocusClaims.sameServerWindow(host, "@1", alias.copy(port = 2222), "@1"))
    }

    @Test fun aWaitingFocusClaimHoldsOnlyWhileItsTerminalStaysShownAndFocused() {
        val (a, b) = listOf(Tab("a"), Tab("b"))
        val claims = TerminalFocusClaims<Tab>()
        assertFalse(claims.stillClaims(a), "nothing was claimed yet")
        assertEquals(listOf(b, a), claims.update(true, a, listOf(a, b)))
        assertTrue(claims.stillClaims(a) && claims.stillClaims(b))
        // Recomposition keeps it; hiding one terminal ends only that one.
        claims.update(true, a, listOf(a, b)); assertTrue(claims.stillClaims(b))
        claims.update(true, a, listOf(a)); assertFalse(claims.stillClaims(b)); assertTrue(claims.stillClaims(a))
        claims.update(true, a, listOf(a, b)); assertFalse(claims.stillClaims(b), "showing it again is not a new focus")
        // Backgrounding ends it, and so does a newer focus, even one still waiting for layout.
        claims.update(false, a, listOf(a, b)); assertFalse(claims.stillClaims(a))
        assertEquals(listOf(b, a), claims.update(true, a, listOf(a, b))); assertTrue(claims.stillClaims(a))
        assertEquals(emptyList(), claims.update(true, Tab("next"), listOf(a, b)))
        assertFalse(claims.stillClaims(a) || claims.stillClaims(b))
    }

    @Test fun aWaitingFocusClaimRechecksTheServerWindowWhenItFires() {
        val (a, b, c) = listOf(Tab("a"), Tab("b"), Tab("c"))
        // b is still reconnecting: its window is unknown when the batch is built.
        val windows = mutableMapOf<Tab, String?>(a to "@1", b to null, c to "@2")
        val same: (Tab, Tab) -> Boolean = { x, y -> windows[x] != null && windows[x] == windows[y] }
        val claims = TerminalFocusClaims<Tab>()
        assertEquals(listOf(b, c, a), claims.update(true, a, listOf(a, b, c), same))
        // At Ready b shows the focused terminal's server window and must not race it.
        windows[b] = "@1"
        assertFalse(claims.stillClaims(b, same), "now on the focused server window")
        assertTrue(claims.stillClaims(a, same), "the focused terminal always keeps its claim")
        assertTrue(claims.stillClaims(c, same), "another window still claims")
        windows[b] = "@3"
        assertTrue(claims.stillClaims(b, same))
        // No focused terminal: nothing to race.
        assertEquals(listOf(b), claims.update(true, null, listOf(b), same))
        assertTrue(claims.stillClaims(b) { _, _ -> true })
    }

    @Test fun focusClaimsRefusedBeforeAttachWaitForTheNextReadyState() {
        val claims = TerminalSizeClaims()
        var runs = 0
        val retry = { runs++; Unit }
        for (phase in listOf(Phase.Connecting, Phase.Authenticating, Phase.Reconnecting, Phase.Restoring)) {
            claims.refused("NotReady", phase, retry)
            assertTrue(claims.waiting, "$phase")
            assertNull(claims.ready(phase), "only Ready sends it")
            assertSame(retry, claims.ready(Phase.Ready)); assertNull(claims.ready(Phase.Ready), "once")
        }
        // Nothing to claim in a shell or a selection; input claims never wait; other errors drop it.
        for (phase in listOf(Phase.Shell, Phase.Selecting, Phase.MissingSession, Phase.Failed, Phase.Disconnected)) {
            claims.refused("NotReady", phase, retry); assertFalse(claims.waiting, "$phase")
        }
        claims.refused("NotReady", Phase.Reconnecting, null); assertFalse(claims.waiting)
        claims.refused("NotReady", Phase.Reconnecting, retry)
        claims.refused("QueueFull", Phase.Reconnecting, retry); assertFalse(claims.waiting, "a newer refusal replaces it")
        // A sent focus claim replaces a waiting one; one that loses its transport waits again.
        claims.refused("NotReady", Phase.Reconnecting, retry)
        claims.submitted(1u, retry); assertFalse(claims.waiting)
        assertTrue(claims.complete(1u, "Network", Phase.Reconnecting)); assertTrue(claims.waiting)
        claims.submitted(2u, retry); claims.submitted(3u)
        assertTrue(claims.complete(3u, "NotReady", Phase.Reconnecting)); assertFalse(claims.waiting, "an input claim never waits")
        assertTrue(claims.complete(2u, "StaleGeneration", Phase.Ready)); assertFalse(claims.waiting)
        // A replaced native connection keeps the waiting claim; closing the adapter drops it.
        claims.refused("NotReady", Phase.Connecting, retry)
        claims.reset(); assertTrue(claims.waiting)
        claims.close(); assertFalse(claims.waiting)
        assertEquals(0, runs, "the owner runs the returned claim")
    }

    @Test fun onlyResizeRacesOfPointerReportsQueuedBehindAClaimStaySilent() {
        val claims = TerminalSizeClaims()
        val sgr = "\u001b[<0;1;1M".encodeToByteArray()
        val legacy = byteArrayOf(0x1b, '['.code.toByte(), 'M'.code.toByte(), 32, 0x85.toByte(), 0xff.toByte())
        assertTrue(TerminalSizeClaims.isPointerReport(sgr) && TerminalSizeClaims.isPointerReport(legacy))
        for (keys in listOf("a", "\u001b[A", "\u001b[200~x\u001b[201~", "\u001b[", "\u001b[I"))
            assertFalse(TerminalSizeClaims.isPointerReport(keys.encodeToByteArray()), keys)
        assertFalse(claims.behindClaim(sgr), "no claim in flight")
        claims.submitted(1u)
        assertTrue(claims.behindClaim(sgr) && claims.behindClaim(legacy))
        assertFalse(claims.behindClaim("a".encodeToByteArray()), "keyboard input stays admitted and keeps its notices")
        for (id in 10u..13u) claims.reportSubmitted(id.toULong())
        assertTrue(claims.completeInput(10u, "StaleGeneration"))
        assertTrue(claims.completeInput(11u, "NotReady"))
        assertFalse(claims.completeInput(12u, "QueueFull"))
        assertFalse(claims.completeInput(13u, "Closed"))
        assertFalse(claims.completeInput(10u, "StaleGeneration"), "each report completes once")
        assertFalse(claims.completeInput(20u, "StaleGeneration"), "other input keeps its notice")
        claims.reportSubmitted(14u); claims.reset()
        assertFalse(claims.completeInput(14u, "StaleGeneration"), "a replaced native connection never reports old requests")
        assertTrue(TerminalSizeClaims.reportRace("NotReady") && TerminalSizeClaims.reportRace("StaleGeneration"))
        assertFalse(TerminalSizeClaims.reportRace("QueueFull") || TerminalSizeClaims.reportRace("Closed") || TerminalSizeClaims.reportRace(null))
    }

    @Test fun onlyMarkedBridgeInputClaims() {
        for (type in listOf("input", "input-binary", "keyboard")) {
            assertTrue(TerminalSizeClaims.fromBridge(type, true), type)
            assertFalse(TerminalSizeClaims.fromBridge(type, false), "$type replies and unmarked input never claim")
        }
        for (type in listOf("blocked-input", "focus", "resize", "resize-pane", "ack", "selection", "ready"))
            assertFalse(TerminalSizeClaims.fromBridge(type, true), type)
    }
}
