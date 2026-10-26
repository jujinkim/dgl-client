package kim.jujin.dglc.core

/** Renderer-owned generations survive surface detach/reattach and reject late OS endings. */
class TerminalImeAnimation {
    var version = 0L
        private set
    private var active = false
    fun begin(): Long { active = true; return ++version }
    fun end(token: Long): Boolean {
        if (!active || token != version) return false
        active = false
        return true
    }
    fun cancel(): Long { active = false; return ++version }
}
