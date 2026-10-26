package kim.jujin.dglc.core

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlin.math.roundToInt

/** Connection-owned, memory-only font override; renderer recreation must not reset it. */
class TerminalFontSize(default: Int) {
    private var defaultSize = default.coerceIn(10, 28)
    private val override = MutableStateFlow<Int?>(null)
    val overrideSize: StateFlow<Int?> = override
    val effective get() = override.value ?: defaultSize
    private var gestureSize: Double? = null

    fun configure(default: Int) { defaultSize = default.coerceIn(10, 28) }
    fun beginGesture() { gestureSize = effective.toDouble() }
    fun endGesture() { gestureSize = null }
    fun scale(factor: Double): Boolean {
        val previous = gestureSize ?: return false
        if (!factor.isFinite() || factor <= 0) return false
        val next = (previous * factor).coerceIn(10.0, 28.0)
        gestureSize = next
        val size = next.roundToInt()
        if (size == effective) return false
        override.value = size
        return true
    }
    fun reset() { override.value = null; gestureSize = null }
}
