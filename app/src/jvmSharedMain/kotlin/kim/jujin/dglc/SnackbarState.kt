package kim.jujin.dglc

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

sealed class SnackbarDuration(val millis: kotlin.Long?) {
    data object Short : SnackbarDuration(3_000)
    data object Normal : SnackbarDuration(5_000)
    data object Long : SnackbarDuration(10_000)
    data object Indefinite : SnackbarDuration(null)
    /** Zero expires immediately; indefinite display uses the explicit preset. */
    data class Millis(val value: kotlin.Long) : SnackbarDuration(value) {
        init { require(value >= 0) { "Snackbar duration must not be negative" } }
    }
}

data class SnackbarMessage(val id: Long, val message: String, val duration: SnackbarDuration, val startedAtNanos: Long) {
    fun remainingFraction(nowNanos: Long): Float? = duration.millis?.let { ms ->
        if (ms == 0L) 0f else
            (1.0 - (nowNanos - startedAtNanos).coerceAtLeast(0).toDouble() / 1_000_000.0 / ms).coerceIn(0.0, 1.0).toFloat()
    }
}

/** Connection-owned state; UI frames advance only the currently displayed gauge. */
class SnackbarState(private val clock: () -> Long = System::nanoTime) {
    private val mutable = MutableStateFlow<SnackbarMessage?>(null)
    val current = mutable.asStateFlow()
    private var nextId = 0L

    @Synchronized fun show(message: String, duration: SnackbarDuration = SnackbarDuration.Normal) {
        mutable.value = SnackbarMessage(++nextId, message, duration, clock())
    }

    fun show(message: String, durationMs: Long) = show(message, SnackbarDuration.Millis(durationMs))

    @Synchronized fun dismiss(id: Long) {
        if (mutable.value?.id == id) mutable.value = null
    }

    @Synchronized fun remainingFraction(id: Long): Float? =
        mutable.value?.takeIf { it.id == id }?.remainingFraction(clock())

    /** Expiry and progress use one clock. An old UI effect cannot expire its replacement. */
    @Synchronized fun tick(id: Long): Float? {
        val message = mutable.value?.takeIf { it.id == id } ?: return null
        return message.remainingFraction(clock()).also { if (it != null && it <= 0f) mutable.value = null }
    }
}
