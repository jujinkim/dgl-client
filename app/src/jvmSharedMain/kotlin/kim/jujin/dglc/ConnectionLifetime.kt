package kim.jujin.dglc

import kim.jujin.dglc.core.Phase

/** App visibility is Activity start/stop, never window focus or an IME/dialog. */
class ConnectionLifetime(
    private val startService: () -> Boolean,
    private val stopService: () -> Unit,
    private val wake: (Boolean) -> Unit,
    private val disconnect: () -> Unit,
    private val dispose: () -> Unit,
) {
    var serviceRequested = false; private set
    private var available = false
    private var closed = false
    private var backgroundDisconnected = false

    fun update(keep: Boolean, visible: Boolean, phases: List<Phase>) {
        if (closed) return
        val active = phases.any { it != Phase.Disconnected && it != Phase.Failed }
        val wanted = keep && active
        if (wanted != serviceRequested) {
            serviceRequested = wanted
            available = wanted && startService()
            if (!wanted) stopService()
        }
        wake(wanted && available && !visible && phases.any { it !in setOf(Phase.Disconnected, Phase.Failed, Phase.TrustRequired) })
        if (visible || keep || !active) backgroundDisconnected = false
        else if (!backgroundDisconnected) { backgroundDisconnected = true; disconnect() }
    }

    /** A rejected start must not reconnect SSH or spin on every status update. */
    fun serviceFailed() { available = false; wake(false) }

    fun taskRemoved() {
        if (closed) return
        closed = true; wake(false)
        if (serviceRequested) stopService()
        serviceRequested = false; available = false
        dispose()
    }
}
