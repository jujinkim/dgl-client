package kim.jujin.dglc.core

/**
 * Fresh-screen acknowledgements belong to one subscription and window/connection cycle.
 * [preview] marks a reattachment to the window this renderer presented last: the renderer may
 * keep showing that frozen frame (input stays blocked) instead of an opaque curtain.
 */
class TerminalPresentation {
    var pending = false; private set
    var cycle = 0L; private set
    var preview = false; private set
    /** The last fully presented window key; it survives unbind so a retained renderer can reattach. */
    var lastPresented: String? = null; private set
    private var subscription = 0L
    private var window: String? = null
    private val displayed = mutableMapOf<String, String>()

    fun bind(id: Long) {
        interrupted()
        subscription = id; window = null
        begin()
    }
    private fun begin() { pending = true; cycle++; displayed.clear(); preview = false }
    fun observe(window: String) {
        if (subscription == 0L || this.window == window) return
        // Leaving a window mid-restoration; the first observation after bind is not a switch.
        if (this.window != null) interrupted()
        this.window = window
        begin()
        preview = window == lastPresented
    }
    fun rendered(id: Long, cycle: Long, pane: String, target: String) {
        if (id == subscription && cycle == this.cycle) displayed[pane] = target
    }
    fun ready(id: Long, cycle: Long, phase: Phase, visible: Set<String>, targets: Map<String, String>, blocked: Boolean): Boolean =
        pending && id == subscription && cycle == this.cycle && !blocked &&
            phase in setOf(Phase.Ready, Phase.Shell) && visible.isNotEmpty() &&
            visible.all { targets[it] != null && displayed[it] == targets[it] }

    fun presented(id: Long, cycle: Long) {
        if (id == subscription && cycle == this.cycle) { pending = false; preview = false; lastPresented = window }
    }
    // A presentation cut short may leave a partially reset renderer; never preview that frame.
    private fun interrupted() { if (pending) lastPresented = null }
    fun unbind() {
        interrupted()
        subscription = 0; window = null; pending = false; preview = false; cycle++; displayed.clear()
    }
}
