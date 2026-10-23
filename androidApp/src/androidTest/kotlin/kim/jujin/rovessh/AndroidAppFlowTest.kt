package kim.jujin.rovessh

import android.content.pm.ActivityInfo
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.webkit.WebView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.SshjTransport
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File

class AndroidAppFlowTest {
    private lateinit var app: AppModel
    private lateinit var connection: ClientModel
    private lateinit var previous: LocalState
    @Test fun composeTerminalKeyboardToolbarRotationAndForegroundRestore() = runBlocking<Unit> {
        assumeTrue(InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val fixture = JSONObject(File(instrumentation.targetContext.cacheDir, "integration.json").readText())
        val credential = Credential(privateKey = fixture.getString("key"))
        var profile = HostProfile("android-ui-test", "UI test", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
        val admin = SshjTransport()
        admin.connect(profile, credential, fixture.getString("fingerprint"))
        val name = "android-ui-${System.nanoTime()}"
        admin.execute("tmux " + TmuxCommand.newSession(name).wire)
        val session = RemoteStateDecoder.sessions(admin.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
        profile = profile.copy(lastSession = session)
        var stage = "launch"
        try {
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
              try {
                scenario.onActivity { activity ->
                    app = ViewModelProvider(activity)[ClientViewModel::class.java].app
                    previous = app.local.value
                    app.credentials.write(profile.id, credential)
                    app.update(previous.copy(hosts = previous.hosts + profile, trust = previous.trust + (profile.endpoint to fixture.getString("fingerprint"))))
                    app.open(profile)
                    connection = app.selected.value!!
                }
                stage = "connection ready"
                withTimeout(20000) { connection.status.first { it.phase == Phase.Ready } }
                val pane = connection.pane.value!!
                val pid = admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString()
                stage = "toolbar accessibility"
                try { withTimeout(5000) {
                    while (findText(instrumentation.uiAutomation.rootInActiveWindow, "Esc") == null) delay(50)
                } } catch (e: TimeoutCancellationException) {
                    val root = instrumentation.uiAutomation.rootInActiveWindow
                    instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
                        File(instrumentation.targetContext.cacheDir, "ui-failure.png").outputStream().use {
                            bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
                        }
                    }
                    throw AssertionError("Toolbar timeout: activePackage=${root?.packageName}; children=${root?.childCount}; phase=${connection.status.value.phase}")
                }
                stage = "keyboard visibility"
                var tapX = 0f; var tapY = 0f
                scenario.onActivity { activity ->
                    val web = findWebView(activity.window.decorView)!!
                    val position = IntArray(2); web.getLocationOnScreen(position)
                    tapX = position[0] + web.width / 2f; tapY = position[1] + web.height / 2f
                }
                val now = android.os.SystemClock.uptimeMillis()
                instrumentation.sendPointerSync(android.view.MotionEvent.obtain(now, now, android.view.MotionEvent.ACTION_DOWN, tapX, tapY, 0))
                instrumentation.sendPointerSync(android.view.MotionEvent.obtain(now, now + 80, android.view.MotionEvent.ACTION_UP, tapX, tapY, 0))
                var imeTop = 0
                try { withTimeout(5000) {
                    while (imeTop == 0) {
                        scenario.onActivity { activity ->
                            val decor = activity.window.decorView
                            val insets = decor.rootWindowInsets
                            if (insets?.isVisible(WindowInsets.Type.ime()) == true) imeTop = decor.height - insets.getInsets(WindowInsets.Type.ime()).bottom
                        }
                        delay(50)
                    }
                } } catch (e: TimeoutCancellationException) {
                    val latch = java.util.concurrent.CountDownLatch(1)
                    var diagnostic = ""
                    scenario.onActivity { activity ->
                        val web = findWebView(activity.window.decorView)
                        diagnostic = "nativeFocus=${web?.hasFocus()}, size=${web?.width}x${web?.height}, status=${connection.status.value.phase}"
                        web?.evaluateJavascript("JSON.stringify({active:document.activeElement.tagName,viewport:[innerWidth,innerHeight],terminalHeight:document.querySelector('#terminal').clientHeight})") { diagnostic += it; latch.countDown() }
                    }
                    latch.await(2, java.util.concurrent.TimeUnit.SECONDS)
                    instrumentation.uiAutomation.takeScreenshot()?.let { bitmap -> File(instrumentation.targetContext.cacheDir, "ui-failure.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) } }
                    throw AssertionError("Keyboard: $diagnostic")
                }
                val toolbar = findText(instrumentation.uiAutomation.rootInActiveWindow, "Esc")!!
                val bounds = android.graphics.Rect(); toolbar.getBoundsInScreen(bounds)
                assertTrue("Shortcut bar must stay above the keyboard", bounds.bottom <= imeTop + 8)
                assertTrue("Shortcut bar must be adjacent to the keyboard", imeTop - bounds.bottom < 160)
                instrumentation.sendKeyDownUpSync(android.view.KeyEvent.KEYCODE_BACK)
                delay(400) // Wait for IME insets animation and the debounced remote resize.
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                stage = "GUI pane split"
                val originalPaneIds = connection.remote.value.panes.map { it.id }.toSet()
                clickText("좌우 분할")
                stage = "GUI split remote update"
                val splitPane = withTimeout(10000) { connection.remote.first { state -> state.panes.any { it.id !in originalPaneIds } } }.panes.single { it.id !in originalPaneIds }
                clickMatching { it.text?.toString()?.startsWith(splitPane.id + " ") == true }
                withTimeout(10000) { connection.pane.first { it == splitPane.id } }
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                stage = "GUI window creation"
                clickText("+ 창")
                stage = "GUI name editor"
                // Wait for the dialog window; the terminal itself also exposes an
                // editable textarea and must not receive the test's name input.
                withTimeout(5000) {
                    while (findText(instrumentation.uiAutomation.rootInActiveWindow, "창 만들기") == null) delay(50)
                }
                val editor = withTimeout(5000) {
                    var found: android.view.accessibility.AccessibilityNodeInfo? = null
                    while (found == null) { found = findNode(instrumentation.uiAutomation.rootInActiveWindow) { it.isEditable }; if (found == null) delay(50) }
                    found
                }
                val text = android.os.Bundle().apply { putCharSequence(android.view.accessibility.AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "GUI window") }
                assertTrue(editor.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                stage = "GUI name confirmation"
                clickText("확인")
                stage = "GUI remote window update"
                withTimeout(10000) { connection.remote.first { state -> state.windows.any { it.name == "GUI window" && it.session == session } } }
                stage = "font setting and renderer replacement"
                var previousView: WebView? = null
                scenario.onActivity { previousView = findWebView(it.window.decorView) }
                clickText("설정"); clickText("크게"); clickText("닫기")
                withTimeout(10000) {
                    while (true) {
                        var replaced = false
                        scenario.onActivity { activity ->
                            val current = findWebView(activity.window.decorView)
                            replaced = current != null && current !== previousView && current.isAttachedToWindow
                        }
                        if (replaced && connection.status.value.phase == Phase.Ready) break
                        delay(50)
                    }
                }
                stage = "rotation and foreground"
                scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE }
                delay(700)
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                scenario.moveToState(Lifecycle.State.CREATED)
                scenario.moveToState(Lifecycle.State.RESUMED)
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                assertEquals(pid, admin.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString())
                scenario.onActivity { app.removeConnection(connection); app.credentials.delete(profile.id); app.update(previous) }
              } catch (e: TimeoutCancellationException) {
                val root = instrumentation.uiAutomation.rootInActiveWindow
                instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
                    File(instrumentation.targetContext.cacheDir, "ui-failure.png").outputStream().use {
                        bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
                    }
                }
                throw AssertionError("Android UI timeout: $stage; activePackage=${root?.packageName}; phase=${connection.status.value.phase}")
              }
            }
        } catch (e: TimeoutCancellationException) {
            val root = instrumentation.uiAutomation.rootInActiveWindow
            throw AssertionError("Android UI timeout: $stage; activePackage=${root?.packageName}; activeClass=${root?.className}; children=${root?.childCount}")
        }
        finally {
            if (::connection.isInitialized) connection.close()
            if (::app.isInitialized) { app.credentials.delete(profile.id); app.update(previous) }
            runCatching { admin.execute("tmux " + TmuxCommand.killSession(session).wire) }
            admin.close()
        }
    }
    private fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) for (i in 0 until view.childCount) findWebView(view.getChildAt(i))?.let { return it }
        return null
    }
    private fun findText(node: android.view.accessibility.AccessibilityNodeInfo?, value: String): android.view.accessibility.AccessibilityNodeInfo? {
        return findNode(node) { it.text?.toString() == value || it.contentDescription?.toString() == value }
    }
    private fun findNode(node: android.view.accessibility.AccessibilityNodeInfo?, predicate: (android.view.accessibility.AccessibilityNodeInfo) -> Boolean): android.view.accessibility.AccessibilityNodeInfo? {
        if (node == null) return null
        if (predicate(node)) return node
        for (i in 0 until node.childCount) findNode(node.getChild(i), predicate)?.let { return it }
        return null
    }
    private suspend fun clickText(value: String) = clickMatching { it.text?.toString() == value || it.contentDescription?.toString() == value }
    private suspend fun clickMatching(predicate: (android.view.accessibility.AccessibilityNodeInfo) -> Boolean) {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        withTimeout(5000) {
            while (true) {
                var node = findNode(automation.rootInActiveWindow, predicate)
                while (node != null && !node.isClickable) node = node.parent
                if (node?.isEnabled == true && node.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK)) break
                delay(50)
            }
        }
    }
}
