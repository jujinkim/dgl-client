package kim.jujin.dglc.core

/** Latest viewport only; the connection owner serializes calls and supplies a monotonic clock. */
class TerminalResizeQueue {
    var size = Size()
        private set
    private var applied: Size? = null
    private var request: Pair<ULong, Size>? = null
    private var due = 0L
    val pending get() = request != null || applied != size

    fun resize(value: Size, nowMillis: Long) {
        if (size == value) return
        size = value
    }

    fun next(nowMillis: Long): Size? =
        size.takeIf { request == null && it != applied && nowMillis >= due }

    fun submitted(id: ULong, value: Size, nowMillis: Long) {
        check(request == null)
        request = id to value
        due = nowMillis + 50
    }

    fun complete(id: ULong, succeeded: Boolean = true, nowMillis: Long) {
        if (request?.first == id) {
            // Submission only reserves a slot. Failed requests may have partly changed
            // the remote grid, so even the previously applied size must be retryable.
            applied = if (succeeded) request?.second else null
            request = null
            if (!succeeded) defer(nowMillis)
        }
    }

    fun defer(nowMillis: Long) { due = maxOf(due, nowMillis + 150) }

    /** Preserve the viewport, but discard the old connection's requests and throttle deadline. */
    fun reset() {
        applied = null
        request = null
        due = 0
    }
}
