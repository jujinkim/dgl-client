package kim.jujin.dglc.core

import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import kotlin.test.*

@OptIn(ExperimentalCoroutinesApi::class)
class PollCadenceTest {
    @Test fun activityKeepsTheFastRateThroughTheLingerThenIdles() {
        val cadence = PollCadence()
        assertEquals(100, cadence.interval(8, 100, 0))
        cadence.mark(1_000)
        assertEquals(8, cadence.interval(8, 100, 1_000))
        assertEquals(16, cadence.interval(16, 100, 1_000 + PollCadence.LINGER - 1))
        assertEquals(100, cadence.interval(8, 100, 1_000 + PollCadence.LINGER))
        cadence.wake(5_000)
        assertEquals(8, cadence.interval(8, 100, 5_001))
    }

    @Test fun wakeEndsEveryIdleWaitAtOnceAndIsKeptForABusyLoop() = runTest {
        val cadence = PollCadence()
        val output = cadence.Waiter(PollCadence.OUTPUT_ACTIVE, PollCadence.IDLE)
        val state = cadence.Waiter(PollCadence.STATE_ACTIVE, PollCadence.IDLE)
        // Idle: both loops wait the long interval.
        val idle = launch { assertFalse(output.wait(0)) }
        advanceTimeBy(PollCadence.IDLE - 1); assertTrue(idle.isActive)
        advanceTimeBy(2); assertTrue(idle.isCompleted)
        // A wake releases both waits without any time passing.
        val results = mutableListOf<Boolean>()
        val waits = listOf(launch { results += output.wait(0) }, launch { results += state.wait(0) })
        runCurrent(); assertTrue(waits.all { it.isActive })
        cadence.wake(0)
        runCurrent()
        assertEquals(listOf(true, true), results)
        assertEquals(0, currentTime - PollCadence.IDLE - 1)
        // Already answered: the next wait runs its interval, now at the fast rate.
        val fast = launch { assertFalse(output.wait(10)) }
        advanceTimeBy(PollCadence.OUTPUT_ACTIVE + 1); assertTrue(fast.isCompleted)
        // A wake while the loop works is not lost: its next wait returns immediately.
        cadence.wake(20)
        val kept = launch { assertTrue(state.wait(20)) }
        runCurrent(); assertTrue(kept.isCompleted)
    }
}
