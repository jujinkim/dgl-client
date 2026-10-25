package kim.jujin.dgl

import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.serialization.json.*
import uniffi.dgl_core.NativeConnection
import java.security.MessageDigest

/** The ViewModel owns this adapter. Each WebView receives its own native output subscription. */
class RustClientConnection(
    override val host: HostProfile,
    private val credentials: CredentialStore,
    private val local: StateFlow<LocalState>,
    private val updateLocal: ((LocalState) -> LocalState) -> Unit,
) : ClientConnection {
    override val agents = MutableStateFlow(AgentSnapshot())
    override val agentSetup = MutableStateFlow<String?>(null)
    override fun configureAgents(operation: String, token: String?) {
        agentSetup.value = null
        action(buildJsonObject { put("action", "agentSetup"); put("operation", operation); token?.let { put("token", it) } })
    }
    override val refreshing = MutableStateFlow(false)
    @Volatile private var refreshRequest: ULong? = null
    override val status = MutableStateFlow(ConnectionStatus())
    override val remote = MutableStateFlow(RemoteState())
    override val session = MutableStateFlow<String?>(null)
    override val pane = MutableStateFlow<String?>(null)
    override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
    override val notice = MutableStateFlow<String?>(null)
    override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val json = Json { encodeDefaults = true; ignoreUnknownKeys = true }
    @Volatile private var native: NativeConnection? = null
    private var stateJob: Job? = null
    private var outputJob: Job? = null
    private var subscription: ULong? = null
    @Volatile private var surface: TerminalSurface? = null
    @Volatile private var epoch = 0L
    private var lastSession = host.lastSession
    private var lastPane = host.lastPane
    private var size = Size()
    private val inputRequests = java.util.concurrent.ConcurrentHashMap.newKeySet<ULong>()
    @Volatile private var closed = false
    private val cleanup = mutableListOf<Job>()
    private fun trustRef() = "trust-" + MessageDigest.getInstance("SHA-256").digest(host.endpoint.toByteArray()).joinToString("") { "%02x".format(it) }

    @Synchronized override fun connect() {
        if (closed) return
        detach()
        notice.value = null
        inputRequests.clear()
        val current = ++epoch
        status.value = ConnectionStatus(Phase.Connecting, "SSH 연결 중…")
        stateJob = scope.launch {
            try {
                val credential = credentials.read(host.credentialRef) ?: error("Storage")
                val trusted = local.value.trust[host.endpoint]?.takeIf { credentials.read(trustRef())?.password == it }
                val secret = buildJsonObject { put("password", credential.password); put("privateKey", credential.privateKey); put("passphrase", credential.passphrase) }.toString()
                val connection = NativeConnection(json.encodeToString(HostProfile.serializer(), host.copy(lastSession = lastSession, lastPane = lastPane)), secret, trusted)
                synchronized(this@RustClientConnection) {
                    if (current != epoch || closed) { connection.stop(); scope.launch { connection.shutdown(); connection.close() }; return@launch }
                    native = connection
                    connection.submitAction(buildJsonObject { put("action", "resize"); put("columns", size.columns); put("rows", size.rows) }.toString())
                    bindOutput()
                }
                var sequence = -1L
                while (isActive && current == epoch) {
                    val state = json.parseToJsonElement(connection.stateJson()).jsonObject
                    val next = state.getValue("sequence").jsonPrimitive.long
                    if (next != sequence) {
                        sequence = next
                        synchronized(this@RustClientConnection) { if (current == epoch && !closed) applyState(state) }
                    }
                    json.parseToJsonElement(connection.completionsJson()).jsonArray.forEach { result ->
                        val record = result.jsonObject
                        synchronized(this@RustClientConnection) {
                            if (current == epoch && record["request_id"]?.jsonPrimitive?.content?.toULongOrNull() == refreshRequest) {
                                refreshRequest = null; refreshing.value = false
                            }
                        }
                        val input = record["request_id"]?.jsonPrimitive?.content?.toULongOrNull()?.let { inputRequests.remove(it) } == true
                        record["error"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content?.let { code ->
                            if (input || code !in setOf("Cancelled", "StaleGeneration")) notice.value = message(code)
                        }
                    }
                    delay(16)
                }
            } catch (e: CancellationException) { throw e }
            catch (_: Exception) { if (current == epoch) { agents.value = AgentSnapshot(); status.value = ConnectionStatus(Phase.Failed, "보호 저장소 또는 네이티브 연결을 열지 못했습니다."); native?.stop() } }
        }
    }
    private fun applyState(s: JsonObject) {
        fun text(name: String) = s[name]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
        agents.value = s["agents"]?.let { json.decodeFromJsonElement(AgentSnapshot.serializer(), it) } ?: AgentSnapshot()
        agentSetup.value = s["agent_setup"]?.takeUnless { it is JsonNull }?.toString()
        val phase = when (text("phase")) { "Closed" -> Phase.Disconnected; else -> runCatching { Phase.valueOf(text("phase") ?: "Failed") }.getOrDefault(Phase.Failed) }
        val error = text("error")
        val retry = s["retry"]?.jsonObject
        val attempt = retry?.get("attempt")?.jsonPrimitive?.int ?: 0
        val wait = retry?.get("wait_seconds")?.takeUnless { it is JsonNull }?.jsonPrimitive?.int
        val stopped = retry?.get("stopped")?.jsonPrimitive?.boolean == true
        val canStopRetrying = phase in setOf(Phase.Connecting, Phase.Authenticating, Phase.Reconnecting) ||
            (phase == Phase.Restoring && retry?.get("connecting")?.jsonPrimitive?.boolean == true)
        val canRetryNow = canStopRetrying || (phase == Phase.Failed && (error == "Network" || stopped))
        val retryMessage = when {
            stopped -> "재시도 정지 · 접속 정보 유지 중"
            phase == Phase.Failed && error == "Network" -> "자동 재시도 7회 종료 · 접속 정보 유지 중"
            wait != null -> "자동 재시도 대기 · $attempt/7회 완료 · 대기 간격 ${wait}초"
            phase == Phase.Reconnecting -> "연결 재시도 중 · 자동 재시도 $attempt/7회"
            else -> null
        }
        val nextStatus = ConnectionStatus(phase, retryMessage ?: error?.let(::message) ?: when (phase) {
            Phase.Connecting -> "SSH 연결 중…"; Phase.Authenticating -> "서버 확인 중…"; Phase.TrustRequired -> "호스트키 지문을 확인하세요."
            Phase.Selecting -> "tmux 세션을 선택하거나 만드세요."; Phase.Restoring -> "터미널 복원 중…"; Phase.Ready -> "연결됨 · tmux"
            Phase.Shell -> "일반 SSH · tmux 탐지 실패"; Phase.Reconnecting -> "연결 복구 중…"; Phase.MissingSession -> "이전 세션이 삭제되었습니다. 직접 선택하세요."
            else -> "연결 종료"
        }, attempt, canRetryNow, canStopRetrying)
        session.value = text("session"); pane.value = text("pane")?.takeUnless { it == "shell" }
        val r = s.getValue("remote").jsonObject
        fun JsonObject.str(k: String) = getValue(k).jsonPrimitive.content
        remote.value = RemoteStateDecoder.json(r)
        trustRequest.value = if (phase == Phase.TrustRequired) text("fingerprint")?.let { UnknownHostKey(host.endpoint, it) } else null
        tmuxProblem.value = if (phase == Phase.Shell) {
            val probes = s.getValue("diagnostics").jsonArray.map { it.jsonObject }
            val version = probes.firstOrNull { it.str("stage") == "version" }
            val kind = when (version?.str("outcome")) {
                "failed" -> TmuxProblemKind.ExecutionFailed
                "invalid" -> TmuxProblemKind.InvalidVersion
                "unsupported" -> TmuxProblemKind.UnsupportedVersion
                else -> TmuxProblemKind.NotFound
            }
            fun JsonObject.optional(key: String) = get(key)?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
            TmuxProblem(kind, TmuxDiagnostics(probes.filter { it.str("stage") != "version" }.map { probe ->
                TmuxProbeAttempt(when (probe.str("stage")) { "direct" -> "SSH 기본 PATH"; "login" -> "로그인 셸 PATH"; else -> "일반 설치 경로" },
                    when (probe.str("outcome")) { "found" -> TmuxProbeOutcome.Found; "timeout" -> TmuxProbeOutcome.TimedOut; "failed" -> TmuxProbeOutcome.Failed; "invalid" -> TmuxProbeOutcome.InvalidResponse; else -> TmuxProbeOutcome.NotFound },
                    listOfNotNull(probe.optional("path")), probe.optional("exit_status")?.toIntOrNull())
            }, probes.firstNotNullOfOrNull { it.optional("path") }), version?.optional("version"), version?.optional("exit_status")?.toIntOrNull())
        } else null
        // Publish readiness only after its matching target and trust details are visible.
        status.value = nextStatus
        if (phase == Phase.Ready && (lastSession != session.value || lastPane != pane.value)) {
            lastSession = session.value; lastPane = pane.value
            try { updateLocal { old -> old.copy(hosts = old.hosts.map { if (it.id == host.id) it.copy(lastConnected = System.currentTimeMillis(), lastSession = lastSession, lastPane = lastPane) else it }) } }
            catch (_: Exception) { notice.value = "최근 연결 정보를 저장하지 못했습니다." }
        }
    }
    @Synchronized private fun bindOutput() {
        val c = native ?: return
        val terminal = surface ?: return
        outputJob?.cancel(); subscription?.let(c::unsubscribe)
        val id = c.subscribe(); subscription = id
        val current = epoch
        outputJob = scope.launch {
            try {
                while (isActive && current == epoch) {
                    val event = c.pollOutput(id)
                    if (event == null) { delay(8); continue }
                    if (surface === terminal && c.targetJson() == event.targetJson) {
                        when (event.kind) {
                            "Resize" -> terminal.resizeRemote(Size(event.columns.toInt(), event.rows.toInt()))
                            "ResetBegin" -> terminal.reset(event.bytes)
                            "ResetChunk", "Live" -> terminal.write(event.bytes)
                        }
                    }
                    // AndroidTerminal.write/reset return only after xterm's write callback.
                    c.acknowledge(id, event.sequence)
                }
            } catch (e: CancellationException) { throw e }
            catch (_: Exception) { if (current == epoch && surface === terminal) notice.value = "터미널 출력 구독이 종료되었습니다. 재연결하세요." }
        }
        action(buildJsonObject { put("action", "snapshot") })
    }
    @Synchronized override fun bindSurface(value: TerminalSurface?) { surface = value; if (value != null) bindOutput() }
    @Synchronized override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) { surface = null; outputJob?.cancel(); subscription?.let { native?.unsubscribe(it) }; subscription = null } }
    override fun trust() {
        val request = trustRequest.value ?: return
        try {
            credentials.write(trustRef(), Credential(password = request.fingerprint))
            updateLocal { it.copy(trust = it.trust + (request.endpoint to request.fingerprint)) }
            trustRequest.value = null; connect()
        } catch (_: Exception) { notice.value = "호스트키를 보호 저장소에 저장하지 못했습니다. 연결을 중단했습니다."; disconnect() }
    }
    private fun action(value: JsonObject) { try { native?.submitAction(value.toString()) } catch (_: Exception) { notice.value = "요청을 처리할 수 없습니다. 연결 상태와 입력 값을 확인하세요." } }
    override fun retryNow() = action(buildJsonObject { put("action", "retryNow") })
    override fun stopRetrying() = action(buildJsonObject { put("action", "stopRetrying") })
    override fun selectTarget(session: String, window: String?, pane: String?) = action(buildJsonObject {
        put("action", "selectTarget"); put("session", session)
        window?.let { put("window", it) }; pane?.let { put("pane", it) }
    })
    override fun selectSession(id: String) = action(buildJsonObject { put("action", "selectSession"); put("id", id) })
    override fun selectPane(id: String) = action(buildJsonObject { put("action", "selectPane"); put("id", id) })
    override fun createSession(name: String) = manage(TmuxCommand.newSession(name))
    override fun manage(command: TmuxCommand) { val op = command.nativeOperation ?: return; action(buildJsonObject { put("action", "manage"); put("operation", op) }) }
    @Synchronized override fun refresh() {
        if (refreshing.value) return
        val c = native ?: return
        refreshing.value = true
        try { refreshRequest = c.submitAction(buildJsonObject { put("action", "refresh") }.toString()) }
        catch (_: Exception) { refreshing.value = false; notice.value = "세션 목록을 조회하지 못했습니다." }
    }
    override fun restore() = action(buildJsonObject { put("action", "snapshot") })
    override fun resize(value: Size) { if (value == size) return; size = value; action(buildJsonObject { put("action", "resize"); put("columns", value.columns); put("rows", value.rows) }) }
    override fun input(bytes: ByteArray) { val c = native ?: return; if (!status.value.acceptsInput) return; try { c.targetJson()?.let { inputRequests.add(c.submitInput(it, bytes)) } } catch (_: Exception) { notice.value = "입력을 전송하지 않았습니다. 대기열 또는 연결 상태를 확인하세요." } }
    @Synchronized private fun detach() {
        refreshRequest = null; refreshing.value = false
        cleanup.removeAll { it.isCompleted }
        val old = native; native = null; val state = stateJob; stateJob = null; val output = outputJob; outputJob = null
        val id = subscription; subscription = null
        state?.cancel(); output?.cancel(); if (old != null) { id?.let(old::unsubscribe); old.stop(); cleanup += scope.launch(NonCancellable) { state?.join(); output?.join(); try { old.shutdown() } finally { old.close() } } }
    }
    @Synchronized override fun disconnect() { agents.value = AgentSnapshot(); ++epoch; detach(); status.value = ConnectionStatus() }
    @Synchronized override fun close() { if (closed) return; closed = true; disconnect(); surface = null; val pending = cleanup.toList(); scope.launch { pending.forEach { it.join() }; scope.cancel() } }

    private fun message(code: String) = when (code) {
        "HostKeyChanged" -> "저장된 호스트키가 변경되어 인증 전에 연결을 차단했습니다."
        "Authentication" -> "인증 실패: 비밀번호·개인키·키 암호를 확인하세요."
        "HostKeyUnknown" -> "서버의 호스트키 지문을 확인하세요."
        "Storage" -> "보호 저장소에 저장하지 못했습니다."
        "Network" -> "네트워크 연결이 끊어졌습니다."
        "MissingSession" -> "원격 세션이 삭제되었습니다. 다른 세션을 직접 선택하세요."
        "StaleGeneration", "Cancelled" -> "대상 변경으로 입력 또는 요청을 취소했습니다."
        else -> "요청 실패: $code"
    }
}
