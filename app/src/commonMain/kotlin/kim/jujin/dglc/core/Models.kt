package kim.jujin.dglc.core

import kotlinx.serialization.Serializable

@Serializable
data class HostProfile(
    val id: String, val alias: String, val hostname: String, val port: Int = 22,
    val username: String, val auth: AuthKind = AuthKind.Password,
    val credentialRef: String = id, val favorite: Boolean = false,
    val lastConnected: Long = 0, val lastSession: String? = null, val lastPane: String? = null,
) {
    init {
        require(id.matches(Regex("[a-zA-Z0-9-]{1,80}")))
        require(hostname.isNotBlank() && hostname.length <= 253 && hostname.none { it.isWhitespace() || it.code < 32 })
        require(port in 1..65535)
        require(username.isNotBlank() && username.length <= 128 && username.none { it.code < 32 })
        require(alias.isNotBlank() && alias.length <= 128)
    }
    val endpoint: String get() = "${hostname.lowercase()}:$port"
}

@Serializable enum class AuthKind { Password, PrivateKey }
// Deliberately not a data class: its generated toString must never expose secrets.
class Credential(val password: String = "", val privateKey: String = "", val passphrase: String = "") {
    override fun toString() = "Credential([redacted])"
}

@Serializable data class Preferences(
    val language: String = "system",
    val theme: String = "system", val fontSize: Int = 12, val scrollback: Int = 5000,
    val mobilePaneToolbarVisible: Boolean = true,
    val mobileShortcutBarTwoRows: Boolean = true,
    val androidKeyboardMode: String = "auto",
    val androidKeyboardAlwaysVisible: Boolean = false,
    val terminalShortcuts: List<TerminalShortcut> = TerminalShortcut.defaultShortcuts,
    val volumeKeysEnabled: Boolean = false,
    val agentNotifications: AgentNotificationSettings = AgentNotificationSettings(),
    val volumeUp: VolumeBinding = VolumeBinding(),
    val volumeDown: VolumeBinding = VolumeBinding(),
    val smoothTerminalTransitions: Boolean = false,
    val keepConnectionsInBackground: Boolean = true,
    val showAllHierarchy: Boolean = false,
    val accent: AccentSettings = AccentSettings(),
) {
    val effectiveAndroidKeyboardMode: String get() = if (androidKeyboardAlwaysVisible) "resize"
        else androidKeyboardMode.takeIf { it in listOf("auto", "pan", "resize") } ?: "auto"
}
@Serializable data class LocalState(val hosts: List<HostProfile> = emptyList(), val trust: Map<String, String> = emptyMap(), val preferences: Preferences = Preferences())

data class RemoteSession(val id: String, val name: String, val attached: Int)
data class RemoteWindow(val id: String, val session: String, val name: String, val active: Boolean, val layout: String, val index: Int = 0, val width: Int = 80, val height: Int = 24, val zoomed: Boolean = false, val visibleLayout: String = "")
data class RemotePane(val id: String, val window: String, val width: Int, val height: Int, val active: Boolean, val mode: String, val command: String, val title: String = "", val left: Int = 0, val top: Int = 0)
data class RemoteState(val sessions: List<RemoteSession> = emptyList(), val windows: List<RemoteWindow> = emptyList(), val panes: List<RemotePane> = emptyList())
data class Size(val columns: Int = 80, val rows: Int = 24) {
    init { require(columns in 1..65535 && rows in 1..65535) }
}
enum class Phase { Disconnected, Connecting, TrustRequired, Authenticating, Selecting, Restoring, Ready, Reconnecting, MissingSession, Failed, Shell }
enum class Failure { Network, Authentication, HostKey, Protocol, MissingSession, Storage }
data class ConnectionStatus(
    val phase: Phase = Phase.Disconnected,
    val message: String = t("연결 안 됨"),
    val attempt: Int = 0,
    val canRetryNow: Boolean = false,
    val canStopRetrying: Boolean = false,
    val resizing: Boolean = false,
    val window: String? = null,
    val checking: Boolean = false,
    val recovering: Boolean = false,
    val retryExhausted: Boolean = false,
    val presenting: Boolean = false,
) {
    val acceptsInput get() = !checking && !recovering && !presenting && (phase == Phase.Ready || phase == Phase.Shell || (phase == Phase.Restoring && resizing && !canStopRetrying))
    val acceptsPointer get() = acceptsInput && !resizing
    val recoveryMessage get() = when {
        checking && !showsConnectionProgress -> t("연결 확인 중…")
        recovering -> if (phase in setOf(Phase.Failed, Phase.MissingSession, Phase.TrustRequired) ||
            (canRetryNow && !showsConnectionProgress)) message else t("연결 복구 중…")
        checking -> t("연결 확인 중…")
        else -> null
    }
    val showsConnectionProgress get() = phase == Phase.Connecting || phase == Phase.Authenticating ||
        phase == Phase.Reconnecting || (phase == Phase.Restoring && canStopRetrying)
}
class UnknownHostKey(val endpoint: String, val fingerprint: String) : Exception(t("호스트키 신뢰 확인 필요"))

interface CredentialStore {
    fun read(reference: String): Credential?
    fun write(reference: String, credential: Credential)
    fun delete(reference: String)
}

/** write completes only when bytes have been accepted by a bounded renderer queue. */
interface TerminalSurface {
    /** Hide a switching window until all of its current panes have painted. */
    suspend fun presentationPending(active: Boolean) {}
    /** Keep the last rendered window visible while a connection restores its panes. */
    suspend fun connectionRecovery(active: Boolean) {}
    suspend fun layout(remote: RemoteState, window: String?, pane: String?, inputTarget: String?) {}
    suspend fun outputPane(pane: String, target: String) {}
    suspend fun write(bytes: ByteArray)
    suspend fun reset(bytes: ByteArray)
    suspend fun resizeRemote(size: Size) {}
    suspend fun beginSnapshot() {}
    suspend fun endSnapshot() {}
    suspend fun cancelSnapshot() {}
    fun close()
}
