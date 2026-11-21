package kim.jujin.dglc.core

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Pacing shared by one connection's polling loops. They poll at their fast rate while
 * anything moves and for [linger] afterwards (echoes, output bursts), then slow down so an
 * idle connection does not keep waking the CPU. [wake] ends every current wait at once, so
 * user input never waits for an idle interval. Times are monotonic milliseconds.
 */
class PollCadence(private val linger: Long = LINGER) {
    @Volatile private var lastActive: Long? = null
    private val wakes = MutableStateFlow(0L)

    /** A loop saw activity: keep the fast rate for [linger] from [now]. */
    fun mark(now: Long) { lastActive = now }

    /** Called from any thread. A wake during a loop's work is kept for its next wait. */
    fun wake(now: Long) {
        mark(now)
        wakes.update { it + 1 }
    }

    fun interval(active: Long, idle: Long, now: Long): Long =
        if (lastActive?.let { now - it < linger } == true) active else idle

    /** One loop's waits. Not shared: it remembers the wakes this loop has already answered. */
    inner class Waiter(private val active: Long, private val idle: Long) {
        private var seen = wakes.value

        /** Waits out the current interval or until a [wake]; true when woken. */
        suspend fun wait(now: Long): Boolean {
            val woken = withTimeoutOrNull(interval(active, idle, now)) { wakes.first { it != seen } } != null
            seen = wakes.value
            return woken
        }
    }

    companion object {
        const val LINGER = 1_500L
        /** Idle wait of both loops: unprompted remote output shows within this delay. */
        const val IDLE = 100L
        const val OUTPUT_ACTIVE = 8L
        const val STATE_ACTIVE = 16L
    }
}
