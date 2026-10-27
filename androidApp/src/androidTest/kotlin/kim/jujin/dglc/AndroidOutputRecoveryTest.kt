package kim.jujin.dglc

import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/** Real Rust/SSH with controlled renderer ACKs; no physical-device timing claims. */
class AndroidOutputRecoveryTest {
    private class Surface : TerminalSurface {
        val rendered = ConcurrentHashMap.newKeySet<String>()
        val entered = Channel<String>(Channel.UNLIMITED)
        @Volatile var held: CompletableDeferred<Unit>? = null
        @Volatile var heldPane: String? = null
        @Volatile var fail = false
        private var pane = ""
        override suspend fun outputPane(pane: String, target: String) { this.pane = pane }
        override suspend fun write(bytes: ByteArray) {}
        override suspend fun reset(bytes: ByteArray) {}
        override suspend fun endSnapshot() {
            if (fail) error("renderer unavailable")
            val ack = held
            if (ack != null && !ack.isCompleted && (heldPane == null || heldPane == pane)) {
                entered.send(pane)
                withTimeout(15_000) { ack.await() }
            }
            rendered += pane
        }
        override fun close() {}
    }

    @Test fun timedOutOutputResumesWithoutSshReconnectAndFailedRetryNeedsManualAction() = runBlocking<Unit> {
        assumeTrue(InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val fixture = JSONObject(File(context.cacheDir, "integration.json").readText())
        val credential = Credential(privateKey = fixture.getString("key"))
        var host = HostProfile("output-recovery", "Recovery", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
        val admin = SshjTransport()
        admin.connect(host, credential, fixture.getString("fingerprint"))
        val name = "output-${System.nanoTime()}"
        admin.execute("tmux " + TmuxCommand.newSession(name).wire)
        val session = RemoteStateDecoder.sessions(admin.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
        admin.execute("tmux split-window -h -t '$session'")
        host = host.copy(lastSession = session)
        val secrets = ConcurrentHashMap<String, Credential>().apply { put(host.credentialRef, credential) }
        val credentials = object : CredentialStore {
            override fun read(reference: String) = secrets[reference]
            override fun write(reference: String, credential: Credential) { secrets[reference] = credential }
            override fun delete(reference: String) { secrets.remove(reference) }
        }
        val local = MutableStateFlow(LocalState())
        val client = RustClientConnection(host, credentials, local) { local.value = it(local.value) }
        val surface = Surface()
        suspend fun ready() { withTimeout(10_000) { client.status.first { it.acceptsInput && !it.recovering } } }
        suspend fun failed() { withTimeout(20_000) { client.status.first { it.recovering && it.canRetryNow && !it.checking } } }
        suspend fun clients() = admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString()
        suspend fun pids() = admin.execute("tmux list-panes -t '$session' -F '#{pane_pid}'").decodeToString()
        try {
            client.bindSurface(surface); client.connect()
            withTimeout(20_000) { client.status.first { it.phase == Phase.TrustRequired } }
            client.trust(); ready()
            withTimeout(10_000) { while (surface.rendered.size != 2) delay(20) }
            val generation = client.generation
            val target = client.captureInputTarget()!!
            val sshClients = clients()
            val remotePids = pids()

            // A real 15s renderer deadline must enter failure and release the queue.
            val expired = CompletableDeferred<Unit>()
            surface.held = expired
            client.restore()
            withTimeout(10_000) { surface.entered.receive() }
            failed()
            assertEquals(generation, client.generation)
            assertNotNull(client.sendInput(target, "must-not-replay".encodeToByteArray()).error)

            // Only one visible pane ACK is withheld; connection check alone must
            // not clear recovery. Late ACK from the old writer cannot clear it either.
            val current = CompletableDeferred<Unit>()
            surface.heldPane = surface.rendered.sorted().last()
            surface.rendered.clear(); surface.held = current
            client.onResume(); client.onResume()
            assertFalse(client.status.value.acceptsInput)
            withTimeout(10_000) { surface.entered.receive() }
            expired.complete(Unit)
            withTimeout(10_000) { client.status.first { !it.checking } }
            assertTrue(client.status.value.recovering)
            assertFalse(client.status.value.acceptsInput)
            current.complete(Unit); ready()
            assertEquals(2, surface.rendered.size)
            assertEquals(target, client.captureInputTarget())
            assertEquals(generation, client.generation)
            assertEquals(sshClients, clients())
            assertEquals(remotePids, pids())

            // A healthy connection check can finish before a changed pane has
            // rendered. Keep dim/input blocking until its already-running ACK.
            val changedPane = CompletableDeferred<Unit>()
            surface.held = changedPane
            client.restore()
            withTimeout(10_000) { surface.entered.receive() }
            client.onResume()
            withTimeout(10_000) { client.status.first { !it.checking } }
            assertTrue(client.status.value.recovering)
            assertFalse(client.status.value.acceptsInput)
            changedPane.complete(Unit); ready()

            // The automatic screen retry may fail once; duplicate resume events
            // must not start a loop. Manual retry can repair it on the same SSH.
            surface.held = null; surface.fail = true
            client.restore(); failed()
            client.onResume(); failed()
            surface.fail = false
            client.onResume(); client.onResume()
            delay(250)
            assertTrue(client.status.value.recovering)
            assertTrue(client.status.value.canRetryNow)
            assertFalse(client.status.value.acceptsInput)
            client.retryNow(); ready()

            // Replacement/rotation while waiting for an ACK cannot let the old
            // writer's cleanup remove the new subscription or mark it failed.
            val abandoned = CompletableDeferred<Unit>()
            surface.held = abandoned; surface.heldPane = null
            client.restore()
            withTimeout(10_000) { surface.entered.receive() }
            val replacement = Surface()
            client.bindSurface(replacement)
            client.unbindSurface(surface)
            abandoned.complete(Unit)
            withTimeout(10_000) { while (replacement.rendered.size != 2) delay(20) }
            ready()
            assertEquals(generation, client.generation)
            assertEquals(sshClients, clients())
            assertEquals(remotePids, pids())
        } finally {
            client.close()
            runCatching { admin.execute("tmux " + TmuxCommand.killSession(session).wire) }
            admin.close()
        }
    }
}
