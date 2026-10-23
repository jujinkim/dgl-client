package kim.jujin.rovessh.tmux

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.RemoteCommandException
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.ByteArrayOutputStream

class ControlClient(
    private val stream: SshTransport.Stream,
    parentScope: CoroutineScope,
    private val onOutput: suspend (PaneOutput) -> Unit,
    private val onSnapshot: suspend (String, ByteArray) -> Unit = { _, _ -> },
    private val onScreenSize: suspend (String, Size) -> Unit = { _, _ -> },
    private val onPause: (String) -> Unit = {},
    private val onFailure: (Exception) -> Unit = {},
) : TmuxController {
    private val scope = CoroutineScope(parentScope.coroutineContext + SupervisorJob(parentScope.coroutineContext[Job]))
    private val mutableState = MutableStateFlow(RemoteState())
    override val state = mutableState.asStateFlow()
    private val requests = Channel<Request>(1)
    private val startup = CompletableDeferred<Unit>()
    private val mutex = Mutex()
    private val changes = Channel<Unit>(Channel.CONFLATED)
    @Volatile private var closed = false
    private var active: Request? = null

    private class Request(val count: Int, val complete: suspend (List<ControlEvent.Response>) -> Unit) {
        val responses = mutableListOf<ControlEvent.Response>()
        val done = CompletableDeferred<List<ControlEvent.Response>>()
    }

    override suspend fun start() {
        scope.launch(Dispatchers.IO) {
            val parser = ControlParser()
            try {
                while (isActive) {
                    val bytes = stream.read() ?: break
                    for (event in parser.feed(bytes)) when (event) {
                        is ControlEvent.Response -> {
                            if (!startup.isCompleted) {
                                if (event.failed) throw RemoteCommandException()
                                startup.complete(Unit)
                            } else {
                                val request = active ?: requests.tryReceive().getOrNull()?.also { active = it }
                                    ?: throw ProtocolException("Unsolicited command response")
                                request.responses += event
                                if (event.failed || request.responses.size == request.count) {
                                    if (request.responses.any { it.failed }) request.done.completeExceptionally(RemoteCommandException())
                                    else {
                                        request.complete(request.responses)
                                        request.done.complete(request.responses)
                                    }
                                    active = null
                                }
                            }
                        }
                        is ControlEvent.Output -> onOutput(PaneOutput(event.pane, event.bytes))
                        is ControlEvent.Notification -> when (event.name) {
                            "%exit" -> throw ClientFailure(Failure.MissingSession, "tmux 연결이 종료되었습니다. 세션 목록을 확인하세요.")
                            "%pause" -> onPause(event.arguments.substringBefore(' '))
                            "%continue" -> Unit
                            else -> if (event.name in stateEvents) changes.trySend(Unit)
                        }
                    }
                }
                // EOF can split a control record when the network disappears.
                // Discard that connection's partial parser state and recover from
                // a fresh snapshot. Complete malformed records still fail closed
                // inside feed(); a remote %exit is handled separately above.
                if (!closed) throw ClientFailure(Failure.Network, "SSH 연결이 끊어졌습니다.")
            } catch (e: Exception) {
                startup.completeExceptionally(e)
                active?.done?.completeExceptionally(e)
                requests.tryReceive().getOrNull()?.done?.completeExceptionally(e)
                if (!closed && e !is CancellationException) onFailure(e)
            }
        }
        withTimeout(15_000) { startup.await() }
        command(TmuxCommand.flowControl)
        refresh()
        scope.launch {
            for (ignored in changes) {
                delay(80) // Coalesce bursts of layout/window events; no periodic polling.
                while (changes.tryReceive().isSuccess) { /* drain */ }
                try { refresh() } catch (e: Exception) { if (e !is CancellationException && !closed) onFailure(e) }
            }
        }
    }

    private suspend fun batch(commands: List<TmuxCommand>, complete: suspend (List<ControlEvent.Response>) -> Unit = {}): List<ControlEvent.Response> = mutex.withLock {
        check(!closed)
        val request = Request(commands.size, complete)
        requests.send(request)
        try {
            withTimeout(15_000) {
                stream.write((commands.joinToString(" ; ") { it.wire } + "\n").encodeToByteArray())
                request.done.await()
            }
        } catch (e: TimeoutCancellationException) {
            close() // A timed-out request must never desynchronize later responses.
            throw ClientFailure(Failure.Network, "tmux 응답 시간이 초과되었습니다.")
        }
    }

    override suspend fun command(command: TmuxCommand) = batch(listOf(command)).single().lines
    override suspend fun refresh() {
        val replies = batch(listOf(TmuxCommand.sessions, TmuxCommand.windows, TmuxCommand.panes))
        mutableState.value = RemoteState(
            RemoteStateDecoder.sessions(replies[0].lines),
            RemoteStateDecoder.windows(replies[1].lines),
            RemoteStateDecoder.panes(replies[2].lines).distinctBy { it.id },
        )
    }

    override suspend fun snapshot(pane: String): ByteArray {
        var snapshot = byteArrayOf()
        // tmux executes this synchronous command list without yielding its event loop.
        // Pausing in a separate SSH round trip would lose bytes before continue.
        batch(listOf(TmuxCommand.pause(pane), TmuxCommand.capture(pane), TmuxCommand.modes(pane), TmuxCommand.pending(pane), TmuxCommand.captureMain(pane), TmuxCommand.resume(pane))) { replies ->
            val modes = replies[2].lines.single().decodeToString()
            val fields = modes.split('|')
            if (fields.size != 17) throw ProtocolException("Invalid screen metadata")
            val screenSize = try { Size(fields[15].toInt(), fields[16].toInt()) }
                catch (_: IllegalArgumentException) { throw ProtocolException("Unsupported pane dimensions") }
            onScreenSize(pane, screenSize)
            snapshot = restoreScreen(replies[1].lines, modes, replies[3].lines, replies[4].lines)
            // Complete the reset in the reader, before delivering subsequent live output.
            onSnapshot(pane, snapshot)
        }
        return snapshot
    }

    override suspend fun input(pane: String, data: ByteArray) {
        require(data.size <= 65536)
        for (chunk in data.asList().chunked(256)) command(TmuxCommand.send(pane, chunk.toByteArray()))
    }

    override fun close() {
        closed = true
        // Release a suspended terminal consumer before draining channel shutdown.
        scope.cancel()
        active?.done?.cancel()
        requests.tryReceive().getOrNull()?.done?.cancel()
        requests.close(); changes.close()
        stream.close()
    }

    companion object {
        private val stateEvents = setOf("%session-changed", "%sessions-changed", "%session-renamed", "%window-add", "%window-close", "%window-renamed", "%unlinked-window-add", "%unlinked-window-close", "%unlinked-window-renamed", "%layout-change", "%window-pane-changed", "%session-window-changed", "%pane-mode-changed")
    }
}

