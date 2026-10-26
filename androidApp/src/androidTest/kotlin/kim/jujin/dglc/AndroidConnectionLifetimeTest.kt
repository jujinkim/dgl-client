package kim.jujin.dglc

import android.Manifest
import android.app.NotificationManager
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.webkit.WebView
import androidx.lifecycle.Lifecycle
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File

/** One API36 smoke: real SSH generation, native IME, service notification and task teardown. */
class AndroidConnectionLifetimeTest {
    private lateinit var app: AppModel
    private lateinit var client: ClientConnection
    private lateinit var previous: LocalState
    @Test fun imeBackgroundIntentAndTaskRemovalPreserveOnlyRemoteWork() = runBlocking<Unit> {
        assumeTrue(InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        instrumentation.uiAutomation.grantRuntimePermission(context.packageName, Manifest.permission.POST_NOTIFICATIONS)
        val fixture = JSONObject(File(context.cacheDir, "integration.json").readText())
        val credential = Credential(privateKey = fixture.getString("key"))
        var host = HostProfile("lifetime-smoke", "Lifetime", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
        val admin = SshjTransport()
        admin.connect(host, credential, fixture.getString("fingerprint"))
        val name = "lifetime-${System.nanoTime()}"
        admin.execute("tmux " + TmuxCommand.newSession(name).wire)
        val session = RemoteStateDecoder.sessions(admin.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
        host = host.copy(lastSession = session)
        var stage = "connect"
        val manager = context.getSystemService(NotificationManager::class.java)
        fun notification() = manager.activeNotifications.firstOrNull { it.id == ConnectionService.NOTIFICATION }
        suspend fun waitUntil(check: suspend () -> Boolean) { withTimeout(10_000) { while (!check()) delay(30) } }
        try {
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", keepConnectionsInBackground = true)) }
                    app.credentials.write(host.id, credential)
                    app.open(host); client = app.selected.value!!
                }
                withTimeout(20_000) { client.status.first { it.phase == Phase.TrustRequired } }
                scenario.onActivity { client.trust() }
                withTimeout(20_000) { client.status.first { it.phase == Phase.Ready } }
                waitUntil { notification() != null }
                assertNull(app.backgroundConnectionError.value)
                val pane = client.pane.value!!
                val target = client.captureInputTarget()!!
                val pid = admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString()
                val sshClients = admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString()
                stage = "IME"
                for (show in listOf(true, false)) {
                    scenario.onActivity { activity ->
                        val web = webView(activity.window.decorView)!!
                        web.requestFocus()
                        web.evaluateJavascript("window.focusTerminal()") {
                            if (show) web.windowInsetsController?.show(WindowInsets.Type.ime())
                            else web.windowInsetsController?.hide(WindowInsets.Type.ime())
                        }
                    }
                    withTimeout(5000) {
                        var visible = !show
                        while (visible != show) {
                            scenario.onActivity { visible = it.window.decorView.rootWindowInsets.isVisible(WindowInsets.Type.ime()) }
                            delay(30)
                        }
                    }
                    delay(500)
                    withTimeout(10_000) { client.status.first { it.phase == Phase.Ready } }
                    assertEquals(target, client.captureInputTarget())
                }
                stage = "background and recreation"
                scenario.moveToState(Lifecycle.State.CREATED)
                delay(300)
                assertEquals(target, client.captureInputTarget())
                assertNotNull(notification())
                scenario.moveToState(Lifecycle.State.RESUMED)
                scenario.recreate()
                scenario.onActivity { assertSame(app, (it.application as DglApplication).connections().app) }
                withTimeout(10_000) { client.status.first { it.phase == Phase.Ready } }
                stage = "notification intent"
                val before = client.captureInputTarget()
                lateinit var activity: MainActivity
                lateinit var launchIntent: android.content.Intent
                scenario.onActivity { activity = it; launchIntent = it.intent }
                notification()!!.notification.contentIntent.send()
                instrumentation.waitForIdleSync()
                delay(300)
                instrumentation.runOnMainSync { activity.intent = launchIntent }
                assertEquals(before, client.captureInputTarget())
                assertEquals(sshClients, admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString())
                stage = "task removal"
                scenario.onActivity { it.finishAndRemoveTask() }
                waitUntil { app.connections.value.isEmpty() && notification() == null }
                assertEquals(Phase.Disconnected, client.status.value.phase)
                assertNotNull(client.sendInput(target, "must-not-replay".encodeToByteArray()).error)
                waitUntil { admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString().isBlank() }
                assertEquals(pid, admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString())
                delay(250)
                assertNull(notification())
            }
        } catch (error: Exception) { throw AssertionError("Lifecycle smoke failed at $stage", error) }
        finally {
            if (::app.isInitialized) { app.credentials.delete(host.id); app.update(previous) }
            if (::client.isInitialized) client.close()
            runCatching { admin.execute("tmux " + TmuxCommand.killSession(session).wire) }
            admin.close()
        }
    }
    private fun webView(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webView(view.getChildAt(it)) }
        else -> null
    }
}
