package kim.jujin.dglc.core

import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import kotlin.test.*

@OptIn(ExperimentalCoroutinesApi::class)
class TerminalOutputTest {
    @Test fun renderTimeoutReleasesBackpressureBeforeCleanupAndLateAck() = runTest {
        val ack = CompletableDeferred<Unit>()
        val cleanup = CompletableDeferred<Unit>()
        val events = mutableListOf<String>()
        val job = launchTerminalOutput(this, null,
            consume = { withTimeout(15_000) { ack.await() }; events += "rendered" },
            failed = { assertIs<TimeoutCancellationException>(it); events += "failed" },
            release = { events += "released" },
            cleanup = { cleanup.await(); events += "cleaned" })
        runCurrent()
        advanceTimeBy(15_001)
        assertEquals(listOf("failed", "released"), events)
        ack.complete(Unit)
        runCurrent()
        assertEquals(listOf("failed", "released"), events)
        cleanup.complete(Unit); job.join()
        assertEquals(listOf("failed", "released", "cleaned"), events)
    }

    @Test fun cancelledReplacementsJoinOldCleanupBeforeUsingOrClosingSurface() = runTest {
        val cleanup = CompletableDeferred<Unit>()
        val events = mutableListOf<String>()
        val old = launchTerminalOutput(this, null,
            consume = { awaitCancellation() }, failed = { fail("Owner cancellation is not a failure") },
            release = { events += "old-release" }, cleanup = { cleanup.await(); events += "old-clean" })
        runCurrent(); old.cancel(); runCurrent()
        val skipped = launchTerminalOutput(this, old,
            consume = { fail("Cancelled before dispatch") }, failed = { fail("Intentional cancellation") },
            release = { events += "skipped-release" }, cleanup = { events += "skipped-clean" })
        skipped.cancel()
        val replacement = launchTerminalOutput(this, skipped,
            consume = { events += "new-write" }, failed = { fail("Unexpected failure") },
            release = { events += "new-release" }, cleanup = { events += "new-clean" })
        runCurrent()
        assertEquals(listOf("old-release"), events)
        assertFalse(skipped.isCompleted)
        cleanup.complete(Unit); replacement.join()
        assertEquals(listOf("old-release", "old-clean", "skipped-release", "skipped-clean", "new-write", "new-release", "new-clean"), events)
    }
}