internal fun restoreScreen(lines: List<ByteArray>, modes: String, pending: List<ByteArray>, main: List<ByteArray> = emptyList()): ByteArray {
    val state = modes.split('|')
    if (state.size != 17) throw ProtocolException("Invalid screen metadata")
    fun n(i: Int) = state[i].toIntOrNull() ?: 0
    val output = ByteArrayOutputStream()
    fun write(s: String) { output.write(s.toByteArray(Charsets.UTF_8)) }
    write("\u001bc")
    fun draw(rows: List<ByteArray>) {
        rows.forEachIndexed { index, bytes ->
            write("\u001b[${index + 1};1H")
            output.write(decodeCapture(bytes))
        }
    }
    if (n(2) == 1) { draw(main); write("\u001b[?1049h") }
    draw(lines)
    write("\u001b[0m\u001b[${n(12) + 1};${n(13) + 1}r\u001b[${n(1) + 1};${n(0) + 1}H")
    fun mode(code: Int, enabled: Boolean) = write("\u001b[?$code${if (enabled) 'h' else 'l'}")
    mode(25, n(3) == 1); mode(1, n(4) == 1)
    write(if (n(5) == 1) "\u001b=" else "\u001b>")
    mode(1003, n(6) == 1); mode(1002, n(7) == 1); mode(1000, n(8) == 1)
    mode(1005, n(9) == 1); mode(1006, n(10) == 1); mode(2004, n(11) == 1)
    write("\u001b[4${if (n(14) == 1) 'h' else 'l'}")
    pending.forEach { output.write(decodeCapture(it)) }
    return output.toByteArray()
}
