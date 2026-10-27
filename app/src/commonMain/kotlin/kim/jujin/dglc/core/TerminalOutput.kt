package kim.jujin.dglc.core

import kotlinx.coroutines.*

/** A replacement cannot touch a surface until the previous writer has finished cleanup. */
@OptIn(ExperimentalCoroutinesApi::class, DelicateCoroutinesApi::class)
fun launchTerminalOutput(
    scope: CoroutineScope,
    previous: Job?,
    consume: suspend CoroutineScope.() -> Unit,
    failed: (Exception) -> Unit,
    release: () -> Unit,
    cleanup: suspend () -> Unit,
): Job = scope.launch(start = CoroutineStart.ATOMIC) {
    try {
        // Even a replacement cancelled before dispatch must join its predecessor.
        // Native shutdown can then safely join only the last writer in the chain.
        withContext(NonCancellable) { previous?.join() }
        ensureActive()
        consume()
    } catch (error: TimeoutCancellationException) {
        // A renderer's deadline is a failure; owner cancellation is not.
        if (currentCoroutineContext().isActive) failed(error)
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        failed(error)
    } finally {
        // Release backpressure before any main-thread/WebView cleanup can suspend.
        release()
        withContext(NonCancellable) { cleanup() }
    }
}
