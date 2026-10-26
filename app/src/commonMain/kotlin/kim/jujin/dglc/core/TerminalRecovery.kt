package kim.jujin.dglc.core

/** Renderer acknowledgements are scoped to both the recovery cycle and subscription. */
class TerminalRecovery {
    var pending = false; private set
    var cycle = 0L; private set
    private var subscription = 0L
    private var outputReady = false
    private val displayed = mutableMapOf<String, String>()

    fun begin() {
        if (pending) return
        pending = true; cycle++; outputReady = false; displayed.clear()
    }
    fun bind(id: Long) { subscription = id; outputReady = false; displayed.clear() }
    fun unbind() { subscription = 0; outputReady = false; displayed.clear() }
    fun rendered(id: Long, cycle: Long, pane: String? = null, target: String? = null) {
        if (id != subscription || cycle != this.cycle) return
        outputReady = true
        if (pane != null && target != null) displayed[pane] = target
    }
    fun update(checking: Boolean, phase: Phase, connecting: Boolean, targets: Map<String, String>): Boolean {
        if (checking || phase == Phase.Reconnecting || (phase == Phase.Restoring && connecting)) begin()
        if (pending && !checking && outputReady && subscription != 0L &&
            (phase == Phase.Shell || (phase == Phase.Ready && targets.isNotEmpty() && targets.all { displayed[it.key] == it.value }))) {
            pending = false
        }
        return pending
    }
    fun reset() { pending = false; cycle++; unbind() }
}
