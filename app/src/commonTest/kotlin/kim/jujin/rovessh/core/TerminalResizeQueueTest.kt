package kim.jujin.rovessh.core

import kotlin.test.*

class TerminalResizeQueueTest {
    @Test fun resizeKeepsInputBlockedWithoutShowingConnectionProgress() {
        val resizing = ConnectionStatus(phase = Phase.Restoring, resizing = true)
        assertFalse(resizing.acceptsInput)
        assertFalse(resizing.showsConnectionProgress)
        assertTrue(resizing.copy(resizing = false).showsConnectionProgress)
        assertTrue(resizing.copy(phase = Phase.Reconnecting).showsConnectionProgress)
        assertTrue(ConnectionStatus(phase = Phase.Ready).acceptsInput)
    }

    @Test fun rejectedSubmissionRetainsTheViewportWithoutBusyRetrying() {
        val queue = TerminalResizeQueue()
        queue.resize(Size(90, 20), 0)
        assertEquals(Size(90, 20), queue.next(150))
        queue.defer(150)
        assertNull(queue.next(299))
        assertEquals(Size(90, 20), queue.next(300))
    }

    @Test fun failedNativeRequestCanBeRetriedAfterConnectionRecovers() {
        val queue = TerminalResizeQueue()
        queue.resize(Size(90, 20), 0)
        queue.submitted(1u, queue.next(150)!!)
        queue.complete(1u, succeeded = false)
        assertEquals(Size(90, 20), queue.next(300))
    }

    @Test fun continuousAnimationWaitsForTheLastChange() {
        val queue = TerminalResizeQueue()
        queue.submitted(1u, queue.next(0)!!)
        queue.complete(1u)
        // A 500 ms animation must not send at 150 ms intervals.
        for (time in 0L..500L step 50) {
            queue.resize(Size(80, 40 - (time / 50).toInt()), time)
            assertNull(queue.next(time + 49))
        }
        assertNull(queue.next(649))
        assertEquals(Size(80, 30), queue.next(650))
    }

    @Test fun onlyOneRequestAndOneLatestValueAreRetained() {
        val queue = TerminalResizeQueue()
        queue.submitted(1u, queue.next(0)!!)
        queue.resize(Size(90, 20), 10)
        queue.resize(Size(90, 18), 20)
        assertNull(queue.next(1000))
        queue.complete(9u) // Unrelated completion cannot release the active request.
        assertNull(queue.next(1000))
        queue.complete(1u)
        assertEquals(Size(90, 18), queue.next(1000))
        queue.submitted(2u, queue.next(1000)!!)
        queue.complete(2u)
        queue.resize(Size(90, 18), 1100)
        assertNull(queue.next(2000))
    }

    @Test fun returningToTheSubmittedSizeNeedsNoRequest() {
        val queue = TerminalResizeQueue()
        val original = queue.size
        queue.submitted(1u, queue.next(0)!!)
        queue.complete(1u)
        queue.resize(Size(90, 20), 10)
        queue.resize(original, 20)
        assertNull(queue.next(1000))
    }

    @Test fun newConnectionImmediatelyUsesTheLatestViewport() {
        val queue = TerminalResizeQueue()
        queue.submitted(1u, queue.next(0)!!)
        queue.resize(Size(90, 20), 1000)
        queue.reset()
        assertEquals(Size(90, 20), queue.next(1000))
        queue.submitted(2u, queue.next(1000)!!)
        queue.complete(1u)
        assertNull(queue.next(2000))
        queue.complete(2u)
        assertNull(queue.next(2000))
    }
}
