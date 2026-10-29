package kim.jujin.dglc.core

import kotlinx.coroutines.flow.StateFlow

// The SSHJ reference engine's contracts. The product uses the Rust core instead.

data class Generation(val connection: Long, val pane: Long, val size: Long)
object RetryPolicy {
    fun retryable(failure: Failure) = failure == Failure.Network
    fun delayMillis(attempt: Int, jitter: Double = 0.5): Long {
        require(attempt >= 0 && jitter in 0.0..1.0)
        return if (attempt == 0) 0L else 1_000L
    }
}
class ClientFailure(val kind: Failure, val safeMessage: String, val diagnosticCode: String? = null) : Exception(safeMessage)
class ChangedHostKey : Exception(t("저장된 호스트키와 다릅니다. 연결을 차단했습니다."))

interface SshTransport {
    val tmuxDiagnostics: TmuxDiagnostics get() = TmuxDiagnostics()
    suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?)
    suspend fun execute(command: String): ByteArray
    suspend fun tmuxVersion(): String = execute("tmux -V").decodeToString()
    suspend fun executeTmux(command: TmuxWire): ByteArray = execute(command.shellWire)
    suspend fun openControl(sessionId: String): Stream
    suspend fun openShell(size: Size): Stream
    fun close()

    abstract class Stream {
        abstract suspend fun read(): ByteArray?
        abstract suspend fun write(bytes: ByteArray)
        abstract suspend fun resize(size: Size)
        abstract fun close()
    }
}

interface TmuxController {
    val state: StateFlow<RemoteState>
    suspend fun start()
    suspend fun refresh()
    suspend fun command(command: TmuxWire): List<ByteArray>
    suspend fun snapshot(pane: String): ByteArray
    suspend fun input(pane: String, data: ByteArray)
    fun close()
}

data class PaneOutput(val pane: String, val bytes: ByteArray)
