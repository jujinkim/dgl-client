package kim.jujin.dglc.core

/** In-memory, one-shot intent owned by a connection, never persisted with host settings. */
class BackgroundResume {
    data class Target(val session: String?, val pane: String?)
    private var target: Target? = null
    val pending get() = target != null

    fun suspend(phase: Phase, stopped: Boolean, session: String?, pane: String?) {
        if (pending || stopped) return
        if (phase in setOf(Phase.Connecting, Phase.Authenticating, Phase.Selecting, Phase.Restoring,
                Phase.Ready, Phase.Reconnecting, Phase.Shell)) target = Target(session, pane)
    }
    fun take(): Target? = target.also { target = null }
    fun cancel() { target = null }
}
