package kim.jujin.dglc

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.core.app.ActivityScenario
import android.widget.FrameLayout
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.AndroidTerminal
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
    private class Surface(private val delegate: TerminalSurface? = null) : TerminalSurface {
        val rendered = ConcurrentHashMap.newKeySet<String>()
        val entered = Channel<String>(Channel.UNLIMITED)
        @Volatile var held: CompletableDeferred<Unit>? = null
        @Volatile var heldPane: String? = null
        @Volatile var fail = false
        @Volatile var pending = false
        @Volatile var snapshotWrites = 0
        private var snapshot = false
        private var pane = ""
        override suspend fun presentationPending(active: Boolean) { delegate?.presentationPending(active); pending = active }
        override suspend fun layout(remote: RemoteState, window: String?, pane: String?, inputTarget: String?) { delegate?.layout(remote, window, pane, inputTarget) }
        override suspend fun outputPane(pane: String, target: String) { this.pane = pane; delegate?.outputPane(pane, target) }
        override suspend fun resizeRemote(size: Size) { delegate?.resizeRemote(size) }
        override suspend fun beginSnapshot() { delegate?.beginSnapshot() }
        override suspend fun cancelSnapshot() { delegate?.cancelSnapshot() }
        override suspend fun connectionRecovery(active: Boolean) { delegate?.connectionRecovery(active) }
        override suspend fun write(bytes: ByteArray) { if (snapshot) snapshotWrites++; delegate?.write(bytes) }
        override suspend fun reset(bytes: ByteArray) { snapshot = true; delegate?.reset(bytes) }
        override suspend fun endSnapshot() {
            if (fail) error("renderer unavailable")
            delegate?.endSnapshot()
            val ack = held
            if (ack != null && !ack.isCompleted && (heldPane == null || heldPane == pane)) {
                entered.send(pane)
                withTimeout(15_000) { ack.await() }
            }
            rendered += pane
            snapshot = false
        }
        override fun close() {}
    }

    @Test fun tabRebindAndWindowSwitchWaitForFreshVisiblePanes() = runBlocking<Unit> {
        assumeTrue(InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val fixture = JSONObject(File(context.cacheDir, "integration.json").readText())
        val credential = Credential(privateKey = fixture.getString("key"))
        var host = HostProfile("fresh-screen", "Fresh screen", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
        val admin = SshjTransport()
        admin.connect(host, credential, fixture.getString("fingerprint"))
        val name = "presentation-${System.nanoTime()}"
        admin.execute("tmux " + TmuxWire.newSession(name).wire)
        val session = TmuxRows.sessions(admin.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
        val firstWindow = admin.execute("tmux display-message -p -t '$session' '#{window_id}'").decodeToString().trim()
        admin.execute("tmux split-window -h -t '$firstWindow'")
        val firstPanes = admin.execute("tmux list-panes -t '$firstWindow' -F '#{pane_id}'").decodeToString().trim().lines().sorted()
        val secondWindow = admin.execute("tmux new-window -t '$session' -P -F '#{window_id}'").decodeToString().trim()
        admin.execute("tmux split-window -h -t '$secondWindow'")
        val secondPanes = admin.execute("tmux list-panes -t '$secondWindow' -F '#{pane_id}'").decodeToString().trim().lines().sorted()
        for (pane in firstPanes + secondPanes) {
            val marker = pane.removePrefix("%")
            val command = "printf 'MAIN_$marker\\n\\033[?1049h\\033[H\\033[2JTUI_$marker\\n'\r".encodeToByteArray()
            admin.execute("tmux send-keys -t '$pane' -H " + command.joinToString(" ") { "%02x".format(it.toInt() and 255) })
            withTimeout(5000) {
                while (admin.execute("tmux display-message -p -t '$pane' '#{alternate_on}'").decodeToString().trim() != "1") delay(20)
            }
        }
        admin.execute("tmux select-window -t '$firstWindow'")
        host = host.copy(lastSession = session)
        val secrets = ConcurrentHashMap<String, Credential>().apply { put(host.credentialRef, credential) }
        val credentials = object : CredentialStore {
            override fun read(reference: String) = secrets[reference]
            override fun write(reference: String, credential: Credential) { secrets[reference] = credential }
            override fun delete(reference: String) { secrets.remove(reference) }
        }
        val local = MutableStateFlow(LocalState())
        val client = RustClientConnection(host, credentials, local) { local.value = it(local.value) }
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        lateinit var renderer: AndroidTerminal
        scenario.onActivity { activity ->
            renderer = AndroidTerminal(activity, Preferences(), {}, { client.resize(it) })
            activity.setContentView(FrameLayout(activity).apply { addView(renderer.view, FrameLayout.LayoutParams(-1, -1)) })
        }
        val surface = Surface(renderer)
        suspend fun ready() {
            withTimeout(10_000) { client.status.first { it.acceptsInput && !it.recovering && !it.presenting } }
            assertFalse(surface.pending); assertFalse(renderer.screenPending)
        }
        suspend fun checkTui(panes: List<String>) {
            val text = withContext(Dispatchers.Main) {
                val result = CompletableDeferred<String>()
                renderer.view.evaluateJavascript("[...document.querySelectorAll('.pane .xterm-screen')].map(s=>s.textContent).join('|')") { result.complete(it) }
                result.await()
            }
            for (pane in panes) assertTrue("Restored alternate buffer must show the TUI for $pane: $text", text.contains("TUI_${pane.removePrefix("%")}"))
            assertFalse("The primary-buffer reconstruction must not be the displayed screen", text.contains("MAIN_"))
        }
        suspend fun held(panes: List<String>) {
            assertEquals(panes.last(), withTimeout(10_000) { surface.entered.receive() })
            assertTrue(surface.pending)
            assertTrue(renderer.screenPending)
            instrumentation.runOnMainSync {
                val bitmap = android.graphics.Bitmap.createBitmap(renderer.view.width, renderer.view.height, android.graphics.Bitmap.Config.ARGB_8888)
                try {
                    renderer.view.draw(android.graphics.Canvas(bitmap))
                    assertEquals("Partial pane data must remain under an opaque native curtain",
                        android.graphics.Color.rgb(17, 21, 27), bitmap.getPixel(bitmap.width / 2, bitmap.height / 2))
                } finally { bitmap.recycle() }
            }
            assertTrue(client.status.value.presenting)
            assertFalse(client.status.value.acceptsInput)
            assertTrue("Other panes may finish without exposing the window", surface.rendered.contains(panes.first()))
            surface.held!!.complete(Unit); ready()
        }
        try {
            surface.held = CompletableDeferred(); surface.heldPane = firstPanes.last()
            client.bindSurface(surface); client.connect()
            withTimeout(20_000) { client.status.first { it.phase == Phase.TrustRequired } }
            client.trust(); held(firstPanes)
            checkTui(firstPanes)
            val generation = client.generation
            val pids = admin.execute("tmux list-panes -s -t '$session' -F '#{pane_pid}'").decodeToString()
            surface.rendered.clear(); surface.snapshotWrites = 0
            surface.held = CompletableDeferred()
            client.unbindSurface(surface); client.bindSurface(surface)
            held(firstPanes)
            assertEquals("One bounded capture write per pane, rather than two per row", 2, surface.snapshotWrites)
            surface.rendered.clear(); surface.snapshotWrites = 0
            surface.held = CompletableDeferred(); surface.heldPane = secondPanes.last()
            client.selectTarget(session, secondWindow, null)
            held(secondPanes)
            checkTui(secondPanes)
            assertEquals(secondWindow, client.status.value.window)
            assertEquals(2, surface.snapshotWrites)
            assertEquals(generation, client.generation)
            assertEquals(pids, admin.execute("tmux list-panes -s -t '$session' -F '#{pane_pid}'").decodeToString())
            surface.held = null
            for ((kind, action) in listOf<Pair<String, () -> Unit>>(
                "tab-rebind" to { client.unbindSurface(surface); client.bindSurface(surface) },
                "tmux-window" to { client.selectTarget(session, firstWindow, null) })) {
                surface.rendered.clear()
                val started = android.os.SystemClock.elapsedRealtime()
                action()
                withTimeout(10_000) { while (surface.rendered.size != 2) delay(5) }
                ready()
                android.util.Log.i("DglPresentationTest", "$kind elapsedMs=${android.os.SystemClock.elapsedRealtime() - started}")
            }
        } finally {
            client.close()
            instrumentation.runOnMainSync { renderer.close() }
            scenario.close()
            admin.execute("tmux " + TmuxWire.killSession(session).wire)
            admin.close()
        }
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
        admin.execute("tmux " + TmuxWire.newSession(name).wire)
        val session = TmuxRows.sessions(admin.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
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
            runCatching { admin.execute("tmux " + TmuxWire.killSession(session).wire) }
            admin.close()
        }
    }
}
