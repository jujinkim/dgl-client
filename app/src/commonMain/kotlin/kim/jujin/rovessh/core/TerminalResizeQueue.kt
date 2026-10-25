package kim.jujin.rovessh.core

/** Latest viewport only; the connection owner serializes calls and supplies a monotonic clock. */
class TerminalResizeQueue {
    var size = Size()
        private set
    private var submitted: Size? = null
    private var request: ULong? = null
    private var due = 0L

    fun resize(value: Size, nowMillis: Long) {
        if (size == value) return
        size = value
        due = nowMillis + 150
    }

    fun next(nowMillis: Long): Size? =
        size.takeIf { request == null && it != submitted && nowMillis >= due }

    fun submitted(id: ULong, value: Size) {
        check(request == null)
        submitted = value
        request = id
    }

    fun complete(id: ULong, succeeded: Boolean = true) {
        if (request == id) {
            request = null
            if (!succeeded) submitted = null
        }
    }

    fun defer(nowMillis: Long) { due = maxOf(due, nowMillis + 150) }

    /** Preserve the viewport, but discard the old connection's requests and debounce deadline. */
    fun reset() {
        submitted = null
        request = null
        due = 0
    }
}
