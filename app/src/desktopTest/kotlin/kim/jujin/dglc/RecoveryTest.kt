package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kim.jujin.dglc.storage.MemoryCredentialStore
import kim.jujin.dglc.tmux.ControlClient
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import org.junit.Assume.assumeTrue
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.*

class RecoveryTest {
    @Test fun repeatedDisconnectBlocksInputPreservesPidAndDoesNotReplaceMissingSession() = runBlocking<Unit> {
        assumeTrue(System.getenv("DGL_TEST_PORT") != null)
        val host = HostProfile("recovery", "Recovery", "127.0.0.1", System.getenv("DGL_TEST_PORT").toInt(), System.getenv("DGL_TEST_USER"), AuthKind.PrivateKey)
        val credential = Credential(privateKey = File(System.getenv("DGL_TEST_KEY_FILE")).readText())
        val fingerprint = System.getenv("DGL_TEST_FINGERPRINT")
        // Closing a connection must not drain queued output into a slow renderer
        // before reporting EOF. That postpones reconnect while input still looks ready.
        val buffered = SshjTransport()
        try {
            buffered.connect(host, credential, fingerprint)
            val stream = buffered.openShell(Size())
            stream.write("i=0; while [ \"\$i\" -lt 256 ]; do printf '%01024d\\n' 0; i=\$((i+1)); done\r".encodeToByteArray())
            assertNotNull(withTimeout(5000) { stream.read() })
            delay(300) // Let the peer fill SSHJ's bounded receive window.
            buffered.close()
            assertNull(withTimeout(2000) { stream.read() }, "Closed transport must discard buffered output")
        } finally { buffered.close() }
        // A socket can close halfway through a control line. The next connection
        // restores a snapshot; an unfinished old line must not disable recovery.
        var pending = true
        val truncated = ControlClient(object : SshTransport.Stream() {
            override suspend fun read(): ByteArray? = if (pending) {
                pending = false; "%output %0 unfinished".encodeToByteArray()
            } else null
            override suspend fun write(bytes: ByteArray) {}
            override suspend fun resize(size: Size) {}
            override fun close() {}
        }, this, onOutput = {})
        try {
            val error = assertFailsWith<ClientFailure> { truncated.start() }
            assertEquals(Failure.Network, error.kind)
        } finally { truncated.close() }
        val admin = SshjTransport()
        admin.connect(host, credential, fingerprint)
        val name = "recovery-${System.nanoTime()}"
        admin.execute("tmux " + TmuxCommand.newSession(name).wire)
        val target = RemoteStateDecoder.sessions(admin.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
        val profile = host.copy(lastSession = target)
        val store = MemoryCredentialStore().apply { write(host.id, credential) }
        val local = MutableStateFlow(LocalState(listOf(profile), mapOf(host.endpoint to fingerprint)))
        val transports = CopyOnWriteArrayList<SshjTransport>()
        var gate = CompletableDeferred<Unit>().apply { complete(Unit) }
        val model = ClientModel(profile, store, local, { transform -> local.update(transform) }) {
            val transport = SshjTransport(); transports += transport
            object : SshTransport by transport {
                override suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?) {
                    gate.await(); transport.connect(host, credential, trustedFingerprint)
                }
            }
        }
        val renderer = HeadlessTerminal()
        model.bindSurface(object : TerminalSurface {
            override suspend fun write(bytes: ByteArray) { renderer.feed(bytes) }
            override suspend fun reset(bytes: ByteArray) { renderer.feed(bytes) }
            override fun close() {}
        })
        suspend fun ready() = withTimeout(15000) { model.status.first { it.phase == Phase.Ready } }
        suspend fun clients(count: Int) = withTimeout(5000) {
            while (admin.execute("tmux list-clients -t '$target' -F '#{client_name}'").decodeToString().lineSequence().count { it.isNotBlank() } != count) delay(50)
        }
        try {
            model.connect(); ready()
            val pane = model.pane.value!!
            val pid = admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString()
            repeat(3) {
                gate = CompletableDeferred()
                transports.last().close()
                withTimeout(5000) { model.status.first { it.phase == Phase.Reconnecting } }
                model.input("printf 'MUST_NOT_REPLAY\\n'\r".encodeToByteArray())
                assertFalse(model.status.value.acceptsInput)
                gate.complete(Unit); ready()
                clients(1) // Old control clients must be reclaimed after each reconnect.
                assertEquals(pid, admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString())
                assertFalse(admin.execute("tmux capture-pane -p -t '$pane'").decodeToString().contains("MUST_NOT_REPLAY"))
            }
            repeat(5) { model.resize(Size(90 + it, 30 + it)); delay(40) }
            ready()
            // Closing a name dialog can resize the viewport before its command runs.
            model.resize(Size(111, 33))
            assertEquals(Phase.Restoring, model.status.value.phase)
            model.manage(TmuxCommand.newWindow(target, "resize-dialog"))
            withTimeout(5000) { model.remote.first { state -> state.windows.any { it.session == target && it.name == "resize-dialog" } } }
            ready()
            // Disconnect must not wait for a channel-close acknowledgement while
            // its receive window is full and the terminal consumer is stalled.
            val stalled = CompletableDeferred<Unit>()
            model.bindSurface(object : TerminalSurface {
                override suspend fun write(bytes: ByteArray) { stalled.complete(Unit); awaitCancellation() }
                override suspend fun reset(bytes: ByteArray) {}
                override fun close() {}
            })
            val outputPane = model.pane.value!!
            admin.execute("tmux send-keys -t '$outputPane' " + TmuxCommand.quote("while :; do printf '%01024d\\n' 0; done") + " Enter")
            withTimeout(5000) { stalled.await() }
            delay(1000)
            val closeStarted = System.nanoTime()
            model.disconnect()
            assertTrue((System.nanoTime() - closeStarted) / 1e9 < 2, "Disconnect must finish within two seconds with a stalled consumer")
            clients(0)
            admin.execute("tmux " + TmuxCommand.killSession(target).wire)
            model.connect()
            withTimeout(15000) { model.status.first { it.phase == Phase.MissingSession } }
            assertFalse(model.status.value.acceptsInput)
            assertTrue(model.remote.value.sessions.none { it.id == target || it.name == name })
        } finally { model.close(); admin.close() }
    }
}
