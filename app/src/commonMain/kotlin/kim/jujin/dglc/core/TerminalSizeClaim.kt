package kim.jujin.dglc.core

/**
 * A tmux window follows the screen focused last. Gaining focus (app activation, or an
 * in-app move to another tab or group) claims the size once for every shown terminal,
 * the focused one last. Claims on different connections run on independent native
 * actors, so that order alone does not decide a window two shown terminals display:
 * a shown terminal on the focused terminal's server window does not claim at all.
 * Hidden tabs never claim. The owner serializes calls; identity, not equality, names
 * a terminal.
 */
class TerminalFocusClaims<T : Any> {
    private var foreground = false
    private var focused: T? = null
    private var pending = false
    // The terminals of the last focus claim, while that focus still holds.
    private var batch: List<T> = emptyList()

    /**
     * The terminals to claim now, in order; empty while nothing new was focused.
     * [sameWindow] tells whether a shown terminal displays the focused one's server window.
     */
    fun update(foreground: Boolean, focused: T?, shown: Collection<T>, sameWindow: (T, T) -> Boolean = { _, _ -> false }): List<T> {
        if (foreground && !this.foreground) pending = true
        if (focused !== this.focused) pending = true
        // Backgrounding or a newer focus ends the previous claim; a hidden terminal leaves it.
        batch = if (!foreground || focused !== this.focused) emptyList() else batch.filter { item -> shown.any { it === item } }
        this.foreground = foreground
        this.focused = focused
        // A new selection is laid out after it is chosen. Claiming the previous layout
        // would let a terminal that is about to be hidden take the size.
        if (!pending || !foreground || shown.isEmpty() || (focused != null && shown.none { it === focused })) return emptyList()
        pending = false
        val order = mutableListOf<T>()
        shown.forEach { item ->
            if (item !== focused && order.none { it === item } && (focused == null || !sameWindow(item, focused))) order += item
        }
        focused?.let { order += it }
        batch = order
        return order
    }

    /**
     * Whether a focus claim of [item] that had to wait for its connection may still go out:
     * the app stayed in the foreground, no newer focus took over and [item] is still shown.
     * [sameWindow] is checked again now: a reconnecting terminal's window may have been
     * unknown when the batch was built, and one now on the focused terminal's server window
     * must not race the focused claim.
     */
    fun stillClaims(item: T, sameWindow: (T, T) -> Boolean = { _, _ -> false }): Boolean {
        if (!foreground || batch.none { it === item }) return false
        val focused = focused ?: return true
        return item === focused || !sameWindow(item, focused)
    }

    companion object {
        /** Window ids are unique per tmux server; one server is one host account. */
        fun sameServerWindow(a: HostProfile, aWindow: String?, b: HostProfile, bWindow: String?): Boolean =
            aWindow != null && aWindow == bWindow && (a.id == b.id ||
                (a.hostname.equals(b.hostname, ignoreCase = true) && a.port == b.port && a.username == b.username))
    }
}

/**
 * Claim requests of one native connection. A focus claim always goes out; user input
 * claims only after another client took the window (`displaced`) and never while a
 * claim is in flight, so ordinary keystrokes add no request between inputs. A claim's
 * completion, success or error, is never shown. A focus claim refused because the
 * connection was not attached yet waits for the next Ready state instead: an automatic
 * reconnect re-attaches without taking the window, so nothing else would claim it.
 */
class TerminalSizeClaims {
    private val inFlight = mutableMapOf<ULong, (() -> Unit)?>()
    private var deferred: (() -> Unit)? = null
    // Pointer reports queued behind a claim in flight; the claim may change their grid.
    private val reports = mutableSetOf<ULong>()
    val pending get() = inFlight.isNotEmpty()
    val waiting get() = deferred != null

    /** [displaced] is read only when an input claim could be sent. */
    fun shouldClaim(force: Boolean, displaced: () -> Boolean): Boolean = force || (inFlight.isEmpty() && displaced())
    /** [retry] re-runs a focus claim that fails for want of an attached client; a sent focus claim replaces a waiting one. */
    fun submitted(id: ULong, retry: (() -> Unit)? = null) {
        inFlight[id] = retry
        if (retry != null) deferred = null
    }
    /** A focus claim refused synchronously with [code] while the connection is in [phase]. */
    fun refused(code: String, phase: Phase, retry: (() -> Unit)?) {
        if (retry == null) return
        deferred = retry.takeIf { code in DEFERRED && waitsForReady(phase) }
    }
    /** True when [id] was a claim: its result must stay silent. */
    fun complete(id: ULong, code: String? = null, phase: Phase = Phase.Ready): Boolean {
        if (id !in inFlight) return false
        val retry = inFlight.remove(id)
        if (code != null) refused(code, phase, retry)
        return true
    }
    /** The waiting focus claim to run, once, now that the connection is in [phase]. */
    fun ready(phase: Phase): (() -> Unit)? = if (phase == Phase.Ready) deferred.also { deferred = null } else null

    /** Whether input [bytes] is a pointer report queued behind a claim in flight. */
    fun behindClaim(bytes: ByteArray): Boolean = pending && isPointerReport(bytes)
    fun reportSubmitted(id: ULong) { reports += id }
    /**
     * True when the failure [code] of input [id] must stay silent: only a pointer report
     * queued behind a claim that changed the grid (`StaleGeneration`) or is still resizing
     * (`NotReady`). `QueueFull`, `Closed` and the rest keep their notice.
     */
    fun completeInput(id: ULong, code: String?): Boolean = reports.remove(id) && reportRace(code)

    /** A new or closed native connection never reports the old one's requests. A waiting focus claim stays. */
    fun reset() { inFlight.clear(); reports.clear() }
    fun close() { reset(); deferred = null }

    companion object {
        private val inputs = setOf("input", "input-binary", "keyboard")
        private val REPORT_RACES = setOf("NotReady", "StaleGeneration")
        // Refused while detached, or lost or cancelled with the transport.
        private val DEFERRED = setOf("NotReady", "Network", "Cancelled")
        /**
         * Bridge messages the renderer marked as the user's own keys, IME text, paste or
         * pointer reports. Emulator replies to DA/DSR/CPR/OSC queries and focus reports
         * arrive unmarked and must never take the window back from another screen.
         */
        fun fromBridge(type: String, user: Boolean): Boolean = user && type in inputs
        /** The only failures of a pointer report queued behind a claim that stay silent. */
        fun reportRace(code: String?): Boolean = code in REPORT_RACES
        /** The core admits input with grid coordinates only for the size it was encoded for. */
        fun isPointerReport(bytes: ByteArray): Boolean =
            bytes.size >= 3 && bytes[0] == 0x1b.toByte() && bytes[1] == '['.code.toByte() && (bytes[2] == 'M'.code.toByte() || bytes[2] == '<'.code.toByte())
        /**
         * Phases a focus claim waits through: connecting, reconnecting or restoring (and a
         * Ready reached between the refusal and this check). Shell, session selection and
         * failures have no window to claim; an explicit session selection takes it anyway.
         */
        fun waitsForReady(phase: Phase): Boolean = phase in setOf(
            Phase.Connecting, Phase.TrustRequired, Phase.Authenticating, Phase.Reconnecting, Phase.Restoring, Phase.Ready)
    }
}
