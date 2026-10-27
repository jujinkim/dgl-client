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
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry
import androidx.test.runner.lifecycle.Stage
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.collect
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
        val launcher = context.packageManager.getLaunchIntentForPackage(context.packageName)!!
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
            ActivityScenario.launch<MainActivity>(launcher).use { scenario ->
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
                assertEquals(R.drawable.ic_notification, notification()!!.notification.smallIcon.resId)
                assertNull(app.backgroundConnectionError.value)
                val pane = client.pane.value!!
                val target = client.captureInputTarget()!!
                val pid = admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString()
                val sshClients = admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString()
                suspend fun restored() {
                    withTimeout(10_000) { client.status.first { it.phase == Phase.Ready && it.acceptsInput && it.recoveryMessage == null } }
                }
                restored()
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
                    restored()
                    assertEquals(target, client.captureInputTarget())
                }
                stage = "root back and home resume"
                lateinit var warmActivity: MainActivity
                scenario.onActivity { warmActivity = it; assertTrue("Launcher Activity must be the task root", it.isTaskRoot) }
                fun shell(command: String) = instrumentation.uiAutomation.executeShellCommand(command).use {
                    android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes()
                }
                for (key in listOf("KEYCODE_HOME")) {
                    stage = "$key background"
                    shell("input keyevent $key")
                    waitUntil { scenario.state !in setOf(Lifecycle.State.RESUMED, Lifecycle.State.STARTED) }
                    assertEquals("$key must background the root Activity", Lifecycle.State.CREATED, scenario.state)
                    assertSame(app, (context.applicationContext as DglApplication).currentConnections()!!.app)
                    val resumeStates = mutableListOf<ConnectionStatus>()
                    val observing = launch(Dispatchers.Unconfined) { client.status.collect { resumeStates += it } }
                    val checked = async(Dispatchers.Unconfined) { withTimeout(10_000) { client.status.first { it.checking } } }
                    shell("am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n kim.jujin.dglc/.MainActivity")
                    stage = "$key foreground"
                    waitUntil { scenario.state == Lifecycle.State.RESUMED }
                    stage = "$key restoration"
                    checked.await()
                    restored()
                    observing.cancelAndJoin()
                    assertTrue("A healthy return dims immediately while checking: $resumeStates", resumeStates.any {
                        it.checking && it.recoveryMessage == t("연결 확인 중…") && !it.acceptsInput
                    })
                    assertTrue("A healthy return needs no SSH reconnect or retry controls: $resumeStates", resumeStates.none {
                        it.showsConnectionProgress || it.canRetryNow || it.canStopRetrying
                    })
                    assertTrue("Any pending screen resync stays dimmed and blocks input: $resumeStates", resumeStates.filter { it.recovering }.all {
                        it.recoveryMessage != null && !it.acceptsInput
                    })
                    scenario.onActivity { assertSame(warmActivity, it) }
                    assertEquals(target, client.captureInputTarget())
                    assertEquals(sshClients, admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString())
                }
                stage = "background and recreation"
                scenario.moveToState(Lifecycle.State.CREATED)
                delay(300)
                assertEquals(target, client.captureInputTarget())
                assertNotNull(notification())
                scenario.moveToState(Lifecycle.State.RESUMED)
                scenario.recreate()
                scenario.onActivity { assertSame(app, (it.application as DglApplication).connections().app) }
                restored()
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
                stage = "background with maintenance disabled"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(keepConnectionsInBackground = false)) } }
                waitUntil { notification() == null }
                scenario.moveToState(Lifecycle.State.CREATED)
                withTimeout(10_000) { client.status.first { it.phase == Phase.Disconnected } }
                assertTrue("The same tab stays open", app.connections.value.any { it === client })
                waitUntil { admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString().isBlank() }
                assertNotNull(client.sendInput(target, "must-not-replay".encodeToByteArray()).error)
                scenario.onActivity { client.suspendForBackground() }
                scenario.moveToState(Lifecycle.State.RESUMED)
                scenario.onActivity { client.onResume(); client.onResume() }
                restored()
                val resumedTarget = client.captureInputTarget()!!
                assertNotEquals(target.epoch, resumedTarget.epoch)
                assertEquals(session, client.session.value)
                assertEquals(pane, client.pane.value)
                assertEquals(pid, admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString())
                val resumedClients = admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString()
                assertEquals("duplicate resumes must open only one tmux client", 1, resumedClients.trim().lines().size)
                assertNotEquals(sshClients, resumedClients)
                assertNull(notification())
                stage = "manual stop cancels background resume"
                scenario.moveToState(Lifecycle.State.CREATED)
                withTimeout(10_000) { client.status.first { it.phase == Phase.Disconnected } }
                scenario.onActivity { client.disconnect() }
                scenario.moveToState(Lifecycle.State.RESUMED)
                scenario.onActivity { client.onResume() }
                delay(300)
                assertEquals(Phase.Disconnected, client.status.value.phase)
                assertNull(client.captureInputTarget())
                waitUntil { admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString().isBlank() }
                stage = "retry stop cancels background resume"
                scenario.onActivity { client.connect() }
                restored()
                scenario.moveToState(Lifecycle.State.CREATED)
                withTimeout(10_000) { client.status.first { it.phase == Phase.Disconnected } }
                scenario.onActivity { client.stopRetrying() }
                scenario.moveToState(Lifecycle.State.RESUMED)
                delay(300)
                assertEquals(Phase.Disconnected, client.status.value.phase)
                stage = "task removal cancels background resume"
                scenario.onActivity { client.connect() }
                restored()
                scenario.moveToState(Lifecycle.State.CREATED)
                withTimeout(10_000) { client.status.first { it.phase == Phase.Disconnected } }
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
            // Android's root-Back rule requires a HOME launch source. An
            // instrumentation/shell start has a different source even with MAIN/LAUNCHER.
            stage = "fresh home-launch after task removal"
            fun shell(command: String) = instrumentation.uiAutomation.executeShellCommand(command).use {
                android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes()
            }
            shell("input keyevent KEYCODE_HOME")
            val metrics = context.resources.displayMetrics
            shell("input swipe ${metrics.widthPixels / 2} ${metrics.heightPixels * 9 / 10} ${metrics.widthPixels / 2} ${metrics.heightPixels / 3} 250")
            var icon: android.view.accessibility.AccessibilityNodeInfo? = null
            waitUntil {
                icon = instrumentation.uiAutomation.rootInActiveWindow?.findAccessibilityNodeInfosByText("DaGaLi")?.firstOrNull { it.isClickable }
                icon != null
            }
            assertTrue(icon!!.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK))
            var fresh: MainActivity? = null
            waitUntil {
                instrumentation.runOnMainSync {
                    fresh = ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED).filterIsInstance<MainActivity>().singleOrNull()
                }
                fresh != null
            }
            val oldApp = app
            instrumentation.runOnMainSync {
                app = (fresh!!.application as DglApplication).connections().app
                assertNotSame(oldApp, app)
                assertTrue(app.connections.value.isEmpty())
                app.modify { it.copy(preferences = it.preferences.copy(keepConnectionsInBackground = true)) }
                app.open(host); client = app.selected.value!!
            }
            withTimeout(20_000) { client.status.first { it.acceptsInput } }
            val warm = fresh!!
            val target = client.captureInputTarget()
            stage = "home-launched root back"
            shell("input keyevent KEYCODE_BACK")
            waitUntil { warm.lifecycle.currentState == Lifecycle.State.CREATED }
            assertFalse(warm.isFinishing)
            val checked = async(Dispatchers.Unconfined) { withTimeout(10_000) { client.status.first { it.checking } } }
            shell("am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n kim.jujin.dglc/.MainActivity")
            checked.await()
            withTimeout(10_000) { client.status.first { it.acceptsInput && it.recoveryMessage == null } }
            assertEquals(target, client.captureInputTarget())
            stage = "deleted session stays missing after automatic resume"
            instrumentation.runOnMainSync { app.modify { it.copy(preferences = it.preferences.copy(keepConnectionsInBackground = false)) } }
            shell("input keyevent KEYCODE_HOME")
            withTimeout(10_000) { client.status.first { it.phase == Phase.Disconnected } }
            admin.execute("tmux " + TmuxCommand.killSession(session).wire)
            shell("am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n kim.jujin.dglc/.MainActivity")
            withTimeout(10_000) { client.status.first { it.phase == Phase.MissingSession } }
            instrumentation.runOnMainSync { client.onResume(); client.onResume() }
            delay(200)
            assertEquals(Phase.MissingSession, client.status.value.phase)
            assertNull(client.pane.value)
            assertTrue(client.remote.value.sessions.none { it.id == session })
            stage = "authentication failure cannot reserve automatic resume"
            instrumentation.runOnMainSync {
                app.credentials.write(host.id, Credential(privateKey = "invalid-private-key"))
                client.connect()
            }
            withTimeout(10_000) { client.status.first { it.phase == Phase.Failed } }
            val failedGeneration = client.generation
            instrumentation.runOnMainSync { client.suspendForBackground(); client.onResume(); client.onResume() }
            delay(200)
            assertEquals(Phase.Failed, client.status.value.phase)
            assertEquals(failedGeneration, client.generation)
            assertFalse(client.status.value.canRetryNow)
            instrumentation.runOnMainSync { assertEquals(Lifecycle.State.RESUMED, warm.lifecycle.currentState); warm.finishAndRemoveTask() }
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
