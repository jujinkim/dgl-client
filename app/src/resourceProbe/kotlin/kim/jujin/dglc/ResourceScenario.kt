package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kim.jujin.dglc.storage.MemoryCredentialStore
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.*
import java.io.File

/** Test-only protocol: bounded current state, no terminal transcripts or old connections. */
interface ResourcePlatform {
    suspend fun attach(model: ClientModel)
    suspend fun detach(model: ClientModel)
    suspend fun recreate()
    suspend fun metrics(): Map<String, Long>
    suspend fun slowConsumer(enabled: Boolean)
    suspend fun presentation() { error("presentation_probe_unavailable") }
}

class ResourceScenario(private val config: JsonObject, private val stateFile: File, private val platform: ResourcePlatform) {
    private fun str(key: String) = config.getValue(key).jsonPrimitive.content
    private val soak = str("profile") == "soak"
    private val warmup = if (soak) 300L else 20L
    private val duration = if (soak) 600L else 30L
    private val observe = if (soak) 120L else 10L
    private val repeats = if (soak) 20 else 2
    private val start = System.nanoTime()
    private var phaseStart = start
    @Volatile private var phase = "setup"
    @Volatile private var model: ClientModel? = null
    @Volatile private var currentTransport: SshjTransport? = null
    @Volatile private var gate = CompletableDeferred<Unit>().apply { complete(Unit) }
    private var recoveries = 0
    private var recreations = 0
    private var checks = 0
    private var controls = 0
    private var remotePid = 0L
    private var sequence = 0L
    private var outputBytes = 0L
    private var blockedInputs = 0
    private var slowChecked = false
    private var outputRate = 0.0
    private var finished = false
    private var failure: String? = null
    private val admin = SshjTransport()
    private lateinit var profile: HostProfile
    private lateinit var credential: Credential
    private val local = MutableStateFlow(LocalState())
    private val store = MemoryCredentialStore()
    private lateinit var session: String
    private val publication = Mutex()

