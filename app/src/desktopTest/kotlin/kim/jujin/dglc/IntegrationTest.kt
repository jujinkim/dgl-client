package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.*
import kim.jujin.dglc.tmux.ControlClient
import kim.jujin.dglc.storage.MemoryCredentialStore
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import org.junit.Assume.assumeTrue
import java.io.File
import kotlin.test.*

class IntegrationTest {
    @Test fun tmuxDiscoveryUsesLoginPathForCreateListAttachAndReconnect() = runBlocking<Unit> {
        val host = host()
        val admin = connected()
        val store = MemoryCredentialStore().apply { write(host.id, credential()) }
        val local = MutableStateFlow(LocalState(trust = mapOf(host.endpoint to System.getenv("DGL_TEST_FINGERPRINT"))))
        val model = ClientModel(host, store, local, { local.update(it) })
        var session: String? = null
        try {
            // Run the actual candidate loop over fixture-owned paths, including a
            // missing file and a non-executable file, without touching user installs.
            val candidates = listOf(System.getenv("DGL_TEST_ROOT") + "/missing-tmux",
                System.getenv("DGL_TEST_ROOT") + "/tmux.conf", System.getenv("DGL_TEST_TMUX_WRAPPER"))
            val fallback = TmuxDiscovery.find { command ->
                if (command != TmuxDiscovery.common) throw RemoteCommandException(127)
                admin.execute(TmuxDiscovery.commonCommand(candidates))
            }
            assertEquals(System.getenv("DGL_TEST_TMUX_WRAPPER"), fallback.executable)
            assertEquals(listOf(TmuxProbeOutcome.NotFound, TmuxProbeOutcome.NotFound, TmuxProbeOutcome.Found), fallback.attempts.map { it.outcome })
            if (System.getenv("DGL_TEST_TMUX_LOGIN_PATH") == "1") {
                assertEquals(127, assertFailsWith<RemoteCommandException> { admin.execute("tmux -V") }.exitStatus)
            }
            assertTrue(supportsTmux(admin.tmuxVersion()))
            model.connect()
            withTimeout(10000) { model.status.first { it.phase == Phase.Selecting } }
            val name = "login-path-${System.nanoTime()}' ;"
            model.createSession(name)
            val target = withTimeout(10000) { model.remote.first { it.sessions.any { s -> s.name == name } } }.sessions.single { it.name == name }.id
            session = target
            model.refresh()
            model.selectSession(target)
            withTimeout(10000) { model.status.first { it.phase == Phase.Ready } }
            val pane = model.pane.value!!
            val window = model.remote.value.panes.single { it.id == pane }.window
            model.input("printf 'LOGIN_%s\\n' PATH_OK\r".encodeToByteArray())
            withTimeout(5000) { while (!admin.executeTmux(TmuxWire.capture(pane)).decodeToString().contains("LOGIN_PATH_OK")) delay(50) }
            model.manage(TmuxCommand.split(pane, true))
            withTimeout(5000) { model.remote.first { it.panes.count { p -> p.window == window } == 2 } }
            model.disconnect()
            model.connect()
            withTimeout(10000) { model.status.first { it.phase == Phase.Ready } }
            assertEquals(target, model.session.value)
            assertEquals(pane, model.pane.value)
            assertTrue(admin.executeTmux(TmuxWire.capture(pane)).decodeToString().contains("LOGIN_PATH_OK"))
        } finally {
            model.close()
            session?.let { runCatching { admin.executeTmux(TmuxWire.killSession(it)) } }
            admin.close()
        }
    }

