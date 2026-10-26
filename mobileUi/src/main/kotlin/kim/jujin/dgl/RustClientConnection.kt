package kim.jujin.dgl

import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.InputTarget
import kim.jujin.rovessh.InputCompletion
import kim.jujin.rovessh.inputTargetIdentity
import kim.jujin.rovessh.InputRejections
import kim.jujin.rovessh.nativeInputError
import uniffi.dgl_core.CoreException
import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.serialization.json.*
import uniffi.dgl_core.NativeConnection
import java.security.MessageDigest

/** The process runtime owns this adapter. Each WebView has its own output subscription. */
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
    override val inputNotice = MutableStateFlow<String?>(null)
    private val rejections = InputRejections()
    private fun rejectInput(code: String) { rejections.reject(code)?.let { inputNotice.value = it } }
    private fun inputCode(error: Exception) = (error as? CoreException.Failure)?.code ?: "Unknown"
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
    private val resizes = TerminalResizeQueue()
    private val inputRequests = java.util.concurrent.ConcurrentHashMap.newKeySet<ULong>()
    private val inputCompletions = mutableMapOf<ULong, CompletableDeferred<InputCompletion>>()
    @Volatile private var closed = false
    private val cleanup = mutableListOf<Job>()
    private fun trustRef() = "trust-" + MessageDigest.getInstance("SHA-256").digest(host.endpoint.toByteArray()).joinToString("") { "%02x".format(it) }

    @Synchronized override fun connect() {
        if (closed) return
        detach()
        notice.value = null
        inputRequests.clear()
        val current = ++epoch
        status.value = ConnectionStatus(Phase.Connecting, t("SSH 연결 중…"))
        stateJob = scope.launch {
            try {
                val credential = credentials.read(host.credentialRef) ?: error("Storage")
                val trusted = local.value.trust[host.endpoint]?.takeIf { credentials.read(trustRef())?.password == it }
                val secret = buildJsonObject { put("password", credential.password); put("privateKey", credential.privateKey); put("passphrase", credential.passphrase) }.toString()
                val connection = NativeConnection(json.encodeToString(HostProfile.serializer(), host.copy(lastSession = lastSession, lastPane = lastPane)), secret, trusted)
                synchronized(this@RustClientConnection) {
                    if (current != epoch || closed) { connection.stop(); scope.launch { connection.shutdown(); connection.close() }; return@launch }
                    native = connection
                    resizes.reset()
                    submitResize(connection)
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
                            if (current == epoch) {
                                val id = record["request_id"]?.jsonPrimitive?.content?.toULongOrNull()
                                if (id != null) resizes.complete(id, record["error"] == null || record["error"] is JsonNull)
                                if (id == refreshRequest) { refreshRequest = null; refreshing.value = false }
                                val code = record["error"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
                                val completion = inputCompletions.remove(id)
                                val input = id?.let { inputRequests.remove(it) } == true
                                if (completion != null) completion.complete(InputCompletion(code?.let(::nativeInputError)))
                                else if (code != null && input) rejectInput(code)
                                else if (code != null && code !in setOf("Cancelled", "StaleGeneration")) notice.value = message(code)
                            }
                        }
                    }
                    synchronized(this@RustClientConnection) {
                        if (current == epoch && !closed && status.value.phase in setOf(Phase.Ready, Phase.Shell, Phase.Selecting, Phase.MissingSession)) submitResize(connection)
                    }
                    delay(16)
                }
            } catch (e: CancellationException) { throw e }
            catch (_: Exception) { synchronized(this@RustClientConnection) { if (current == epoch) { failInputs(); agents.value = AgentSnapshot(); status.value = ConnectionStatus(Phase.Failed, t("보호 저장소 또는 네이티브 연결을 열지 못했습니다.")); native?.stop() } } }
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
        val resizing = phase == Phase.Restoring && error == null &&
            retry?.get("connecting")?.jsonPrimitive?.boolean != true
        val canStopRetrying = phase in setOf(Phase.Connecting, Phase.Authenticating, Phase.Reconnecting) ||
            (phase == Phase.Restoring && retry?.get("connecting")?.jsonPrimitive?.boolean == true)
        val canRetryNow = canStopRetrying || (phase == Phase.Failed && (error == "Network" || stopped))
        val retryMessage = when {
            stopped -> t("재시도 정지 · 접속 정보 유지 중")
            phase == Phase.Failed && error == "Network" -> t("자동 재시도 7회 종료 · 접속 정보 유지 중")
            wait != null -> tr("자동 재시도 대기 · {0}/7회 완료 · 대기 간격 {1}초", attempt, wait)
            phase == Phase.Reconnecting -> tr("연결 재시도 중 · 자동 재시도 {0}/7회", attempt)
            else -> null
        }
        val nextStatus = ConnectionStatus(phase, retryMessage ?: error?.let(::message) ?: when (phase) {
            Phase.Connecting -> t("SSH 연결 중…"); Phase.Authenticating -> t("서버 확인 중…"); Phase.TrustRequired -> t("호스트키 지문을 확인하세요.")
            Phase.Selecting -> t("tmux 세션을 선택하거나 만드세요."); Phase.Restoring -> if (resizing) t("연결됨 · tmux") else t("터미널 복원 중…"); Phase.Ready -> t("연결됨 · tmux")
            Phase.Shell -> t("일반 SSH · tmux 탐지 실패"); Phase.Reconnecting -> t("연결 복구 중…"); Phase.MissingSession -> t("이전 세션이 삭제되었습니다. 직접 선택하세요.")
            else -> t("연결 종료")
        }, attempt, canRetryNow, canStopRetrying, resizing, text("window"))
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
                TmuxProbeAttempt(when (probe.str("stage")) { "direct" -> t("SSH 기본 PATH"); "login" -> t("로그인 셸 PATH"); else -> t("일반 설치 경로") },
                    when (probe.str("outcome")) { "found" -> TmuxProbeOutcome.Found; "timeout" -> TmuxProbeOutcome.TimedOut; "failed" -> TmuxProbeOutcome.Failed; "invalid" -> TmuxProbeOutcome.InvalidResponse; else -> TmuxProbeOutcome.NotFound },
                    listOfNotNull(probe.optional("path")), probe.optional("exit_status")?.toIntOrNull())
            }, probes.firstNotNullOfOrNull { it.optional("path") }), version?.optional("version"), version?.optional("exit_status")?.toIntOrNull())
        } else null
        // Publish readiness only after its matching target and trust details are visible.
        rejections.phase(phase)
        status.value = nextStatus
        if (phase == Phase.Ready && (lastSession != session.value || lastPane != pane.value)) {
            lastSession = session.value; lastPane = pane.value
            try { updateLocal { old -> old.copy(hosts = old.hosts.map { if (it.id == host.id) it.copy(lastConnected = System.currentTimeMillis(), lastSession = lastSession, lastPane = lastPane) else it }) } }
            catch (_: Exception) { notice.value = t("최근 연결 정보를 저장하지 못했습니다.") }
        }
    }
    @Synchronized private fun bindOutput() {
        val c = native ?: return
        val terminal = surface ?: return
        outputJob?.cancel(); subscription?.let(c::unsubscribe)
        val id = c.subscribe(); subscription = id
        val current = epoch
        outputJob = scope.launch {
            val displayed = mutableMapOf<String, String>()
            var layoutSequence = ""
            try {
                terminal.cancelSnapshot()
                while (isActive && current == epoch) {
                    val state = json.parseToJsonElement(c.stateJson()).jsonObject
                    val sequence = state.getValue("sequence").toString()
                    if (sequence != layoutSequence) {
                        layoutSequence = sequence
                        fun text(key: String) = state[key]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
                        terminal.layout(RemoteStateDecoder.json(state.getValue("remote").jsonObject), text("window"), text("pane"), c.targetJson())
                        if (text("phase") !in setOf("Ready", "Restoring", "Shell")) {
                            terminal.cancelSnapshot(); displayed.clear()
                        }
                    }
                    val event = c.pollOutput(id)
                    if (event == null) { delay(8); continue }
                    if (surface === terminal && c.acceptsOutput(event.targetJson)) {
                        val target = json.parseToJsonElement(event.targetJson).jsonObject.getValue("pane_id").jsonPrimitive.content
                        terminal.outputPane(target, event.targetJson)
                        when (event.kind) {
                            "Resize" -> {
                                if (displayed[target] != null && displayed[target] != event.targetJson) terminal.beginSnapshot()
                                terminal.resizeRemote(Size(event.columns.toInt(), event.rows.toInt()))
                            }
                            "ResetBegin" -> {
                                if (displayed[target] != null) terminal.beginSnapshot()
                                terminal.reset(event.bytes)
                            }
                            "ResetChunk", "Live" -> terminal.write(event.bytes)
                            "ResetEnd" -> { terminal.endSnapshot(); displayed[target] = event.targetJson }
                        }
                    }
                    // Payload stays charged until xterm confirms the pane's write callback.
                    c.acknowledge(id, event.sequence)
                }
            } catch (e: CancellationException) { throw e }
            catch (_: Exception) { if (current == epoch && surface === terminal) notice.value = t("터미널 출력 구독이 종료되었습니다. 재연결하세요.") }
            finally {
                withContext(NonCancellable) {
                    if (surface !== terminal || (current == epoch && subscription == id)) terminal.cancelSnapshot()
                }
            }
        }
        action(buildJsonObject { put("action", "snapshot") })
    }
    @Synchronized override fun bindSurface(value: TerminalSurface?) {
        if (surface === value) return
        surface = value
        if (value != null) bindOutput()
    }
    @Synchronized override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) { surface = null; outputJob?.cancel(); subscription?.let { native?.unsubscribe(it) }; subscription = null } }
    override fun trust() {
        val request = trustRequest.value ?: return
        try {
            credentials.write(trustRef(), Credential(password = request.fingerprint))
            updateLocal { it.copy(trust = it.trust + (request.endpoint to request.fingerprint)) }
            trustRequest.value = null; connect()
        } catch (_: Exception) { notice.value = t("호스트키를 보호 저장소에 저장하지 못했습니다. 연결을 중단했습니다."); disconnect() }
    }
    private fun action(value: JsonObject) { try { native?.submitAction(value.toString()) } catch (_: Exception) { notice.value = t("요청을 처리할 수 없습니다. 연결 상태와 입력 값을 확인하세요.") } }
    override fun retryNow() = action(buildJsonObject { put("action", "retryNow") })
    override fun stopRetrying() = action(buildJsonObject { put("action", "stopRetrying") })
    override fun selectTarget(session: String, window: String?, pane: String?) {
        if (this.session.value == session && (pane == null || this.pane.value == pane) &&
            (window == null || status.value.window == window)) return
        action(buildJsonObject {
        put("action", "selectTarget"); put("session", session)
        window?.let { put("window", it) }; pane?.let { put("pane", it) }
        })
    }
    override fun selectSession(id: String) = action(buildJsonObject { put("action", "selectSession"); put("id", id) })
    override fun selectPane(id: String) = action(buildJsonObject { put("action", "selectPane"); put("id", id) })
    override fun createSession(name: String) = manage(TmuxCommand.newSession(name))
    override fun manage(command: TmuxCommand) { val op = command.nativeOperation ?: return; action(buildJsonObject { put("action", "manage"); put("operation", op) }) }
    @Synchronized override fun refresh() {
        if (refreshing.value) return
        val c = native ?: return
        refreshing.value = true
        try { refreshRequest = c.submitAction(buildJsonObject { put("action", "refresh") }.toString()) }
        catch (_: Exception) { refreshing.value = false; notice.value = t("세션 목록을 조회하지 못했습니다.") }
    }
    override fun restore() = action(buildJsonObject { put("action", "snapshot") })
    @Synchronized override fun resize(value: Size) { if (!closed) resizes.resize(value, System.nanoTime() / 1_000_000) }
    // Called only by the current connection's polling loop while holding the owner lock.
    // No timer can outlive detach(), and native completions bound us to one request in flight.
    private fun submitResize(connection: NativeConnection) {
        val now = System.nanoTime() / 1_000_000
        val value = resizes.next(now) ?: return
        try {
            val id = connection.submitAction(buildJsonObject {
                put("action", "resize"); put("columns", value.columns); put("rows", value.rows)
            }.toString())
            resizes.submitted(id, value)
        } catch (_: Exception) {
            // A full request queue must not tear down a healthy SSH connection.
            resizes.defer(now)
            notice.value = t("화면 크기 변경을 전송하지 못했습니다. 잠시 후 다시 시도합니다.")
        }
    }
    @Synchronized override fun paneInput(pane: String, target: String?, bytes: ByteArray) {
        val c = native ?: return rejectInput("Closed")
        if (!status.value.acceptsInput) return rejectInput("NotReady")
        val current = c.targetJson() ?: return rejectInput("NotReady")
        if (target == null || inputTargetIdentity(target) != inputTargetIdentity(current) || json.parseToJsonElement(current).jsonObject["pane_id"]?.jsonPrimitive?.content != pane) return rejectInput("StaleGeneration")
        // A size-only change keeps the destination; submit against the latest grid generation.
        try { inputRequests.add(c.submitInput(current, bytes)) } catch (error: Exception) { rejectInput(inputCode(error)) }
    }
    @Synchronized override fun focusPane(pane: String, target: String?) {
        if (target != null && native?.targetJson()?.let { inputTargetIdentity(it) == inputTargetIdentity(target) } == true) selectPane(pane)
    }
    @Synchronized override fun resizePane(pane: String, target: String?, size: Size) {
        if (target == null || native?.targetJson() != target) return
        action(buildJsonObject { put("action", "manage"); put("operation", buildJsonObject {
            put("op", "resizePaneTo"); put("target", pane); put("columns", size.columns); put("rows", size.rows)
        }) })
    }
    @Synchronized override fun input(bytes: ByteArray) {
        val c = native ?: return rejectInput("Closed")
        if (!status.value.acceptsInput) return rejectInput("NotReady")
        try {
            val target = c.targetJson() ?: return rejectInput("NotReady")
            inputRequests.add(c.submitInput(target, bytes))
        } catch (error: Exception) { rejectInput(inputCode(error)) }
    }
    @Synchronized override fun captureInputTarget(): InputTarget? = native?.targetJson()?.let {
        InputTarget(epoch, session.value, inputTargetIdentity(it))
    }
    override suspend fun sendInput(target: InputTarget, bytes: ByteArray): InputCompletion = submitText(target, bytes).await()
    @Synchronized private fun submitText(target: InputTarget, bytes: ByteArray): CompletableDeferred<InputCompletion> {
        val completion = CompletableDeferred<InputCompletion>()
        val c = native
        val raw = c?.targetJson()
        when {
            c == null || raw == null || target != InputTarget(epoch, session.value, inputTargetIdentity(raw)) ->
                completion.complete(InputCompletion(t("연결·세션·패널이 변경되어 입력을 취소했습니다.")))
            !status.value.acceptsInput -> completion.complete(InputCompletion(t("연결이 입력 가능한 상태가 아닙니다. 초안을 보존했습니다.")))
            bytes.isEmpty() || bytes.size > TerminalInput.MAX_BYTES -> completion.complete(InputCompletion(t("입력은 1바이트 이상, 64 KiB 이하여야 합니다.")))
            else -> try {
                // Use the latest size generation, then let the core validate again at dequeue.
                inputCompletions[c.submitInput(raw, bytes)] = completion
            } catch (error: Exception) { completion.complete(InputCompletion(nativeInputError(inputCode(error)))) }
        }
        return completion
    }
    private fun failInputs() {
        inputCompletions.values.forEach { it.complete(InputCompletion(t("연결이 종료되어 전송 결과를 확인하지 못했습니다. 자동 재전송하지 않습니다."))) }
        inputCompletions.clear(); inputRequests.clear()
    }
    @Synchronized private fun detach() {
        failInputs()
        resizes.reset()
        refreshRequest = null; refreshing.value = false
        cleanup.removeAll { it.isCompleted }
        val old = native; native = null; val state = stateJob; stateJob = null; val output = outputJob; outputJob = null
        val id = subscription; subscription = null
        state?.cancel(); output?.cancel(); if (old != null) { id?.let(old::unsubscribe); old.stop(); cleanup += scope.launch(NonCancellable) { state?.join(); output?.join(); try { old.shutdown() } finally { old.close() } } }
    }
    @Synchronized override fun disconnect() { agents.value = AgentSnapshot(); ++epoch; detach(); status.value = ConnectionStatus() }
    @Synchronized override fun close() { if (closed) return; closed = true; disconnect(); surface = null; val pending = cleanup.toList(); scope.launch { pending.forEach { it.join() }; scope.cancel() } }

    private fun message(code: String) = when (code) {
        "HostKeyChanged" -> t("저장된 호스트키가 변경되어 인증 전에 연결을 차단했습니다.")
        "Authentication" -> t("인증 실패: 비밀번호·개인키·키 암호를 확인하세요.")
        "HostKeyUnknown" -> t("서버의 호스트키 지문을 확인하세요.")
        "Storage" -> t("보호 저장소에 저장하지 못했습니다.")
        "Network" -> t("네트워크 연결이 끊어졌습니다.")
        "MissingSession" -> t("원격 세션이 삭제되었습니다. 다른 세션을 직접 선택하세요.")
        "StaleGeneration", "Cancelled" -> t("대상 변경으로 입력 또는 요청을 취소했습니다.")
        else -> tr("요청 실패: {0}", code)
    }
}
