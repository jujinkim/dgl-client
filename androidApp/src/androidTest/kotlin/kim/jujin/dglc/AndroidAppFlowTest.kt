package kim.jujin.dglc

import android.content.pm.ActivityInfo
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

class AndroidAppFlowTest {
    private lateinit var app: AppModel
    private lateinit var connection: ClientConnection
    private lateinit var previous: LocalState
    @Test fun composeTerminalKeyboardToolbarRotationAndForegroundRestore() = runBlocking<Unit> {
        assumeTrue(InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val fixture = JSONObject(File(instrumentation.targetContext.cacheDir, "integration.json").readText())
        val credential = Credential(privateKey = fixture.getString("key"))
        run {
            assertTrue(kim.jujin.dglc.RustKeyTools.publicKey(credential).startsWith("ssh-ed25519 "))
            assertTrue(kim.jujin.dglc.RustKeyTools.publicKey(Credential(privateKey = fixture.getString("rsa"))).startsWith("ssh-rsa "))
            assertTrue(kim.jujin.dglc.RustKeyTools.publicKey(Credential(privateKey = fixture.getString("encrypted"), passphrase = fixture.getString("passphrase"))).startsWith("ssh-ed25519 "))
        }
        var profile = HostProfile("android-ui-test", "개발 서버 · 긴 호스트 이름 확인용 · production candidate", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
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
                    app = (activity.application as DglApplication).connections().app
                    previous = app.local.value
                    app.credentials.write(profile.id, credential)
                    app.update(previous.copy(hosts = previous.hosts + profile, trust = previous.trust + (profile.endpoint to fixture.getString("fingerprint"))))
                    app.update(app.local.value.copy(preferences = app.local.value.preferences.copy(language = "ko", volumeKeysEnabled = true, smoothTerminalTransitions = false, mobilePaneToolbarVisible = true, showAllHierarchy = true,
                        volumeUp = VolumeBinding(VolumeAction.Macro, listOf(
                            MacroStep.Text("dgl_volume_count=\$((\${dgl_volume_count:-0}+1)); tmux set-option -p -t \"\$TMUX_PANE\" @dgl-volume-count \"\$dgl_volume_count\""),
                            MacroStep.Key(TerminalKey.Enter))))))
                    app.open(profile)
                    connection = app.selected.value!!
                }
                run {
                    withTimeout(20000) { connection.status.first { it.phase == Phase.TrustRequired } }
                    assertEquals(fixture.getString("fingerprint"), connection.trustRequest.value!!.fingerprint)
                    scenario.onActivity { connection.trust() }
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
                val sessionLabel = "${connection.host.alias} · $name"
                assertNotNull(findText(instrumentation.uiAutomation.rootInActiveWindow, sessionLabel))

                assertNotNull(findText(instrumentation.uiAutomation.rootInActiveWindow, sessionLabel))
                clickText("tmux 관리")
                clickText("터미널로 돌아가기")
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                withTimeout(5000) {
                    while (true) {
                        var focused = false
                        scenario.onActivity { focused = it.hasWindowFocus() && it.volumeKeys.focused }
                        if (focused) break
                        delay(20)
                    }
                }
                instrumentation.waitForIdleSync()
                delay(350) // The modal window must finish its IME-control handoff before a new tap.
                val clientsBeforeKeyboard = admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString().trim()
                val targetBeforeKeyboard = connection.captureInputTarget()!!
                val keyboardStates = java.util.Collections.synchronizedList(mutableListOf<ConnectionStatus>())
                val keyboardObserver = launch(start = CoroutineStart.UNDISPATCHED) {
                    connection.status.collect { keyboardStates.add(it) }
                }
                var keyboardView: WebView? = null
                var tapX = 0f; var tapY = 0f
                scenario.onActivity { activity ->
                    val web = findWebView(activity.window.decorView)!!
                    keyboardView = web
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
                        web?.evaluateJavascript("JSON.stringify({active:document.activeElement.tagName,viewport:[innerWidth,innerHeight],terminalHeight:document.querySelector('#terminal').clientHeight,panes:[...document.querySelectorAll('.pane')].map(p=>({display:p.style.display,bounds:p.getBoundingClientRect().toJSON()}))})") { diagnostic += it; latch.countDown() }
                    }
                    latch.await(2, java.util.concurrent.TimeUnit.SECONDS)
                    instrumentation.uiAutomation.takeScreenshot()?.let { bitmap -> File(instrumentation.targetContext.cacheDir, "ui-failure.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) } }
                    throw AssertionError("Keyboard: $diagnostic")
                }
                val toolbar = findText(instrumentation.uiAutomation.rootInActiveWindow, "Esc")!!
                val bounds = android.graphics.Rect(); toolbar.getBoundsInScreen(bounds)
                assertTrue("Shortcut bar must stay above the keyboard", bounds.bottom <= imeTop + 8)
                assertTrue("Shortcut bar must be adjacent to the keyboard", imeTop - bounds.bottom < 160)
                stage = "volume macro with IME visible"
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                runVolumeMacro(scenario, admin, pane)
                instrumentation.sendKeyDownUpSync(android.view.KeyEvent.KEYCODE_BACK)
                delay(400) // Wait for IME insets animation and the debounced remote resize.
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                repeat(2) {
                    tapTerminal(scenario)
                    waitForIme(scenario, true)
                    delay(500)
                    withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                    instrumentation.sendKeyDownUpSync(android.view.KeyEvent.KEYCODE_BACK)
                    waitForIme(scenario, false)
                    delay(500)
                    withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                }
                keyboardObserver.cancelAndJoin()
                assertFalse("Keyboard resize must not reconnect", keyboardStates.any {
                    it.phase in setOf(Phase.Connecting, Phase.Authenticating, Phase.Reconnecting, Phase.Failed, Phase.Disconnected)
                })
                assertTrue("Resize restoration keeps the connected label and hides connection progress", keyboardStates.filter { it.resizing }.all {
                    it.message == "연결됨 · tmux" && !it.showsConnectionProgress && !it.acceptsInput
                })
                assertEquals(clientsBeforeKeyboard, admin.execute("tmux list-clients -t '$session' -F '#{client_pid}'").decodeToString().trim())
                scenario.onActivity { assertSame(keyboardView, findWebView(it.window.decorView)) }
                assertEquals(targetBeforeKeyboard, connection.captureInputTarget())
                val acknowledged = connection.sendInput(targetBeforeKeyboard, "printf 'DGL_COMPOSER_ACK\\n'\r".encodeToByteArray())
                assertNull(acknowledged.error)
                withTimeout(5000) {
                    while (!admin.execute("tmux capture-pane -p -t '$pane'").decodeToString().contains("DGL_COMPOSER_ACK")) delay(50)
                }
                stage = "GUI pane split"
                val originalPaneIds = connection.remote.value.panes.map { it.id }.toSet()
                clickText("tmux 패널 좌우 분할")
                stage = "GUI split remote update"
                val splitPane = withTimeout(10000) { connection.remote.first { state -> state.panes.any { it.id !in originalPaneIds } } }.panes.single { it.id !in originalPaneIds }
                // The list is published before the split's target selection and snapshot finish.
                withTimeout(10000) { connection.pane.first { it == splitPane.id } }
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                assertNotNull(connection.sendInput(targetBeforeKeyboard, "must-not-send".encodeToByteArray()).error)
                admin.execute("tmux " + TmuxCommand.renamePane(splitPane.id, "UI 분할 패널").wire)
                withTimeout(10000) { connection.remote.first { it.panes.any { p -> p.id == splitPane.id && p.title == "UI 분할 패널" } } }
                stage = "GUI tree expansion"
                // Expansion must not change the selected target; selecting a row is separate.
                val beforeExpand = connection.pane.value
                clickText("tmux 관리")
                stage = "session expand"
                clickMatching { it.contentDescription?.toString()?.let { text -> text.startsWith("$name\n") && text.endsWith(" 펼치기") } == true }
                val window = connection.remote.value.windows.first { it.id == splitPane.window && it.session == session }
                assertEquals(beforeExpand, connection.pane.value)
                stage = "window selection"
                clickMatching { it.contentDescription?.toString() == connection.remote.value.windowLabel(window).fullText }
                withTimeout(10000) { connection.pane.first { it == splitPane.id } }
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                clickText("터미널로 돌아가기")
                stage = "focus left"
                clickText("왼쪽 패널로 이동")
                withTimeout(10000) { connection.pane.first { it == pane } }
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                stage = "focus right"
                clickText("오른쪽 패널로 이동")
                withTimeout(10000) { connection.pane.first { it == splitPane.id } }
                withTimeout(10000) { connection.status.first { it.phase == Phase.Ready } }
                clickText("tmux 관리")
                clickText("세션 새로고침")
                withTimeout(10000) { connection.refreshing.first { !it } }
                instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
                    File(instrumentation.targetContext.cacheDir, "rust-tree.png").outputStream().use {
                        bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
                    }
                    bitmap.recycle()
                }
                clickText("터미널로 돌아가기")
                stage = "GUI pane close cancellation"
                clickText("현재 tmux 패널 닫기")
                clickText("취소")
                assertTrue(connection.remote.value.panes.any { it.id == splitPane.id })
                assertEquals(splitPane.id, connection.pane.value)
                stage = "GUI window creation"
                runVolumeMacro(scenario, admin, splitPane.id)
                assertEquals("1", admin.execute("tmux show-options -pv -t '$pane' @dgl-volume-count").decodeToString().trim())
                clickText("tmux 관리")
                stage = "session expand"
                clickMatching { it.contentDescription?.toString()?.let { text -> text.startsWith("$name\n") && text.endsWith(" 메뉴") } == true }
                clickText("창 만들기")
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
                scenario.onActivity { assertFalse(it.volumeKeys.dispatch(android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_VOLUME_UP))) }
                assertTrue(editor.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                stage = "GUI name confirmation"
                clickText("확인")
                stage = "GUI remote window update"
                withTimeout(10000) { connection.remote.first { state -> state.windows.any { it.name == "GUI window" && it.session == session } } }
                clickText("터미널로 돌아가기")
                stage = "font setting and renderer replacement"
                var previousView: WebView? = null
                scenario.onActivity { previousView = findWebView(it.window.decorView) }
                clickText("설정"); clickText("부드러운 터미널 화면 전환"); clickText("닫기")
                assertTrue(app.local.value.preferences.smoothTerminalTransitions)
                scenario.onActivity { assertSame("Animation settings must not replace the WebView", previousView, findWebView(it.window.decorView)) }
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
                stage = "terminal screenshots"
                // New windows now become the shared current window. Seed its own screen;
                // the old test implicitly photographed the previously selected pane's text.
                admin.execute("tmux " + TmuxCommand.send(connection.pane.value!!,
                    "printf 'WINDOW_CAPTURE_1\\nWINDOW_CAPTURE_2\\nWINDOW_CAPTURE_3\\nWINDOW_CAPTURE_4\\n'\r".encodeToByteArray()).wire)
                run {
                    for (theme in listOf("light", "dark")) {
                        scenario.onActivity { app.update(app.local.value.copy(preferences = app.local.value.preferences.copy(theme = theme))) }
                        instrumentation.waitForIdleSync()
                        delay(900) // Await compositing after the theme changes on the software GPU.
                        captureTerminal(scenario, "rust-$theme")
                    }
                    clickText("호스트"); clickText("+ 호스트")
                    withTimeout(5000) { while (findText(instrumentation.uiAutomation.rootInActiveWindow, "SSH 호스트 추가") == null) delay(50) }
                    assertNotNull(findText(instrumentation.uiAutomation.rootInActiveWindow, "저장"))
                    assertNotNull(findText(instrumentation.uiAutomation.rootInActiveWindow, "취소"))
                    instrumentation.waitForIdleSync()
                    delay(350)
                    val hostField = findNode(instrumentation.uiAutomation.rootInActiveWindow) { it.isEditable }!!
                    val hostBounds = android.graphics.Rect(); hostField.getBoundsInScreen(hostBounds)
                    val tapTime = android.os.SystemClock.uptimeMillis()
                    instrumentation.sendPointerSync(android.view.MotionEvent.obtain(tapTime, tapTime, android.view.MotionEvent.ACTION_DOWN, hostBounds.centerX().toFloat(), hostBounds.centerY().toFloat(), 0))
                    instrumentation.sendPointerSync(android.view.MotionEvent.obtain(tapTime, tapTime + 80, android.view.MotionEvent.ACTION_UP, hostBounds.centerX().toFloat(), hostBounds.centerY().toFloat(), 0))
                    var editorImeTop = 0
                    withTimeout(5000) {
                        while (editorImeTop == 0) {
                            scenario.onActivity {
                                android.view.inspector.WindowInspector.getGlobalWindowViews().firstOrNull { view -> view.hasWindowFocus() && view.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true }?.let { view ->
                                    val position = IntArray(2); view.getLocationOnScreen(position)
                                    editorImeTop = position[1] + view.height - view.rootWindowInsets.getInsets(WindowInsets.Type.ime()).bottom
                                }
                            }
                            delay(50)
                        }
                    }
                    delay(250)
                    val saveBounds = android.graphics.Rect()
                    findText(instrumentation.uiAutomation.rootInActiveWindow, "저장")!!.getBoundsInScreen(saveBounds)
                    assertTrue("Host save must stay above IME", saveBounds.bottom <= editorImeTop + 8)
                    instrumentation.uiAutomation.takeScreenshot()?.let { bitmap -> File(instrumentation.targetContext.cacheDir, "rust-editor.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) } }
                    clickText("취소"); clickText("터미널")
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
                throw AssertionError("Android UI timeout: $stage; activePackage=${root?.packageName}; phase=${connection.status.value.phase}; all=${app.local.value.preferences.showAllHierarchy}; notice=${connection.notice.value}; ui=${dumpUi(root)}; windows=${connection.remote.value.windows}; panes=${connection.remote.value.panes}")
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
    private suspend fun captureTerminal(scenario: ActivityScenario<MainActivity>, name: String) {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        withTimeout(10000) {
            while (true) {
                val bounds = android.graphics.Rect()
                scenario.onActivity { findWebView(it.window.decorView)?.getGlobalVisibleRect(bounds) }
                val visual = CompletableDeferred<Unit>()
                scenario.onActivity { findWebView(it.window.decorView)?.postVisualStateCallback(0, object : WebView.VisualStateCallback() {
                    override fun onComplete(requestId: Long) { visual.complete(Unit) }
                }) ?: visual.complete(Unit) }
                withTimeout(5000) { visual.await() }
                val bitmap = instrumentation.uiAutomation.takeScreenshot()
                if (bitmap != null) {
                    var ink = 0; var dark = 0; var samples = 0
                    for (y in (bounds.top + 64).coerceAtLeast(0) until minOf(bounds.top + 400, bounds.bottom - 16, bitmap.height) step 3)
                        for (x in (bounds.left + 24).coerceAtLeast(0) until minOf(bounds.right - 24, bitmap.width) step 3) {
                            val pixel = bitmap.getPixel(x, y); samples++
                            val red = android.graphics.Color.red(pixel); val green = android.graphics.Color.green(pixel); val blue = android.graphics.Color.blue(pixel)
                            if (red > 100 && green > 100 && blue > 100) ink++
                            if (red < 24 && green < 28 && blue < 35) dark++
                        }
                    if (ink > 40 && dark > samples * 0.8) {
                        File(instrumentation.targetContext.cacheDir, "$name.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
                        bitmap.recycle()
                        return@withTimeout
                    }
                    bitmap.recycle()
                }
                delay(100)
            }
        }
    }
    private fun tapTerminal(scenario: ActivityScenario<MainActivity>) {
        var x = 0f; var y = 0f
        scenario.onActivity { activity ->
            val web = findWebView(activity.window.decorView)!!
            val position = IntArray(2); web.getLocationOnScreen(position)
            x = position[0] + web.width / 2f; y = position[1] + web.height / 2f
        }
        val now = android.os.SystemClock.uptimeMillis()
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.sendPointerSync(android.view.MotionEvent.obtain(now, now, android.view.MotionEvent.ACTION_DOWN, x, y, 0))
        instrumentation.sendPointerSync(android.view.MotionEvent.obtain(now, now + 80, android.view.MotionEvent.ACTION_UP, x, y, 0))
    }
    private suspend fun waitForIme(scenario: ActivityScenario<MainActivity>, visible: Boolean) = withTimeout(5000) {
        while (true) {
            var matches = false
            scenario.onActivity { matches = it.window.decorView.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == visible }
            if (matches) break
            delay(20)
        }
    }
    private suspend fun runVolumeMacro(scenario: ActivityScenario<MainActivity>, admin: SshjTransport, pane: String) {
        withTimeout(5000) {
            while (true) {
                var ready = false
                scenario.onActivity { ready = it.volumeKeys.focused && it.volumeKeys.resumed && connection.status.value.acceptsInput }
                if (ready) break
                delay(20)
            }
        }
        val time = android.os.SystemClock.uptimeMillis()
        scenario.onActivity { activity ->
            val target = activity as android.app.Activity
            assertTrue(target.dispatchKeyEvent(android.view.KeyEvent(time, time, android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_VOLUME_UP, 0)))
            repeat(5) { assertTrue(target.dispatchKeyEvent(android.view.KeyEvent(time, time + it + 1, android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_VOLUME_UP, it + 1))) }
            assertTrue(target.dispatchKeyEvent(android.view.KeyEvent(time, time + 10, android.view.KeyEvent.ACTION_UP, android.view.KeyEvent.KEYCODE_VOLUME_UP, 0)))
        }
        withTimeout(5000) {
            while (runCatching { admin.execute("tmux show-options -pv -t '$pane' @dgl-volume-count").decodeToString().trim() }.getOrNull() != "1") delay(50)
        }
        delay(150)
        assertEquals("1", admin.execute("tmux show-options -pv -t '$pane' @dgl-volume-count").decodeToString().trim())
    }
    private fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) for (i in 0 until view.childCount) findWebView(view.getChildAt(i))?.let { return it }
        return null
    }
    private fun dumpUi(node: android.view.accessibility.AccessibilityNodeInfo?): String {
        if (node == null) return ""
        return listOfNotNull(node.text, node.contentDescription).joinToString("|") +
            (0 until node.childCount).joinToString(prefix = "[", postfix = "]") { dumpUi(node.getChild(it)) }
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
                automation.clearCache()
                var node = findNode(automation.rootInActiveWindow, predicate)
                while (node != null && !node.isClickable) node = node.parent
                if (node?.isEnabled == true && node.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK)) break
                delay(50)
            }
        }
    }
}