    private suspend fun phase(name: String) {
        publication.withLock { phase = name; phaseStart = System.nanoTime() }
        publish() // A collector must not assign new-phase OS metrics to an old phase.
    }
    private suspend fun command(command: String) = withTimeout(20000) { admin.execute(command).decodeToString().trim() }
    private suspend fun ready() = withTimeout(25000) { model!!.status.first { it.phase == Phase.Ready } }
    private suspend fun clients(count: Int) {
        withTimeout(10000) {
            while (true) {
                controls = command("tmux list-clients -t '$session' -F '#{client_control_mode}'").lines().count { it == "1" }
                if (controls == count) break
                delay(100)
            }
        }
    }
    private suspend fun progress() {
        val value = Json.parseToJsonElement(command(str("progressCommand"))).jsonObject
        val pid = value.getValue("pid").jsonPrimitive.long
        if (remotePid == 0L) remotePid = pid
        check(pid == remotePid) { "remote_pid_replaced" }
        check(value.getValue("inputBytes").jsonPrimitive.long == 0L) { "input_replayed" }
        sequence = value.getValue("sequence").jsonPrimitive.long
        outputBytes = value.getValue("bytes").jsonPrimitive.long
        checks++
    }
    private suspend fun open() {
        model = ClientModel(profile, store, local, { transform -> local.update(transform) }) {
            val transport = SshjTransport()
            currentTransport = transport
            object : SshTransport by transport {
                override suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?) {
                    gate.await(); transport.connect(host, credential, trustedFingerprint)
                }
            }
        }
        platform.attach(model!!)
        model!!.connect(); ready(); clients(1)
    }
    private suspend fun close() {
        model?.let { it.close(); platform.detach(it) }
        model = null; currentTransport = null
        clients(0)
    }
    private suspend fun linger(seconds: Long) { delay(seconds * 1000) }
    private suspend fun until(deadline: Long) { delay(((deadline - System.nanoTime()) / 1_000_000).coerceAtLeast(0)) }
    private suspend fun publish() = publication.withLock {
        val metrics = platform.metrics()
        val runtime = Runtime.getRuntime()
        val value = buildJsonObject {
            put("elapsed", (System.nanoTime() - start) / 1e9)
            put("phase", phase); put("phaseElapsed", (System.nanoTime() - phaseStart) / 1e9)
            put("modelPhase", model?.status?.value?.phase?.name ?: "Disconnected")
            put("appPid", str("appPid")); put("heapBytes", runtime.totalMemory() - runtime.freeMemory())
            put("heapCommittedBytes", runtime.totalMemory())
            put("recoveries", recoveries); put("recreations", recreations); put("checks", checks)
            put("controlClients", controls); put("remotePid", remotePid); put("sequence", sequence)
            put("outputBytes", outputBytes); put("blockedInputs", blockedInputs); put("slowConsumerChecked", slowChecked)
            put("outputBytesPerSecond", outputRate)
            put("complete", finished); failure?.let { put("failure", it) }
            metrics.forEach { (key, value) -> put(key, value) }
        }
        val temp = File(stateFile.path + ".tmp")
        temp.writeText(value.toString()); check(temp.renameTo(stateFile))
    }
    suspend fun run() = coroutineScope {
        credential = Credential(privateKey = str("key"))
        profile = HostProfile("resource-probe", "Resource probe", "127.0.0.1", str("port").toInt(), str("user"), AuthKind.PrivateKey)
        admin.connect(profile, credential, str("fingerprint"))
        val sampler = launch(Dispatchers.Default) {
            var next = System.nanoTime()
            while (isActive) { publish(); next += 2_000_000_000; until(next) }
        }
        try {
            command("tmux new-session -d -s resource-bootstrap")
            command("tmux set-option -g history-limit 10000")
            session = command("tmux new-session -d -P -F '#{session_id}' -s resource-probe " + TmuxWire.quote(str("workloadCommand")))
            profile = profile.copy(lastSession = session)
            store.write(profile.id, credential)
            local.value = LocalState(listOf(profile), mapOf(profile.endpoint to str("fingerprint")), Preferences(scrollback = 10000))
            open()
            if (str("profile") == "migration-presentation") {
                // Short hook verification, separate from idle and the canceled soak.
                linger(2); command(str("pauseCommand")); linger(2)
                progress(); val pausedBytes = outputBytes
                sampler.cancelAndJoin() // No ADB/JS metrics polling during presentation tracing.
                phase("presentation")
                platform.presentation()
                progress(); check(outputBytes == pausedBytes) { "presentation_output_changed" }
                close(); finished = true
                return@coroutineScope
            }
            if (str("profile") == "migration-idle") {
                // Separate connected-idle baseline; never claim the soak workload ran.
                command(str("pauseCommand")); phase("stabilize"); linger(30)
                progress(); val idleBytes = outputBytes
                phase("idle"); linger(65); progress()
                check(outputBytes == idleBytes) { "idle_output_changed" }
                close(); phase("observe"); linger(5); finished = true
                return@coroutineScope
            }
            phase("warmup"); linger(warmup)
            progress(); close()
            command(str("pauseCommand")); System.gc(); phase("baseline"); linger(if (soak) 20 else 6)
            phase("connecting"); open(); progress(); val initialBytes = outputBytes
            command(str("resumeCommand")); phase("output")
            val outputEnd = System.nanoTime() + duration * 1_000_000_000
            // Stall the actual consumer briefly; producer/bridge must remain bounded.
            platform.slowConsumer(true); linger(6); platform.slowConsumer(false); slowChecked = true
            until(outputEnd); progress()
            outputRate = (outputBytes - initialBytes).toDouble() / duration
            phase("recovery")
            val recoveryStart = System.nanoTime()
            repeat(repeats) { index ->
                val previous = sequence
                gate = CompletableDeferred()
                currentTransport!!.close()
                withTimeout(10000) { model!!.status.first { it.phase == Phase.Reconnecting } }
                check(!model!!.status.value.acceptsInput)
                model!!.input("MUST_NOT_REPLAY\r".encodeToByteArray()); blockedInputs++
                gate.complete(Unit); ready(); clients(1)
                until(recoveryStart + (index + 1) * duration * 1_000_000_000 / repeats)
                progress(); check(sequence > previous) { "output_stalled" }; recoveries++
            }
            phase("recreate")
            val recreateStart = System.nanoTime()
            repeat(repeats) { index ->
                close(); platform.recreate(); System.gc(); open()
                until(recreateStart + (index + 1) * duration * 1_000_000_000 / repeats)
                progress(); recreations++
            }
            close(); command(str("pauseCommand")); System.gc(); phase("observe"); linger(observe)
            progress(); clients(0); finished = true
        } catch (e: Throwable) {
            // Exception messages may contain paths or server data; persist only the type.
            failure = e.javaClass.simpleName
            throw e
        } finally {
            sampler.cancelAndJoin()
            withContext(NonCancellable) {
                runCatching { close() }
                publish()
                runCatching { command("tmux kill-session -t '$session'") }
                admin.close(); store.delete(profile.id)
            }
        }
    }
}
