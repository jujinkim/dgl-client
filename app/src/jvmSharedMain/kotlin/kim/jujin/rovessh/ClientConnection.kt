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
    fun input(bytes: ByteArray)
    fun bindSurface(value: TerminalSurface?)
    fun unbindSurface(expected: TerminalSurface)
    fun resize(value: Size)
    fun restore()
    fun disconnect()
    fun close()
}
