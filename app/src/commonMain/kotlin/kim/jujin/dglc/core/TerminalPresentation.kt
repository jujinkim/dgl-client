package kim.jujin.dglc.core

/** Fresh-screen acknowledgements belong to one subscription and window/connection cycle. */
class TerminalPresentation {
    var pending = false; private set
    var cycle = 0L; private set
    private var subscription = 0L
    private var window: String? = null
    private val displayed = mutableMapOf<String, String>()

    fun bind(id: Long) {
        subscription = id; window = null
        begin()
    }
    private fun begin() { pending = true; cycle++; displayed.clear() }
    fun observe(window: String) {
        if (subscription == 0L || this.window == window) return
        this.window = window
        begin()
    }
    fun rendered(id: Long, cycle: Long, pane: String, target: String) {
        if (id == subscription && cycle == this.cycle) displayed[pane] = target
    }
    fun ready(id: Long, cycle: Long, phase: Phase, visible: Set<String>, targets: Map<String, String>, blocked: Boolean): Boolean =
        pending && id == subscription && cycle == this.cycle && !blocked &&
            phase in setOf(Phase.Ready, Phase.Shell) && visible.isNotEmpty() &&
            visible.all { targets[it] != null && displayed[it] == targets[it] }

    fun presented(id: Long, cycle: Long) {
        if (id == subscription && cycle == this.cycle) pending = false
    }
    fun unbind() {
        subscription = 0; window = null; pending = false; cycle++; displayed.clear()
    }
}
