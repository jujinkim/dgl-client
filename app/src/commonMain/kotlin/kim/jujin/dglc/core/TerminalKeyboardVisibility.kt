package kim.jujin.dglc.core

/** Native IME events drive requests; identical frames never retry a rejected show. */
internal class TerminalKeyboardVisibility {
    data class State(
        val enabled: Boolean = false,
        val eligible: Boolean = false,
        val visible: Boolean = false,
        val focused: Boolean = false,
        val animating: Boolean = false,
    ) {
        val needsShow get() = enabled && eligible && !animating && (!visible || !focused)
    }

    private var state = State()
    private var version = 0L

    fun update(next: State): Long? {
        if (next == state) return null
        state = next
        version++
        return version.takeIf { next.needsShow }
    }

    fun isCurrent(request: Long) = request == version && state.needsShow
}