    @Test fun macrosUseSelectedPaneAndPlainShellOnce() = runBlocking<Unit> {
        val host = host()
        val admin = connected()
        val store = MemoryCredentialStore().apply { write(host.id, credential()) }
        val local = MutableStateFlow(LocalState(trust = mapOf(host.endpoint to System.getenv("DGL_TEST_FINGERPRINT"))))
        var model: ClientModel? = null
        var session: String? = null
        val macro = TerminalInput.encode(listOf(
            MacroStep.Text("volume_count=\$((\${volume_count:-0}+1)); printf 'MACRO_RUN_%s\\n' \"\$volume_count\""),
            MacroStep.Key(TerminalKey.Enter)))
        try {
            val name = "volume-${System.nanoTime()}"
            admin.execute(TmuxWire.newSession(name).shellWire)
            val target = TmuxRows.sessions(admin.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            session = target
            val tmuxModel = ClientModel(host.copy(lastSession = target), store, local, { local.update(it) })
            model = tmuxModel
            tmuxModel.connect()
            withTimeout(10000) { tmuxModel.status.first { it.phase == Phase.Ready } }
            val original = tmuxModel.pane.value!!
            admin.execute("tmux " + TmuxWire.split(original, true).wire)
            val other = withTimeout(5000) { tmuxModel.remote.first { it.panes.size == 2 } }.panes.single { it.id != original }.id
            tmuxModel.selectPane(other)
            withTimeout(5000) { tmuxModel.pane.first { it == other }; tmuxModel.status.first { it.phase == Phase.Ready } }
            tmuxModel.input(macro)
            suspend fun capture(pane: String) = admin.execute("tmux capture-pane -p -t '$pane'").decodeToString()
            withTimeout(5000) { while (!capture(other).contains("MACRO_RUN_1")) delay(50) }
            delay(200)
            assertFalse(capture(other).contains("MACRO_RUN_2"))
            assertFalse(capture(original).contains("MACRO_RUN_"))
            tmuxModel.close()

            // Real SSH PTY, with only tmux discovery reporting unavailable.
            val output = java.io.ByteArrayOutputStream()
            val writes = java.util.concurrent.atomic.AtomicInteger()
            val shellModel = ClientModel(host, store, local, { local.update(it) }) {
                val transport = SshjTransport()
                object : SshTransport by transport {
                    override suspend fun tmuxVersion() = ""
                    override suspend fun openShell(size: Size): SshTransport.Stream {
                        val stream = transport.openShell(size)
                        return object : SshTransport.Stream() {
                            override suspend fun read() = stream.read()
                            override suspend fun write(bytes: ByteArray) { writes.incrementAndGet(); stream.write(bytes) }
                            override suspend fun resize(size: Size) = stream.resize(size)
                            override fun close() = stream.close()
                        }
                    }
                }
            }
            model = shellModel
            shellModel.bindSurface(object : TerminalSurface {
                override suspend fun write(bytes: ByteArray) { synchronized(output) { output.write(bytes) } }
                override suspend fun reset(bytes: ByteArray) { synchronized(output) { output.reset(); output.write(bytes) } }
                override fun close() {}
            })
            shellModel.connect()
            withTimeout(10000) { shellModel.status.first { it.phase == Phase.Shell } }
            shellModel.input(macro)
            withTimeout(5000) { while (!synchronized(output) { output.toByteArray().decodeToString().contains("MACRO_RUN_1") }) delay(50) }
            delay(200)
            assertEquals(1, writes.get())
            assertFalse(synchronized(output) { output.toByteArray().decodeToString().contains("MACRO_RUN_2") })
        } finally {
            model?.close()
            session?.let { runCatching { admin.execute("tmux " + TmuxWire.killSession(it).wire) } }
            admin.close()
        }
    }
    private fun host(): HostProfile {
        assumeTrue("Run scripts/integration.py to enable real SSH tests", System.getenv("DGL_TEST_PORT") != null)
        return HostProfile("integration", "Isolated test", "127.0.0.1", System.getenv("DGL_TEST_PORT").toInt(), System.getenv("DGL_TEST_USER"), AuthKind.PrivateKey)
    }
    private fun credential(variable: String = "DGL_TEST_KEY_FILE", passphrase: String = "") = Credential(privateKey = File(System.getenv(variable)).readText(), passphrase = passphrase)
    private suspend fun connected() = SshjTransport().also { it.connect(host(), credential(), System.getenv("DGL_TEST_FINGERPRINT")) }

    @Test fun plainSshShellAndHostileNamesAndErrorRecovery() = runBlocking<Unit> {
        val ssh = connected(); val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var client: ControlClient? = null
        try {
            val shell = ssh.openShell(Size())
            shell.resize(Size(90, 30))
            shell.write("printf 'PLAIN_%s\\n' SSH_OK\r".encodeToByteArray())
            var output = ""
            withTimeout(5000) { while (!output.contains("PLAIN_SSH_OK")) output += shell.read()!!.decodeToString() }
            shell.close()
            val name = "bootstrap-${System.nanoTime()};"
            ssh.execute(TmuxWire.newSession(name).shellWire)
            val session = TmuxRows.sessions(ssh.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            client = ControlClient(ssh.openControl(session), scope, {})
            client.start()
            for (value in listOf("#(printf unintended)", "#{session_name}", "quote'| pipe ; dollar\$ value")) {
                client.command(TmuxWire.renameSession(session, value)); client.refresh()
                assertEquals(value, client.state.value.sessions.single { it.id == session }.name)
            }
            assertFailsWith<RemoteCommandException> { client.snapshot("%9999999") }
            client.refresh()
            assertTrue(client.state.value.sessions.any { it.id == session })
            client.close(); client = null
            ssh.execute("tmux " + TmuxWire.killSession(session).wire)
        } finally { client?.close(); ssh.close(); scope.cancel() }
    }

    @Test fun hostTrustMismatchAndKeyAuthentication() = runBlocking<Unit> {
        val host = host()
        val ssh = SshjTransport()
        try {
            val unknown = assertFailsWith<UnknownHostKey> { ssh.connect(host, credential(), null) }
            assertEquals(System.getenv("DGL_TEST_FINGERPRINT"), unknown.fingerprint)
            assertFailsWith<ChangedHostKey> { ssh.connect(host, credential(), "SHA256:synthetic-mismatch") }
            for ((path, pass) in listOf("DGL_TEST_KEY_FILE" to "", "DGL_TEST_RSA_FILE" to "", "DGL_TEST_ENCRYPTED_FILE" to System.getenv("DGL_TEST_PASSPHRASE"))) {
                ssh.connect(host, credential(path, pass), unknown.fingerprint)
                assertTrue(supportsTmux(ssh.execute("tmux -V").decodeToString()))
                ssh.close()
            }
            assertFailsWith<ClientFailure> { ssh.connect(host, credential("DGL_TEST_ENCRYPTED_FILE", "incorrect synthetic passphrase"), unknown.fingerprint) }
        } finally { ssh.close() }
    }

    @Test fun actualSshControlInputExternalChangesAndProcessPreservation() = runBlocking<Unit> {
        val ssh = connected()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val output = Channel<PaneOutput>(128)
        var client: ControlClient? = null
        try {
            val name = "dgl-test-${System.nanoTime()}"
            ssh.execute("tmux " + TmuxWire.newSession(name).wire)
            val sessionBytes = ssh.execute("tmux " + TmuxWire.sessions.wire)
            val sessions = TmuxRows.sessions(sessionBytes.decodeToString().trimEnd().lines().map { it.encodeToByteArray() })
            val session = sessions.single { it.name == name }.id
            client = ControlClient(ssh.openControl(session), scope, { output.send(it) })
            client.start()
            val initial = client.state.value.panes.single { p -> client.state.value.windows.any { it.id == p.window && it.session == session } }.id
            client.command(TmuxWire.clientSize(Size(100, 30)))
            val snapshot = client.snapshot(initial)
            assertTrue(snapshot.isNotEmpty())
            client.input(initial, "printf 'DGL_UNICODE_한글_日本語\\n'\r".encodeToByteArray())
            val seen = java.io.ByteArrayOutputStream()
            withTimeout(5000) { while (!seen.toByteArray().decodeToString().contains("DGL_UNICODE_한글_日本語")) seen.write(output.receive().bytes) }
            val pidBefore = ssh.execute("tmux display-message -p -t '$initial' '#{pane_pid}'").decodeToString().trim()
            ssh.execute("tmux " + TmuxWire.split(initial, true).wire)
            withTimeout(5000) { while (client.state.value.panes.size < 2) delay(20) }
            client.command(TmuxWire.renameSession(session, "safe'; kill-server; #"))
            client.refresh()
            assertTrue(client.state.value.sessions.any { it.id == session && it.name == "safe'; kill-server; #" })
            client.close(); client = null; ssh.close()
            val next = connected()
            try {
                val pidAfter = next.execute("tmux display-message -p -t '$initial' '#{pane_pid}'").decodeToString().trim()
                assertEquals(pidBefore, pidAfter)
                val reattached = ControlClient(next.openControl(session), scope, {})
                reattached.start()
                assertTrue(reattached.snapshot(initial).decodeToString().contains("DGL_UNICODE_한글_日本語"))
                reattached.close()
                assertTrue(next.execute("tmux list-sessions").isNotEmpty())
                next.execute("tmux " + TmuxWire.killSession(session).wire)
            } finally { next.close() }
        } finally { client?.close(); ssh.close(); scope.cancel() }
    }

    @Test fun simultaneousClientsSnapshotDuringOutputAndSessionDeletion() = runBlocking<Unit> {
        val ssh = connected(); val other = connected()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var first: ControlClient? = null; var second: ControlClient? = null
        try {
            val name = "dgl-stream-${System.nanoTime()}"
            ssh.execute("tmux " + TmuxWire.newSession(name).wire)
            val session = TmuxRows.sessions(ssh.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            val firstStream = ssh.openControl(session)
            val failure = CompletableDeferred<Exception>()
            val renderer = HeadlessTerminal()
            first = ControlClient(firstStream, scope, { renderer.feed(it.bytes) }, onSnapshot = { _, bytes -> renderer.feed(bytes) }, onFailure = { failure.complete(it) })
            first.start()
            val pane = first.state.value.panes.first { p -> first.state.value.windows.any { it.session == session && it.id == p.window } }.id
            second = ControlClient(other.openControl(session), scope, {})
            second.start()
            first.input(pane, "i=0; while [ \$i -lt 200 ]; do printf 'SEQ_%04d\\n' \"\$i\"; i=\$((i+1)); sleep 0.002; done\r".encodeToByteArray())
            repeat(10) { assertTrue(first.snapshot(pane).isNotEmpty()); delay(10) }
            withTimeout(5000) { while (!renderer.text().contains("SEQ_0199")) delay(20) }
            val sequence = Regex("(?m)^SEQ_(\\d{4})").findAll(renderer.text()).map { it.groupValues[1].toInt() }.toList()
            assertTrue(sequence.size >= 30)
            assertEquals((sequence.first()..199).toList(), sequence, "Snapshot/live transition must not duplicate or omit output rows")
            val attached = ssh.execute("tmux display-message -p -t '$session' '#{session_attached}'").decodeToString().trim().toInt()
            assertEquals(2, attached)
            other.execute("tmux " + TmuxWire.killSession(session).wire)
            withTimeout(5000) { failure.await() }
        } finally { first?.close(); second?.close(); ssh.close(); other.close(); scope.cancel() }
    }
}
