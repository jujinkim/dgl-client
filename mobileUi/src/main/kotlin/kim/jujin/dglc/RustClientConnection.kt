package kim.jujin.dglc

import android.os.SystemClock
import android.util.Log
import kim.jujin.dglc.mobile.BuildConfig
import kim.jujin.dglc.ClientConnection
import kim.jujin.dglc.InputTarget
import kim.jujin.dglc.InputCompletion
import kim.jujin.dglc.inputTargetIdentity
import kim.jujin.dglc.InputRejections
import kim.jujin.dglc.nativeInputError
import uniffi.dgl_core.CoreException
import kim.jujin.dglc.core.*
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
    override val inputNotice = SnackbarState()
    private val rejections = InputRejections()
    private fun rejectInput(code: String) { rejections.reject(code)?.let { inputNotice.show(it, if (code == "InvalidArgument") SnackbarDuration.Normal else SnackbarDuration.Short) } }
    private fun inputCode(error: Exception) = (error as? CoreException.Failure)?.code ?: "Unknown"
    override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val json = Json { encodeDefaults = true; ignoreUnknownKeys = true }
    private var titleUpdatesEnabled = false
    @Synchronized override fun setTitleUpdatesEnabled(enabled: Boolean) {
        if (titleUpdatesEnabled == enabled) return
        titleUpdatesEnabled = enabled
        native?.setTitleUpdatesEnabled(enabled)
    }
    @Volatile private var native: NativeConnection? = null
    private var stateJob: Job? = null
    private var outputJob: Job? = null
    private var outputFailed = false
    private var outputRecoveryAttempted = false
    private var tmuxOutput = false
    private var snapshotRequest: ULong? = null
    private var subscription: ULong? = null
    @Volatile private var surface: TerminalSurface? = null
    @Volatile private var epoch = 0L
    @Volatile private var adapterGeneration = 0L
    private var nativeGeneration: Long? = null
    private var appliedSequence = -1L
    override val generation: Long get() = adapterGeneration
    private var lastSession = host.lastSession
    private var lastPane = host.lastPane
    private val resizes = TerminalResizeQueue()
    private val recovery = TerminalRecovery()
    private val backgroundResume = BackgroundResume()
    private var userStopped = false
    private val inputRequests = java.util.concurrent.ConcurrentHashMap.newKeySet<ULong>()
    private val inputCompletions = mutableMapOf<ULong, CompletableDeferred<InputCompletion>>()
    private val resumeRequests = mutableSetOf<ULong>()
    @Volatile private var closed = false
    private val cleanup = mutableListOf<Job>()
    private fun trace(event: String) {
        if (BuildConfig.DEBUG) Log.d("DglResume", "t=${SystemClock.elapsedRealtime()} adapter=${System.identityHashCode(this)} epoch=$epoch generation=$adapterGeneration subscription=$subscription cycle=${recovery.cycle} $event")
    }
    private fun outputFailureMessage() = t("터미널 화면을 복원하지 못했습니다. 새로고침 재시도로 다시 복원하세요.")
    private fun trustRef() = "trust-" + MessageDigest.getInstance("SHA-256").digest(host.endpoint.toByteArray()).joinToString("") { "%02x".format(it) }

    @Synchronized override fun connect() {
        if (closed) return
        val restoring = (native != null && status.value.phase != Phase.TrustRequired) || recovery.pending ||
            (surface != null && session.value != null)
        backgroundResume.cancel(); userStopped = false
        detach()
        if (restoring) recovery.begin()
        outputFailed = false
        outputRecoveryAttempted = false
        notice.value = null
        inputRequests.clear()
        val current = ++epoch
        ++adapterGeneration; nativeGeneration = null
        status.value = ConnectionStatus(Phase.Connecting, t("SSH 연결 중…"), recovering = recovery.pending)
        stateJob = scope.launch {
            try {
                val credential = credentials.read(host.credentialRef) ?: error("Storage")
                val trusted = local.value.trust[host.endpoint]?.takeIf { credentials.read(trustRef())?.password == it }
                val secret = buildJsonObject { put("password", credential.password); put("privateKey", credential.privateKey); put("passphrase", credential.passphrase) }.toString()
                val connection = NativeConnection(json.encodeToString(HostProfile.serializer(), host.copy(lastSession = lastSession, lastPane = lastPane)), secret, trusted)
                synchronized(this@RustClientConnection) {
                    if (current != epoch || closed) { connection.stop(); scope.launch { connection.shutdown(); connection.close() }; return@launch }
                    native = connection
                    connection.setTitleUpdatesEnabled(titleUpdatesEnabled)
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
                                if (id != null) resizes.complete(id, record["error"] == null || record["error"] is JsonNull, System.nanoTime() / 1_000_000)
                                if (id == refreshRequest) { refreshRequest = null; refreshing.value = false }
                                val code = record["error"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
                                val completion = inputCompletions.remove(id)
                                val input = id?.let { inputRequests.remove(it) } == true
                                val resumed = id?.let { resumeRequests.remove(it) } == true
                                if (resumed) trace("check-complete request=$id error=${code ?: "none"}")
                                if (id != null && id == snapshotRequest) {
                                    snapshotRequest = null
                                    if (code != null && status.value.phase in setOf(Phase.Ready, Phase.Shell, Phase.Restoring) && !status.value.showsConnectionProgress) {
                                        failOutput(connection, subscription, current)
                                        outputJob?.cancel()
                                        releaseOutput(connection, subscription, current)
                                    }
                                }
                                if (completion != null) completion.complete(InputCompletion(code?.let(::nativeInputError)))
                                else if (code != null && input) rejectInput(code)
                                else if (code != null && !resumed && code !in setOf("Cancelled", "StaleGeneration")) notice.value = message(code)
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
        val sequence = s.getValue("sequence").jsonPrimitive.long
        // State and output pollers can finish out of order. Equal sequences still
        // need to re-evaluate render acknowledgements, older ones must not restart recovery.
        if (sequence < appliedSequence) return
        appliedSequence = sequence
        s["generation"]?.jsonObject?.get("connection")?.jsonPrimitive?.longOrNull?.let { next ->
            val previous = nativeGeneration
            if (previous != null && previous > 0 && previous != next) {
                ++adapterGeneration
                trace("native-generation previous=$previous next=$next")
            }
            nativeGeneration = next
        }
        fun text(name: String) = s[name]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
        agents.value = s["agents"]?.let { json.decodeFromJsonElement(AgentSnapshot.serializer(), it) } ?: AgentSnapshot()
        agentSetup.value = s["agent_setup"]?.takeUnless { it is JsonNull }?.toString()
        val phase = when (text("phase")) { "Closed" -> Phase.Disconnected; else -> runCatching { Phase.valueOf(text("phase") ?: "Failed") }.getOrDefault(Phase.Failed) }
        val error = text("error")
        val retry = s["retry"]?.jsonObject
        val attempt = retry?.get("attempt")?.jsonPrimitive?.int ?: 0
        val wait = retry?.get("wait_seconds")?.takeUnless { it is JsonNull }?.jsonPrimitive?.int
        val stopped = retry?.get("stopped")?.jsonPrimitive?.boolean == true
        val checking = s["checking"]?.jsonPrimitive?.boolean == true
        val connecting = retry?.get("connecting")?.jsonPrimitive?.boolean == true
        val resizing = s["resizing"]?.jsonPrimitive?.boolean == true
        val targets = if (phase == Phase.Shell) native?.targetJson()?.let { mapOf("shell" to it) } ?: emptyMap()
            else s["render_targets"]?.jsonObject?.mapValues { it.value.toString() } ?: emptyMap()
        val recovering = recovery.update(checking || outputFailed, phase, connecting, targets)
        if (!recovering && !outputFailed) outputRecoveryAttempted = false
        val outputRetry = outputFailed && phase in setOf(Phase.Ready, Phase.Shell, Phase.Restoring) && !connecting
        val canStopRetrying = phase in setOf(Phase.Connecting, Phase.Authenticating, Phase.Reconnecting) ||
            (phase == Phase.Restoring && retry?.get("connecting")?.jsonPrimitive?.boolean == true)
        val canRetryNow = outputRetry || canStopRetrying || (phase == Phase.Failed && (error == "Network" || stopped))
        val retryMessage = when {
            stopped -> t("재시도 정지 · 접속 정보 유지 중")
            phase == Phase.Failed && error == "Network" -> t("자동 재시도 3회 종료 · 접속 정보 유지 중")
            wait != null -> tr("자동 재시도 대기 · {0}/3회 완료 · 대기 간격 {1}초", attempt, wait)
            phase == Phase.Reconnecting -> tr("연결 재시도 중 · 자동 재시도 {0}/3회", attempt)
            else -> null
        }
        val nextStatus = ConnectionStatus(phase, (if (outputRetry) outputFailureMessage() else null) ?: retryMessage ?: error?.let(::message) ?: when (phase) {
            Phase.Connecting -> t("SSH 연결 중…"); Phase.Authenticating -> t("서버 확인 중…"); Phase.TrustRequired -> t("호스트키 지문을 확인하세요.")
            Phase.Selecting -> t("tmux 세션을 선택하거나 만드세요."); Phase.Restoring -> if (resizing) t("연결됨 · tmux") else t("터미널 복원 중…"); Phase.Ready -> t("연결됨 · tmux")
            Phase.Shell -> t("일반 SSH · tmux 탐지 실패"); Phase.Reconnecting -> t("연결 복구 중…"); Phase.MissingSession -> t("이전 세션이 삭제되었습니다. 직접 선택하세요.")
            else -> t("연결 종료")
        }, attempt, canRetryNow, canStopRetrying, resizing, text("window"), checking, recovering, phase == Phase.Failed && error == "Network" && !stopped && attempt >= 3)
        session.value = text("session"); pane.value = text("pane")?.takeUnless { it == "shell" }
        if (phase == Phase.Shell) tmuxOutput = false
        else if (phase in setOf(Phase.Ready, Phase.Restoring) && session.value != null) tmuxOutput = true
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
        rejections.status(nextStatus)
        if (status.value.checking != checking) trace("checking=$checking")
        if (status.value.acceptsInput != nextStatus.acceptsInput) trace("input-enabled=${nextStatus.acceptsInput}")
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
        if (notice.value == outputFailureMessage()) notice.value = null
        val previous = outputJob
        previous?.cancel(); releaseOutput(c, subscription, epoch)
        val id = c.subscribe(); subscription = id
        outputFailed = false
        recovery.bind(id.toLong())
        val current = epoch
        trace("output-bind")
        outputJob = launchTerminalOutput(scope, previous, consume = {
            val displayed = mutableMapOf<String, String>()
            var layoutSequence = ""
            var preserving: Boolean? = null
            terminal.cancelSnapshot()
            while (isActive && current == epoch) {
                val state = json.parseToJsonElement(c.stateJson()).jsonObject
                synchronized(this@RustClientConnection) {
                    if (current == epoch && subscription == id) applyState(state)
                }
                val preserve = synchronized(this@RustClientConnection) { recovery.pending } ||
                    state["phase"]?.jsonPrimitive?.content == "Reconnecting" ||
                    (state["phase"]?.jsonPrimitive?.content == "Restoring" &&
                        state["retry"]?.jsonObject?.get("connecting")?.jsonPrimitive?.boolean == true)
                if (preserving != preserve) {
                    // Capture before applying a new layout or clearing any pane.
                    terminal.connectionRecovery(preserve)
                    preserving = preserve
                }
                val cycle = synchronized(this@RustClientConnection) { recovery.cycle }
                val sequence = state.getValue("sequence").toString()
                if (sequence != layoutSequence) {
                    layoutSequence = sequence
                    fun text(key: String) = state[key]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
                    // Recovery clears the native input target. Keep the last visible
                    // window underneath the dim until a replacement layout exists.
                    if (text("phase") in setOf("Ready", "Restoring", "Shell") &&
                        (text("phase") != "Restoring" || text("window") != null)) {
                        terminal.layout(RemoteStateDecoder.json(state.getValue("remote").jsonObject), text("window"), text("pane"), c.targetJson())
                    }
                    if (text("phase") !in setOf("Ready", "Restoring", "Shell")) {
                        displayed.clear()
                    }
                }
                val event = c.pollOutput(id)
                if (event == null) {
                    synchronized(this@RustClientConnection) {
                        if (current == epoch && subscription == id && state["checking"]?.jsonPrimitive?.boolean != true) {
                            applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
                        }
                    }
                    delay(8); continue
                }
                if (current == epoch && subscription == id && surface === terminal && c.acceptsOutput(event.targetJson)) {
                    val target = json.parseToJsonElement(event.targetJson).jsonObject.getValue("pane_id").jsonPrimitive.content
                    terminal.outputPane(target, event.targetJson)
                    when (event.kind) {
                        "Resize" -> {
                            val shellRecovery = target == "shell" && synchronized(this@RustClientConnection) { recovery.pending }
                            if (shellRecovery || (displayed[target] != null && displayed[target] != event.targetJson)) terminal.beginSnapshot()
                            // A shell resize can supersede its initial Reset events before
                            // the renderer polls them. Render an empty new shell at the
                            // current generation rather than waiting for a discarded ResetEnd.
                            if (shellRecovery && displayed[target] == null) terminal.reset(byteArrayOf(27, 99))
                            terminal.resizeRemote(Size(event.columns.toInt(), event.rows.toInt()))
                            if (shellRecovery) {
                                terminal.endSnapshot(); displayed[target] = event.targetJson
                                synchronized(this@RustClientConnection) {
                                    if (current == epoch && subscription == id && c.acceptsOutput(event.targetJson)) {
                                        recovery.rendered(id.toLong(), cycle, target, event.targetJson)
                                        trace("screen-ack cycle=$cycle")
                                        applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
                                    }
                                }
                            }
                        }
                        "ResetBegin" -> {
                            if (displayed[target] != null || synchronized(this@RustClientConnection) { recovery.pending }) terminal.beginSnapshot()
                            terminal.reset(event.bytes)
                        }
                        "ResetChunk", "Live" -> terminal.write(event.bytes)
                        "ResetEnd" -> {
                            terminal.endSnapshot(); displayed[target] = event.targetJson
                            synchronized(this@RustClientConnection) {
                                if (current == epoch && subscription == id && c.acceptsOutput(event.targetJson)) {
                                    recovery.rendered(id.toLong(), cycle, target, event.targetJson)
                                    trace("screen-ack cycle=$cycle")
                                    applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
                                }
                            }
                        }
                    }
                }
                // Payload stays charged until xterm confirms the pane's write callback.
                ensureActive()
                synchronized(this@RustClientConnection) {
                    if (current == epoch && subscription == id) {
                        c.acknowledge(id, event.sequence)
                        if (state["checking"]?.jsonPrimitive?.boolean != true) applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
                    }
                }
            }
        }, failed = { error ->
            synchronized(this@RustClientConnection) {
                if (surface === terminal) {
                    trace("output-failed kind=${if (error is TimeoutCancellationException) "timeout" else "renderer"}")
                    failOutput(c, id, current)
                }
            }
        }, release = {
            synchronized(this@RustClientConnection) { releaseOutput(c, id, current) }
        }, cleanup = {
            // The successor joins this cleanup before touching the same WebView.
            // Cleanup never waits for an output ACK or delays native unsubscribe.
            runCatching {
                withTimeout(1_000) {
                    val preserve = synchronized(this@RustClientConnection) { surface === terminal && recovery.pending }
                    // Freeze the visible snapshot cache before cancelling it.
                    if (preserve) terminal.connectionRecovery(true)
                    terminal.cancelSnapshot()
                    if (!preserve) terminal.connectionRecovery(false)
                }
            }
        })
        try {
            snapshotRequest = c.submitAction(buildJsonObject { put("action", "snapshot") }.toString())
        } catch (_: Exception) {
            failOutput(c, id, current)
            outputJob?.cancel()
            releaseOutput(c, id, current)
        }
    }
    // All callers hold the adapter lock. Old cleanup must never clear a newer subscription.
    private fun releaseOutput(c: NativeConnection, id: ULong?, current: Long) {
        if (id == null || current != epoch || native !== c || subscription != id) return
        c.unsubscribe(id)
        subscription = null; snapshotRequest = null; recovery.unbind()
        trace("output-release id=$id")
    }
    private fun failOutput(c: NativeConnection, id: ULong?, current: Long) {
        if (id == null || current != epoch || native !== c || subscription != id) return
        recovery.begin()
        outputFailed = true
        applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
        notice.value = outputFailureMessage()
    }
    @Synchronized override fun bindSurface(value: TerminalSurface?) {
        if (surface === value) return
        surface = value
        if (value != null) bindOutput()
        else { outputJob?.cancel(); native?.let { releaseOutput(it, subscription, epoch) } }
    }
    @Synchronized override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) bindSurface(null) }
    override fun trust() {
        val request = trustRequest.value ?: return
        try {
            credentials.write(trustRef(), Credential(password = request.fingerprint))
            updateLocal { it.copy(trust = it.trust + (request.endpoint to request.fingerprint)) }
            trustRequest.value = null; connect()
        } catch (_: Exception) { notice.value = t("호스트키를 보호 저장소에 저장하지 못했습니다. 연결을 중단했습니다."); disconnect() }
    }
    private fun action(value: JsonObject) { try { native?.submitAction(value.toString()) } catch (_: Exception) { notice.value = t("요청을 처리할 수 없습니다. 연결 상태와 입력 값을 확인하세요.") } }
    @Synchronized override fun retryNow() {
        backgroundResume.cancel(); userStopped = false
        val c = native
        if (c != null) applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
        if (outputFailed && status.value.phase in setOf(Phase.Ready, Phase.Restoring, Phase.Shell) && !status.value.showsConnectionProgress) {
            if (status.value.phase == Phase.Shell) connect()
            else {
                outputRecoveryAttempted = false
                onResume()
            }
            return
        }
        action(buildJsonObject { put("action", "retryNow") })
    }
    @Synchronized override fun stopRetrying() {
        backgroundResume.cancel(); userStopped = true
        action(buildJsonObject { put("action", "stopRetrying") })
    }
    @Synchronized override fun suspendForBackground() {
        if (closed || backgroundResume.pending) return
        // Read the current native phase so an unpolled auth/host-key failure cannot
        // be turned into a fresh automatic connection by backgrounding the app.
        val state = native?.let { json.parseToJsonElement(it.stateJson()).jsonObject }
        if (state != null) applyState(state)
        if (status.value.phase in setOf(Phase.Disconnected, Phase.Failed)) return
        val stopped = userStopped || state?.get("retry")?.jsonObject?.get("stopped")?.jsonPrimitive?.boolean == true
        backgroundResume.suspend(status.value.phase, stopped, session.value ?: lastSession, pane.value ?: lastPane)
        agents.value = AgentSnapshot(); ++epoch; ++adapterGeneration
        detach()
        status.value = ConnectionStatus()
    }
    @Synchronized override fun onResume() {
        if (closed) return
        trace("resume")
        backgroundResume.take()?.let {
            lastSession = it.session; lastPane = it.pane
            connect()
            recovery.begin()
            status.value = status.value.copy(recovering = true)
            return
        }
        val c = native ?: return
        if (userStopped) return
        try {
            applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
            val request = c.submitAction(buildJsonObject { put("action", "checkConnection") }.toString())
            resumeRequests.add(request)
            trace("check-submit request=$request")
            // Admission sets checking synchronously in Rust. Publish it in onStart,
            // before Compose draws the first foreground frame, without a timer.
            applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
            if (outputFailed) {
                // Only tmux can reconstruct a lost screen. One automatic attempt,
                // then an explicit retry; auth/trust/stopped states stay untouched.
                if (!outputRecoveryAttempted && surface != null && tmuxOutput &&
                    (status.value.phase in setOf(Phase.Ready, Phase.Restoring, Phase.Reconnecting) ||
                        (status.value.checking && status.value.retryExhausted))) {
                    outputRecoveryAttempted = true
                    bindOutput()
                    applyState(json.parseToJsonElement(c.stateJson()).jsonObject)
                }
            } else if (surface != null && outputJob?.isActive != true) bindOutput()
        } catch (_: Exception) { notice.value = t("요청을 처리할 수 없습니다. 연결 상태와 입력 값을 확인하세요.") }
    }
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
    @Synchronized override fun restore() {
        if (outputFailed) retryNow() else action(buildJsonObject { put("action", "snapshot") })
    }
    @Synchronized override fun resize(value: Size) { if (!closed) resizes.resize(value, System.nanoTime() / 1_000_000) }
    // Called only by the current connection's polling loop while holding the owner lock.
    // No timer can outlive detach(), and native completions bound us to one request in flight.
    private fun submitResize(connection: NativeConnection) {
        if (surface == null) return
        val now = System.nanoTime() / 1_000_000
        val value = resizes.next(now) ?: return
        try {
            val id = connection.submitAction(buildJsonObject {
                put("action", "resize"); put("columns", value.columns); put("rows", value.rows)
            }.toString())
            resizes.submitted(id, value, now)
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
    @Synchronized override fun inputBlocked() { if (!status.value.acceptsInput) rejectInput("NotReady") }
    @Synchronized override fun pointerInput(pane: String, target: String?, bytes: ByteArray) {
        if (!status.value.acceptsPointer || resizes.pending || target == null || native?.targetJson() != target) return
        paneInput(pane, target, bytes)
    }
    @Synchronized override fun focusPane(pane: String, target: String?) {
        if (status.value.acceptsPointer && !resizes.pending && target != null && native?.targetJson() == target) selectPane(pane)
    }
    @Synchronized override fun resizePane(pane: String, target: String?, size: Size) {
        if (!status.value.acceptsPointer || resizes.pending || target == null || native?.targetJson() != target) return
        action(buildJsonObject { put("action", "manage"); put("operation", buildJsonObject {
            put("op", "resizePaneTo"); put("target", pane); put("columns", size.columns); put("rows", size.rows)
        }) })
    }
    override fun encodeKeyboard(key: String, text: String?, modifiers: TerminalModifiers, applicationCursor: Boolean): ByteArray? =
        runCatching { uniffi.dgl_core.encodeTerminalInput(key, text, modifiers.control, modifiers.shift, modifiers.alt, applicationCursor) }.getOrNull()
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
        resumeRequests.clear()
        snapshotRequest = null
        resizes.reset()
        appliedSequence = -1L
        recovery.reset()
        refreshRequest = null; refreshing.value = false
        cleanup.removeAll { it.isCompleted }
        // Keep the last writer as the next bind's predecessor, even across a
        // native reconnect using the same WebView.
        val old = native; native = null; val state = stateJob; stateJob = null; val output = outputJob
        val id = subscription; subscription = null
        state?.cancel(); output?.cancel(); if (old != null) { id?.let(old::unsubscribe); old.stop(); cleanup += scope.launch(NonCancellable) { state?.join(); output?.join(); try { old.shutdown() } finally { old.close() } } }
    }
    @Synchronized override fun disconnect() { backgroundResume.cancel(); agents.value = AgentSnapshot(); ++epoch; ++adapterGeneration; detach(); status.value = ConnectionStatus() }
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
