package kim.jujin.dglc.core

/** Renderer acknowledgements are scoped to both the recovery cycle and subscription. */
class TerminalRecovery {
    var pending = false; private set
    var cycle = 0L; private set
    private var subscription = 0L
    private var wasChecking = false
    private val displayed = mutableMapOf<String, String>()

    fun begin() {
        if (pending) return
        pending = true; cycle++; displayed.clear()
    }
    fun bind(id: Long) { subscription = id; displayed.clear() }
    fun unbind() { subscription = 0; displayed.clear() }
    fun rendered(id: Long, cycle: Long, pane: String? = null, target: String? = null) {
        if (id != subscription || cycle != this.cycle) return
        if (pane != null && target != null) displayed[pane] = target
    }
    fun update(checking: Boolean, phase: Phase, connecting: Boolean, targets: Map<String, String>): Boolean {
        if (phase == Phase.Reconnecting || (phase == Phase.Restoring && connecting)) begin()
        if (!pending && (checking || wasChecking) && subscription != 0L &&
            phase in setOf(Phase.Ready, Phase.Restoring) &&
            targets.any { displayed[it.key] != it.value }) {
            // A healthy check may recapture only changed/paused panes. Continue
            // the current renderer cycle: unchanged panes and an in-flight ACK
            // are still valid, and the changed target must finish before release.
            pending = true
        }
        wasChecking = checking
        if (pending && !checking && subscription != 0L && phase in setOf(Phase.Ready, Phase.Shell) &&
            targets.isNotEmpty() && targets.all { displayed[it.key] == it.value }) {
            pending = false
        }
        return pending
    }
    fun reset() { pending = false; wasChecking = false; cycle++; unbind() }
}
