package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.SshjTransport
import kim.jujin.rovessh.storage.MemoryCredentialStore
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
            model.disconnect()
            clients(0)
            admin.execute("tmux " + TmuxCommand.killSession(target).wire)
            model.connect()
            withTimeout(15000) { model.status.first { it.phase == Phase.MissingSession } }
            assertFalse(model.status.value.acceptsInput)
            assertTrue(model.remote.value.sessions.none { it.id == target || it.name == name })
        } finally { model.close(); admin.close() }
    }
}
