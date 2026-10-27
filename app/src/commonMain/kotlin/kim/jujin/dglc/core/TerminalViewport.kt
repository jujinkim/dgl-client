package kim.jujin.dglc.core

data class TerminalViewport(val width: Int, val logicalHeight: Int, val visibleHeight: Int, val imeVisible: Boolean)

/** A renderer owns this generation across composition and WebView detach/reattach. */
class TerminalViewportState {
    var version = 0L
        private set
    var viewport: TerminalViewport? = null
        private set
    var attached = false
        private set
    fun attach() { attached = true; ++version }
    fun detach() { attached = false; ++version }
    fun update(value: TerminalViewport): Boolean {
        if (value.width <= 0 || value.logicalHeight <= 0 || value.visibleHeight <= 0 || value.visibleHeight > value.logicalHeight || value == viewport) return false
        viewport = value
        ++version
        return true
    }
    fun accepts(token: Long) = attached && token == version
}
