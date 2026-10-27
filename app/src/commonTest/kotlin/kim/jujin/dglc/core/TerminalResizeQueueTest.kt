package kim.jujin.dglc.core

import kotlin.test.*

class TerminalResizeQueueTest {
    @Test fun resizeAcceptsKeysButBlocksPointersWithoutConnectionProgress() {
        val resizing = ConnectionStatus(phase = Phase.Restoring, resizing = true)
        assertTrue(resizing.acceptsInput)
        assertFalse(resizing.acceptsPointer)
        assertFalse(resizing.copy(checking = true).acceptsInput)
        assertFalse(resizing.copy(recovering = true).acceptsInput)
        assertFalse(resizing.copy(resizing = false).acceptsInput)
        assertFalse(resizing.showsConnectionProgress)
        assertFalse(resizing.copy(resizing = false).showsConnectionProgress)
        assertTrue(resizing.copy(canStopRetrying = true).showsConnectionProgress)
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
        queue.submitted(1u, queue.next(150)!!, 150)
        queue.complete(1u, succeeded = false, nowMillis = 150)
        assertEquals(Size(90, 20), queue.next(300))
    }

    @Test fun continuousAnimationSubmitsImmediatelyThenLatestEvery50Millis() {
        val queue = TerminalResizeQueue()
        queue.resize(Size(80, 40), 0)
        queue.submitted(1u, queue.next(0)!!, 0)
        queue.complete(1u, nowMillis = 1)
        for (time in 1L..49L) {
            queue.resize(Size(80, 40 + time.toInt()), time)
            assertNull(queue.next(time))
        }
        assertEquals(Size(80, 89), queue.next(50))
        queue.submitted(2u, queue.next(50)!!, 50)
        queue.resize(Size(80, 90), 51)
        assertNull(queue.next(1000), "only one request may be in flight")
        queue.complete(2u, nowMillis = 1000)
        assertEquals(Size(80, 90), queue.next(1000))
    }

    @Test fun onlyOneRequestAndOneLatestValueAreRetained() {
        val queue = TerminalResizeQueue()
        queue.submitted(1u, queue.next(0)!!, 0)
        queue.resize(Size(90, 20), 10)
        queue.resize(Size(90, 18), 20)
        assertNull(queue.next(1000))
        queue.complete(9u, nowMillis = 0) // Unrelated completion cannot release the active request.
        assertNull(queue.next(1000))
        queue.complete(1u, nowMillis = 0)
        assertEquals(Size(90, 18), queue.next(1000))
        queue.submitted(2u, queue.next(1000)!!, 1000)
        queue.complete(2u, nowMillis = 0)
        queue.resize(Size(90, 18), 1100)
        assertNull(queue.next(2000))
    }

    @Test fun returningToTheSubmittedSizeNeedsNoRequest() {
        val queue = TerminalResizeQueue()
        val original = queue.size
        queue.submitted(1u, queue.next(0)!!, 0)
        queue.complete(1u, nowMillis = 0)
        queue.resize(Size(90, 20), 10)
        queue.resize(original, 20)
        assertNull(queue.next(1000))
    }

    @Test fun failedIntermediateSizeInvalidatesPreviouslyAppliedGeometry() {
        val queue = TerminalResizeQueue()
        val original = queue.size
        queue.submitted(1u, queue.next(0)!!, 0); queue.complete(1u, nowMillis = 0)
        queue.resize(Size(90, 20), 10)
        queue.submitted(2u, queue.next(160)!!, 160)
        queue.resize(original, 170)
        queue.complete(2u, succeeded = false, nowMillis = 170)
        assertNull(queue.next(319))
        assertEquals(original, queue.next(320), "failure may have partially changed the remote grid")
        queue.submitted(3u, original, 320)
        queue.complete(2u, nowMillis = 0)
        assertNull(queue.next(500))
        queue.complete(3u, nowMillis = 0)
        assertNull(queue.next(500))
    }

    @Test fun newConnectionImmediatelyUsesTheLatestViewport() {
        val queue = TerminalResizeQueue()
        queue.submitted(1u, queue.next(0)!!, 0)
        queue.resize(Size(90, 20), 1000)
        queue.reset()
        assertEquals(Size(90, 20), queue.next(1000))
        queue.submitted(2u, queue.next(1000)!!, 1000)
        queue.complete(1u, nowMillis = 0)
        assertNull(queue.next(2000))
        queue.complete(2u, nowMillis = 0)
        assertNull(queue.next(2000))
    }
}
