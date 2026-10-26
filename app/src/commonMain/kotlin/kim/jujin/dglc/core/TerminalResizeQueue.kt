package kim.jujin.dglc.core

/** Latest viewport only; the connection owner serializes calls and supplies a monotonic clock. */
class TerminalResizeQueue {
    var size = Size()
        private set
    private var applied: Size? = null
    private var request: Pair<ULong, Size>? = null
    private var due = 0L

    fun resize(value: Size, nowMillis: Long) {
        if (size == value) return
        size = value
        due = nowMillis + 150
    }

    fun next(nowMillis: Long): Size? =
        size.takeIf { request == null && it != applied && nowMillis >= due }

    fun submitted(id: ULong, value: Size) {
        check(request == null)
        request = id to value
    }

    fun complete(id: ULong, succeeded: Boolean = true) {
        if (request?.first == id) {
            // Submission only reserves a slot. Failed requests may have partly changed
            // the remote grid, so even the previously applied size must be retryable.
            applied = if (succeeded) request?.second else null
            request = null
        }
    }

    fun defer(nowMillis: Long) { due = maxOf(due, nowMillis + 150) }

    /** Preserve the viewport, but discard the old connection's requests and debounce deadline. */
    fun reset() {
        applied = null
        request = null
        due = 0
    }
}
