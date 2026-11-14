package kim.jujin.dglc

import kim.jujin.dglc.core.*

fun nativeInputError(code: String): String = when (code) {
    "StaleGeneration" -> t("input.cancelled_target_changed")
    "NotReady" -> t("input.rejected_recovering")
    "QueueFull" -> t("input.rejected_queue_full")
    "Cancelled", "Closed" -> t("input.rejected_cancelled_or_closed")
    "InvalidArgument" -> t("input.rejected_size")
    else -> t("input.rejected_not_sent")
}

/** One nonblocking notice per recovery cycle, even if the user dismisses it. */
class InputRejections {
    private var recovering = false
    private var shown = false
    fun phase(phase: Phase) = status(ConnectionStatus(phase = phase))
    fun status(status: ConnectionStatus) {
        // A check, reconnect, restore and exhausted failure form one blocked cycle.
        val next = !status.acceptsInput
        if (next != recovering) shown = false
        recovering = next
    }
    fun reject(code: String): String? {
        if (recovering && shown) return null
        shown = true
        return nativeInputError(code)
    }
}
