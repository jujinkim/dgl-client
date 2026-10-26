package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kotlinx.coroutines.flow.MutableStateFlow

interface ClientConnection {
    val agents: MutableStateFlow<AgentSnapshot> get() = MutableStateFlow(AgentSnapshot())
    val agentSetup: MutableStateFlow<String?> get() = MutableStateFlow(null)
    fun configureAgents(operation: String, token: String? = null) {}
    val refreshing: MutableStateFlow<Boolean>
    val host: HostProfile
    val status: MutableStateFlow<ConnectionStatus>
    val remote: MutableStateFlow<RemoteState>
    val session: MutableStateFlow<String?>
    val pane: MutableStateFlow<String?>
    val trustRequest: MutableStateFlow<UnknownHostKey?>
    val notice: MutableStateFlow<String?>
    val inputNotice: MutableStateFlow<String?> get() = notice
    val tmuxProblem: MutableStateFlow<TmuxProblem?>
    fun connect()
    fun retryNow() = connect()
    fun stopRetrying() = disconnect()
    fun trust()
    fun selectTarget(session: String, window: String? = null, pane: String? = null)
    fun selectSession(id: String)
    fun selectPane(id: String)
    fun createSession(name: String)
    fun manage(command: TmuxCommand)
    fun refresh()
    fun paneInput(pane: String, target: String?, bytes: ByteArray) { input(bytes) }
    fun focusPane(pane: String, target: String?) { selectPane(pane) }
    fun resizePane(pane: String, target: String?, size: Size) {}
    fun input(bytes: ByteArray)
    /** Identity excludes the size generation: opening the IME must not change the destination. */
    fun captureInputTarget(): InputTarget? = null
    /** Completes only when the existing input queue reports its result. Never retries. */
    suspend fun sendInput(target: InputTarget, bytes: ByteArray): InputCompletion =
        InputCompletion(t("이 연결은 텍스트 전송을 지원하지 않습니다."))
    fun bindSurface(value: TerminalSurface?)
    fun unbindSurface(expected: TerminalSurface)
    fun resize(value: Size)
    fun restore()
    fun disconnect()
    fun close()
}

data class InputTarget(val epoch: Long, val session: String?, val identity: String)
data class InputCompletion(val error: String? = null)
