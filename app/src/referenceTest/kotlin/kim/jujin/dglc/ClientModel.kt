package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.*
import kim.jujin.dglc.storage.LocalRepository
import kim.jujin.dglc.tmux.ControlClient
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

/** One instance per connection. No cached remote state is persisted. */
class ClientModel(
    override val host: HostProfile,
    private val credentials: CredentialStore,
    private val local: StateFlow<LocalState>,
    private val updateLocal: ((LocalState) -> LocalState) -> Unit,
    private val transportFactory: () -> SshTransport = { SshjTransport() },
) : ClientConnection {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    override val refreshing = MutableStateFlow(false)
    override val status = MutableStateFlow(ConnectionStatus())
    override val remote = MutableStateFlow(RemoteState())
    override val session = MutableStateFlow<String?>(null)
    override val pane = MutableStateFlow<String?>(null)
    override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
    override val notice = MutableStateFlow<String?>(null)
    override val inputNotice = SnackbarState()
    override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
    private val connectionGeneration = AtomicLong()
    private val paneGeneration = AtomicLong()
    private val sizeGeneration = AtomicLong()
    private val operations = Mutex()
    private val inputQueue = Channel<Pair<Generation, ByteArray>>(32)
    @Volatile private var transport: SshTransport? = null
    @Volatile private var control: ControlClient? = null
    @Volatile private var shell: SshTransport.Stream? = null
    @Volatile private var surface: TerminalSurface? = null
    @Volatile private var size = Size()
    @Volatile private var manualClose = false
    private var connectionJob: Job? = null
    private var stateJob: Job? = null
    private var resizeJob: Job? = null
    private var snapshotGeneration: Generation? = null
    private var rememberedSession: String? = host.lastSession
    private var rememberedPane: String? = host.lastPane

    init {
        scope.launch {
            for ((generation, bytes) in inputQueue) {
                if (!status.value.acceptsInput || generation != generation()) continue
                try {
                    operations.withLock {
                        if (status.value.acceptsInput && generation == generation()) {
                            shell?.write(bytes) ?: pane.value?.let { control?.input(it, bytes) }
                        }
                    }
                } catch (e: Exception) { failure(e, generation.connection) }
            }
        }
    }

    private fun generation() = Generation(connectionGeneration.get(), paneGeneration.get(), sizeGeneration.get())
    override fun connect() {
        manualClose = false
        connectionJob?.cancel()
        val epoch = connectionGeneration.incrementAndGet()
        releaseConnection()
        connectionJob = scope.launch { connectLoop(epoch, false) }
    }

    private suspend fun connectLoop(epoch: Long, recovering: Boolean) {
        var attempt = 0
        while (scope.isActive && !manualClose && epoch == connectionGeneration.get()) {
            tmuxProblem.value = null
            status.value = ConnectionStatus(if (recovering || attempt > 0) Phase.Reconnecting else Phase.Connecting, if (attempt == 0) "SSH 연결 중…" else "재접속 중…", attempt)
            val ssh = transportFactory()
            transport = ssh
            try {
                val credential = try { credentials.read(host.credentialRef) } catch (_: Exception) {
                    throw ClientFailure(Failure.Storage, "자격 증명 보호 저장소를 열 수 없습니다.")
                } ?: throw ClientFailure(Failure.Authentication, "호스트를 편집하여 자격 증명을 입력하세요.")
                ssh.connect(host, credential, local.value.trust[host.endpoint])
                if (epoch != connectionGeneration.get()) { ssh.close(); return }
                val version = try { ssh.tmuxVersion() } catch (e: TmuxDiscoveryException) {
                    startShell(ssh, epoch, TmuxProblem(TmuxProblemKind.NotFound, e.diagnostics))
                    return
                } catch (e: RemoteCommandException) {
                    startShell(ssh, epoch, TmuxProblem(TmuxProblemKind.ExecutionFailed, ssh.tmuxDiagnostics, exitStatus = e.exitStatus))
                    return
                }
                if (!supportsTmux(version)) {
                    val detected = version.trim().takeIf { it.matches(Regex("tmux [0-9][a-zA-Z0-9.+-]{0,30}")) }
                    startShell(ssh, epoch, TmuxProblem(if (detected != null) TmuxProblemKind.UnsupportedVersion else TmuxProblemKind.InvalidVersion,
                        ssh.tmuxDiagnostics, detected))
                    return
                }
                val rows = try { ssh.executeTmux(TmuxWire.sessions) } catch (_: RemoteCommandException) { byteArrayOf() }
                val sessions = TmuxRows.sessions(rows.decodeToString().lineSequence().filter { it.isNotEmpty() }.map { it.encodeToByteArray() }.toList())
                remote.value = RemoteState(sessions)
                updateLocal { current -> current.copy(hosts = current.hosts.map { if (it.id == host.id) it.copy(lastConnected = System.currentTimeMillis()) else it }) }
                if (rememberedSession != null) {
                    if (sessions.none { it.id == rememberedSession }) {
                        status.value = ConnectionStatus(Phase.MissingSession, "이전 세션이 삭제되었습니다. 다른 세션을 직접 선택하세요.")
                    } else operations.withLock { attach(rememberedSession!!, epoch) }
                } else status.value = ConnectionStatus(Phase.Selecting, "tmux 세션을 선택하거나 만드세요.")
                return
            } catch (e: Exception) {
                ssh.close()
                if (e is CancellationException) throw e
                if (epoch != connectionGeneration.get()) return
                when (e) {
                    is UnknownHostKey -> { trustRequest.value = e; status.value = ConnectionStatus(Phase.TrustRequired, "서버의 호스트키를 확인하세요."); return }
                    is ChangedHostKey -> { status.value = ConnectionStatus(Phase.Failed, e.message!!); return }
                    is ClientFailure -> if (!RetryPolicy.retryable(e.kind)) { status.value = ConnectionStatus(Phase.Failed, e.safeMessage); return }
                    is ProtocolException -> { status.value = ConnectionStatus(Phase.Failed, "tmux 프로토콜 오류로 연결을 중단했습니다."); return }
                }
                if (attempt >= 7) { status.value = ConnectionStatus(Phase.Failed, "재접속에 실패했습니다. 네트워크를 확인한 뒤 다시 연결하세요."); return }
                status.value = ConnectionStatus(Phase.Reconnecting, "네트워크 연결 대기…", attempt + 1)
                delay(RetryPolicy.delayMillis(attempt++, Random.nextDouble()))
            }
        }
    }

    override fun trust() {
        val request = trustRequest.value ?: return
        updateLocal { it.copy(trust = it.trust + (request.endpoint to request.fingerprint)) }
        trustRequest.value = null
        connect()
    }

    override fun selectSession(id: String) = action {
        rememberedSession = id; rememberedPane = null
        attach(id, connectionGeneration.get())
    }

    private suspend fun attach(id: String, epoch: Long) {
        status.value = ConnectionStatus(Phase.Restoring, "세션 복원 중…")
        paneGeneration.incrementAndGet()
        control?.close(); control = null; stateJob?.cancel()
        val ssh = transport ?: return
        val stream = ssh.openControl(id)
        if (epoch != connectionGeneration.get()) { stream.close(); return }
        session.value = id; pane.value = null
        val controller = ControlClient(stream, scope,
            onOutput = { output ->
                if (epoch == connectionGeneration.get() && pane.value == output.pane && snapshotGeneration == generation()) surface?.write(output.bytes)
            },
            onSnapshot = { target, bytes ->
                if (epoch == connectionGeneration.get() && pane.value == target && snapshotGeneration == generation()) surface?.reset(bytes)
            },
            onScreenSize = { target, remoteSize ->
                if (epoch == connectionGeneration.get() && pane.value == target && snapshotGeneration == generation()) surface?.resizeRemote(remoteSize)
            },
            onPause = { target ->
                // Explicit snapshot pauses are already handled by the current operation.
                if (status.value.phase == Phase.Ready && target == pane.value) restore()
            },
            onFailure = { failure(it, epoch) },
        )
        control = controller
        controller.start()
        remote.value = controller.state.value
        stateJob = scope.launch {
            controller.state.collect { state ->
                if (epoch == connectionGeneration.get()) {
                    remote.value = state
                    if (pane.value != null && state.panes.none { it.id == pane.value }) {
                        pane.value = null
                        status.value = ConnectionStatus(Phase.Selecting, "선택한 패널이 삭제되었습니다. 다른 패널을 선택하세요.")
                    }
                }
            }
        }
        controller.command(TmuxWire.clientSize(size))
        val windowIds = remote.value.windows.filter { it.session == id }.map { it.id }
        val activeWindow = remote.value.windows.firstOrNull { it.session == id && it.active }?.id
        val target = remote.value.panes.firstOrNull { it.id == rememberedPane && it.window in windowIds }
            ?: remote.value.panes.firstOrNull { it.window == activeWindow && it.active }
            ?: remote.value.panes.firstOrNull { it.window in windowIds }
        if (target == null) status.value = ConnectionStatus(Phase.Selecting, "패널을 선택하세요.") else selectPaneInternal(target.id)
    }

    override fun selectTarget(session: String, window: String?, pane: String?) = action {
        if (this.session.value != session) attach(session, connectionGeneration.get())
        val windows = remote.value.windows.filter { it.session == session && (window == null || it.id == window) }
        val target = remote.value.panes.firstOrNull { p -> windows.any { it.id == p.window } && (pane == null && p.active || p.id == pane) }
        target?.let { selectPaneInternal(it.id) }
    }
    override fun selectPane(id: String) = action { selectPaneInternal(id) }
    private suspend fun selectPaneInternal(id: String) {
        val controller = control ?: return
        val target = remote.value.panes.firstOrNull { it.id == id } ?: return
        status.value = ConnectionStatus(Phase.Restoring, "화면 복원 중…")
        paneGeneration.incrementAndGet(); pane.value = id; rememberedPane = id
        controller.command(TmuxWire.selectWindow(target.window))
        controller.command(TmuxWire.selectPane(id))
        snapshotGeneration = generation()
        controller.snapshot(id)
        if (snapshotGeneration == generation()) status.value = ConnectionStatus(Phase.Ready, "연결됨 · tmux")
        saveNavigation()
    }

    private suspend fun startShell(ssh: SshTransport, epoch: Long, problem: TmuxProblem) {
        val channel = ssh.openShell(size); shell = channel
        surface?.reset(byteArrayOf(27, 99))
        tmuxProblem.value = problem
        status.value = ConnectionStatus(Phase.Shell, "일반 SSH · ${problem.label}")
        scope.launch {
            try {
                while (epoch == connectionGeneration.get()) {
                    val bytes = channel.read() ?: throw ClientFailure(Failure.Network, "SSH 연결 종료")
                    surface?.write(bytes)
                }
            } catch (e: Exception) { failure(e, epoch) }
        }
    }

    override fun createSession(name: String) = action {
        val command = TmuxWire.newSession(name)
        if (control != null) { control!!.command(command); control!!.refresh(); remote.value = control!!.state.value }
        else {
            transport?.executeTmux(command)
            val rows = transport?.executeTmux(TmuxWire.sessions) ?: return@action
            remote.value = RemoteState(TmuxRows.sessions(rows.decodeToString().lineSequence().filter { it.isNotEmpty() }.map { it.encodeToByteArray() }.toList()))
        }
    }
    override fun manage(command: TmuxCommand) = action {
        // Dialog IME insets can schedule a resize before its confirmation arrives.
        // The operations mutex serializes both; do not discard the user's command.
        if (status.value.phase !in setOf(Phase.Ready, Phase.Restoring)) return@action
        control?.command(TmuxWire.of(command)); control?.refresh()
    }
    override fun refresh() = action {
        if (control != null) control!!.refresh()
        else transport?.executeTmux(TmuxWire.sessions)?.let { rows ->
            remote.value = RemoteState(TmuxRows.sessions(rows.decodeToString().lineSequence().filter { it.isNotEmpty() }.map { it.encodeToByteArray() }.toList()))
        }
    }

    override fun input(bytes: ByteArray) {
        if (bytes.isEmpty() || !status.value.acceptsInput) return
        if (bytes.size > TerminalInput.MAX_BYTES || inputQueue.trySend(generation() to bytes.copyOf()).isFailure) notice.value = "입력 대기열이 가득 찼습니다. 입력을 전송하지 않았습니다."
    }

    override fun bindSurface(value: TerminalSurface?) {
        surface = value
        if (value != null && pane.value != null && control != null) restore()
    }
    override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) surface = null }

    override fun resize(value: Size) {
        if (size == value) return
        size = value; sizeGeneration.incrementAndGet()
        if (status.value.phase == Phase.Ready) status.value = ConnectionStatus(Phase.Restoring, "화면 크기 변경 중…")
        resizeJob?.cancel()
        resizeJob = scope.launch {
            delay(150)
            action {
                    shell?.resize(size)
                    control?.command(TmuxWire.clientSize(size))
                    restoreInternal()
            }
        }
    }
    override fun restore() = action { restoreInternal() }
    private suspend fun restoreInternal() {
        val target = pane.value ?: return
        val controller = control ?: return
        status.value = ConnectionStatus(Phase.Restoring, "화면 복원 중…")
        snapshotGeneration = generation()
        controller.snapshot(target)
        if (snapshotGeneration == generation()) status.value = ConnectionStatus(Phase.Ready, "연결됨 · tmux")
    }

    private fun action(block: suspend () -> Unit) {
        val epoch = connectionGeneration.get()
        scope.launch {
            operations.withLock {
                if (epoch != connectionGeneration.get() || manualClose) return@withLock
                try { block() } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    when (e) {
                        is IllegalArgumentException -> notice.value = "입력 값이 올바르지 않습니다. 이름과 대상을 확인하세요."
                        is RemoteCommandException -> { notice.value = e.message; if (status.value.phase == Phase.Restoring) status.value = ConnectionStatus(Phase.Selecting, "대상을 다시 선택하세요.") }
                        else -> failure(e, connectionGeneration.get())
                    }
                }
            }
        }
    }

    @Synchronized private fun failure(error: Exception, epoch: Long) {
        if (manualClose || epoch != connectionGeneration.get() || error is CancellationException || status.value.phase == Phase.Reconnecting) return
        val kind = when (error) { is ClientFailure -> error.kind; is ProtocolException -> Failure.Protocol; else -> Failure.Network }
        status.value = ConnectionStatus(if (kind == Failure.MissingSession) Phase.MissingSession else Phase.Failed, "연결이 종료되었습니다.")
        if (!RetryPolicy.retryable(kind)) {
            releaseConnection()
            status.value = ConnectionStatus(if (kind == Failure.MissingSession) Phase.MissingSession else Phase.Failed,
                if (kind == Failure.MissingSession) "세션이 종료되었습니다. 다시 연결해 목록을 확인하세요." else "프로토콜 오류로 연결을 중단했습니다.")
            return
        }
        val next = connectionGeneration.incrementAndGet()
        status.value = ConnectionStatus(Phase.Reconnecting, "연결 복구 중…")
        releaseConnection()
        connectionJob?.cancel()
        connectionJob = scope.launch { connectLoop(next, true) }
    }

    private fun saveNavigation() {
        updateLocal { current -> current.copy(hosts = current.hosts.map { if (it.id == host.id) it.copy(lastSession = session.value, lastPane = pane.value) else it }) }
    }
    private fun releaseConnection() {
        snapshotGeneration = null
        stateJob?.cancel(); stateJob = null
        control?.close(); control = null; shell?.close(); shell = null
        transport?.close(); transport = null
        while (inputQueue.tryReceive().isSuccess) { /* never replay input */ }
    }
    override fun disconnect() {
        manualClose = true
        connectionGeneration.incrementAndGet(); connectionJob?.cancel(); resizeJob?.cancel()
        releaseConnection()
        status.value = ConnectionStatus()
    }
    override fun close() { disconnect(); surface = null; scope.cancel() }
}
