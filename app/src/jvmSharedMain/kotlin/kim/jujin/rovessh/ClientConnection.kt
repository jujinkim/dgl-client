package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kotlinx.coroutines.flow.MutableStateFlow

interface ClientConnection {
    val host: HostProfile
    val status: MutableStateFlow<ConnectionStatus>
    val remote: MutableStateFlow<RemoteState>
    val session: MutableStateFlow<String?>
    val pane: MutableStateFlow<String?>
    val trustRequest: MutableStateFlow<UnknownHostKey?>
    val notice: MutableStateFlow<String?>
    val tmuxProblem: MutableStateFlow<TmuxProblem?>
    fun connect()
    fun trust()
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
