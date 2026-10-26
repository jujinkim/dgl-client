package kim.jujin.dglc

import android.content.pm.ActivityInfo
import android.graphics.Rect
import android.view.*
import android.view.accessibility.AccessibilityNodeInfo
import android.view.inputmethod.EditorInfo
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.*
import org.junit.Test
import java.io.File

/** Real Compose/WebView/IME; deterministic connection so failures and pending sends are reproducible. */
class AndroidMobileInputTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private class Connection(id: String) : ClientConnection {
        override val host = HostProfile(id, "$id · 아주 긴 서버 이름 · 한글과 emoji 😀 production", "example.test", username = "test")
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready, "연결됨 · tmux"))
        override val remote = MutableStateFlow(RemoteState())
        override val session = MutableStateFlow<String?>("\$1")
        override val pane = MutableStateFlow<String?>("%1")
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        var surface: TerminalSurface? = null
        val writes = java.util.Collections.synchronizedList(mutableListOf<ByteArray>())
        var completion = CompletableDeferred<InputCompletion>()
        var connects = 0
        override fun captureInputTarget() = InputTarget(1, session.value, pane.value!!)
        override suspend fun sendInput(target: InputTarget, bytes: ByteArray): InputCompletion {
            assertEquals(captureInputTarget(), target); writes += bytes; return completion.await()
        }
        override fun input(bytes: ByteArray) { writes += bytes }
        override fun bindSurface(value: TerminalSurface?) { surface = value }
        override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) surface = null }
        override fun connect() { connects++ }
        override fun trust() {}
        override fun selectTarget(session: String, window: String?, pane: String?) {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun disconnect() {}
        override fun close() {}
    }

    @Test fun mobileLayoutImeComposerAndSharedPreference() = runBlocking<Unit> {
        val first = Connection("first"); val second = Connection("second")
        lateinit var app: AppModel
        lateinit var previous: LocalState
        var oldConnections = emptyList<ClientConnection>()
        var oldSelected: ClientConnection? = null
        var stage = "launch"
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", theme = "light", mobilePaneToolbarVisible = true,
                        volumeKeysEnabled = true, volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.Enter))))) }
                    app.connections.value = listOf(first, second); app.selected.value = first
                    it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                }
                waitOrientation(scenario, false)
                awaitNode("텍스트 작성 열기")
                withTimeout(10000) { while (first.surface == null) delay(20) }
                first.surface!!.write("MOBILE INPUT\r\n한글 😀 여러 줄 작성".encodeToByteArray())
                stage = "toolbar preference"
                click("패널 도구 접기"); awaitNode("패널 도구 펼치기")
                assertFalse(app.local.value.preferences.mobilePaneToolbarVisible)
                assertFalse(app.repository.read().preferences.mobilePaneToolbarVisible)
                val restarted = createEngine(instrumentation.targetContext.applicationContext as android.app.Application)
                try { assertFalse(restarted.local.value.preferences.mobilePaneToolbarVisible) } finally { restarted.close() }
                scenario.onActivity { app.selected.value = second }
                awaitNode("패널 도구 펼치기")
                click("패널 도구 펼치기"); awaitNode("패널 도구 접기")
                scenario.onActivity { app.selected.value = first }
                awaitNode("패널 도구 접기")
                scenario.recreate()
                awaitNode("패널 도구 접기")
                scenario.onActivity { assertSame(app, (it.application as DglApplication).connections().app) }
                stage = "fixed geometry"
                delay(300)
                checkBar()
                val fixedBeforeScroll = bounds(awaitNode("텍스트 작성 열기"))
                val shortcuts = find(awaitNode("터미널 입력 도구")) { it.isScrollable }
                assertNotNull(shortcuts)
                assertTrue(shortcuts!!.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))
                delay(200)
                assertEquals(fixedBeforeScroll, bounds(awaitNode("텍스트 작성 열기")))
                checkBar()
                var web: WebView? = null
                scenario.onActivity { web = webView(it.window.decorView) }
                repeat(3) {
                    stage = "keyboard open $it"
                    click("키보드 열기"); stage = "keyboard opened click $it"; waitIme(scenario, true); awaitNode("키보드 닫기")
                    var imeTop = 0
                    scenario.onActivity { activity -> imeTop = activity.window.decorView.height - activity.window.decorView.rootWindowInsets.getInsets(WindowInsets.Type.ime()).bottom }
                    assertTrue(bounds(awaitNode("CLI 실행 메뉴")).bottom <= imeTop + 2)
                    stage = "keyboard close $it"
                    click("키보드 닫기"); stage = "keyboard closed click $it"; waitIme(scenario, false)
                    scenario.onActivity { assertSame(web, webView(it.window.decorView)) }
                }
                screenshot("mobile-light-portrait")
                stage = "landscape and hidden shortcuts"
                scenario.onActivity { activity ->
                    app.modify { it.copy(preferences = it.preferences.copy(theme = "dark", terminalShortcuts = emptyList())) }
                    activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                }
                waitOrientation(scenario, true)
                awaitNode("텍스트 작성 열기"); checkBar()
                screenshot("mobile-dark-landscape")
                scenario.onActivity { assertSame(web, webView(it.window.decorView)); it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT }
                waitOrientation(scenario, false)
                stage = "native composition"
                click("텍스트 작성 열기"); awaitNode("전송할 텍스트")
                withTimeout(5000) {
                    var committed = false
                    while (!committed) {
                        instrumentation.runOnMainSync {
                            val focus = android.view.inspector.WindowInspector.getGlobalWindowViews().lastOrNull { it.hasWindowFocus() }?.findFocus()
                            val editor = EditorInfo()
                            val input = focus?.onCreateInputConnection(editor)
                            if (input != null && editor.inputType != 0) {
                                input.setComposingText("ㅎ", 1); input.setComposingText("하", 1)
                                input.setComposingText("한글", 1); input.commitText("한글😀\n두 줄", 1)
                                input.finishComposingText(); committed = true
                            }
                        }
                        if (!committed) delay(50)
                    }
                }
                withTimeout(5000) { while (app.composer(first).value.value.text != "한글😀\n두 줄") delay(20) }
                assertTrue(first.writes.isEmpty())
                stage = "selection copy paste"
                selectText(0, 2)
                withTimeout(5000) { while (app.composer(first).value.value.selection.start != 0 || app.composer(first).value.value.selection.end != 2) delay(25) }
                val editor = awaitAction("전송할 텍스트", AccessibilityNodeInfo.ACTION_COPY)
                assertTrue(editor.performAction(AccessibilityNodeInfo.ACTION_COPY))
                selectText(8, 8)
                withTimeout(5000) { while (app.composer(first).value.value.selection.start != 8 || app.composer(first).value.value.selection.end != 8) delay(25) }
                assertTrue(awaitAction("전송할 텍스트", AccessibilityNodeInfo.ACTION_PASTE).performAction(AccessibilityNodeInfo.ACTION_PASTE))
                withTimeout(5000) { while (app.composer(first).value.value.text != "한글😀\n두 줄한글") delay(20) }
                screenshot("mobile-composer-portrait")
                val draft = app.composer(first).value.value.text
                stage = "dialog background guard"
                val count = first.writes.size
                scenario.onActivity { activity ->
                    val time = android.os.SystemClock.uptimeMillis()
                    activity.dispatchKeyEvent(KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_VOLUME_UP, 0))
                    activity.dispatchKeyEvent(KeyEvent(time, time + 1, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_VOLUME_UP, 0))
                    webView(activity.window.decorView)!!.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked'}))", null)
                }
                delay(100); assertEquals(count, first.writes.size)
                stage = "draft rotation and tab restoration"
                scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE }
                waitOrientation(scenario, true)
                awaitNode("전송할 텍스트"); assertEquals(draft, app.composer(first).value.value.text)
                waitDialogIme()
                assertTrue(awaitNode("Enter 함께 보내기").isVisibleToUser)
                var dialogImeTop = Int.MAX_VALUE
                instrumentation.runOnMainSync {
                    android.view.inspector.WindowInspector.getGlobalWindowViews().lastOrNull { it.hasWindowFocus() }?.let { root ->
                        val position = IntArray(2); root.getLocationOnScreen(position)
                        dialogImeTop = position[1] + root.height - (root.rootWindowInsets?.getInsets(WindowInsets.Type.ime())?.bottom ?: 0)
                    }
                }
                assertTrue("Send must remain above the landscape IME", bounds(awaitNode("전송")).bottom <= dialogImeTop + 2)
                assertTrue("Enter option must remain above the landscape IME", bounds(awaitNode("Enter 함께 보내기")).bottom <= dialogImeTop + 2)
                screenshot("mobile-composer-landscape")
                click("취소"); awaitNode("텍스트 작성 열기")
                scenario.onActivity { app.selected.value = second }
                click("텍스트 작성 열기"); awaitNode("전송할 텍스트")
                assertTrue(app.composer(second).value.value.text.isEmpty())
                click("취소")
                scenario.onActivity { app.selected.value = first; it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT }
                waitOrientation(scenario, false)
                click("텍스트 작성 열기"); awaitNode("전송할 텍스트")
                assertEquals(draft, app.composer(first).value.value.text)
                stage = "target rejection"
                first.pane.value = "%2"
                click("전송")
                withTimeout(5000) { while (app.composer(first).error.value == null) delay(20) }
                assertEquals(count, first.writes.size)
                stage = "reopen after target rejection"
                click("취소"); click("텍스트 작성 열기"); awaitNode("전송할 텍스트")
                stage = "completion failure and duplicate guard"
                // The label is intentionally a separate checkbox row; find its native checkable node.
                val check = find(instrumentation.uiAutomation.rootInActiveWindow) { it.isCheckable }!!
                assertFalse(check.isChecked); assertTrue(check.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                click("전송")
                withTimeout(5000) { while (first.writes.size == count) delay(20) }
                assertEquals(draft.replace('\n', '\r') + "\r", first.writes.last().decodeToString())
                assertTrue(app.composer(first).sending.value)
                val pending = awaitNode("전송 중…")
                pending.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                delay(100); assertEquals(count + 1, first.writes.size)
                first.completion.complete(InputCompletion("합성 전송 실패"))
                withTimeout(5000) { while (app.composer(first).sending.value) delay(20) }
                assertEquals(draft, app.composer(first).value.value.text)
                first.completion = CompletableDeferred()
                click("전송")
                withTimeout(5000) { while (first.writes.size < count + 2) delay(20) }
                first.completion.complete(InputCompletion())
                awaitNode("텍스트 작성 열기")
                assertTrue(app.composer(first).value.value.text.isEmpty())
                assertEquals(0, first.connects)
                stage = "memory cleanup"
                val removed = app.composer(second)
                scenario.onActivity { app.removeConnection(second) }
                assertFalse(removed.opened.value)
                assertTrue(removed.value.value.text.isEmpty())
            } catch (failure: Throwable) {
                val active = instrumentation.uiAutomation.rootInActiveWindow
                println("DGL mobile failure: stage=$stage, root=${active?.packageName}, open=${node("키보드 열기") != null}, close=${node("키보드 닫기") != null}")
                node("전송할 텍스트")?.let { println("DGL editor: class=${it.className}, editable=${it.isEditable}, selection=${it.textSelectionStart}..${it.textSelectionEnd}, actions=${it.actionList.map { a -> a.id }}, draftSelection=${app.composer(first).value.value.selection}") }
                scenario.onActivity { activity -> println("DGL insets: decor=${activity.window.decorView.rootWindowInsets?.isVisible(WindowInsets.Type.ime())}, web=${webView(activity.window.decorView)?.rootWindowInsets?.isVisible(WindowInsets.Type.ime())}, focus=${webView(activity.window.decorView)?.hasFocus()}") }
                screenshot("mobile-failure")
                throw AssertionError("Mobile input stage: $stage", failure)
            } finally {
                scenario.onActivity {
                    app.removeConnection(first); app.removeConnection(second)
                    app.connections.value = oldConnections; app.selected.value = oldSelected; app.update(previous)
                    it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                }
            }
        }
    }

    private suspend fun checkBar() {
        val bar = bounds(awaitNode("터미널 입력 도구"))
        val keyboard = bounds(awaitNode(if (node("키보드 닫기") != null) "키보드 닫기" else "키보드 열기"))
        val text = bounds(awaitNode("텍스트 작성 열기")); val menu = bounds(awaitNode("CLI 실행 메뉴"))
        val density = instrumentation.targetContext.resources.displayMetrics.density
        fun near(actual: Int, dp: Int) = assertTrue("$actual px != $dp dp", kotlin.math.abs(actual - dp * density) <= 2)
        near(keyboard.height(), 40); near(menu.height(), 40)
        near(keyboard.left - bar.left, 4); near(keyboard.top - bar.top, 4)
        near(bar.bottom - keyboard.bottom, 4); near(bar.right - menu.right, 4)
        near(text.left - keyboard.right, 4)
        assertTrue(text.right < menu.left)
    }
    private fun screenshot(name: String) {
        instrumentation.waitForIdleSync()
        Thread.sleep(250) // Allow the committed Compose frame and IME animation to reach SurfaceFlinger.
        instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
            File(instrumentation.targetContext.cacheDir, "$name.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
            bitmap.recycle()
        }
    }
    private fun bounds(node: AccessibilityNodeInfo) = Rect().also(node::getBoundsInScreen)
    private fun webView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) for (i in 0 until view.childCount) webView(view.getChildAt(i))?.let { return it }
        return null
    }
    private fun find(node: AccessibilityNodeInfo?, predicate: (AccessibilityNodeInfo) -> Boolean): AccessibilityNodeInfo? {
        if (node == null) return null
        if (predicate(node)) return node
        for (i in 0 until node.childCount) find(node.getChild(i), predicate)?.let { return it }
        return null
    }
    private fun node(text: String): AccessibilityNodeInfo? {
        instrumentation.uiAutomation.clearCache()
        return find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.toString() == text || it.contentDescription?.toString() == text }
    }
    private suspend fun awaitNode(text: String): AccessibilityNodeInfo = withTimeout(5000) {
        while (true) { node(text)?.let { return@withTimeout it }; delay(25) }
        error("unreachable")
    }
    private suspend fun awaitAction(text: String, action: Int): AccessibilityNodeInfo = withTimeout(5000) {
        while (true) {
            val labelled = node(text)
            val editor = find(labelled) { it.isEditable }
                ?: find(instrumentation.uiAutomation.rootInActiveWindow) { it.isEditable }
            editor?.takeIf { it.actionList.any { candidate -> candidate.id == action } }?.let { return@withTimeout it }
            delay(25)
        }
        error("unreachable")
    }
    private suspend fun selectText(start: Int, end: Int) {
        val arguments = android.os.Bundle().apply {
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start)
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end)
        }
        // Native IME commits reach the model before the accessibility text snapshot.
        // Require the real selection action to succeed on a refreshed node.
        withTimeout(5000) {
            while (!awaitAction("전송할 텍스트", AccessibilityNodeInfo.ACTION_SET_SELECTION)
                    .performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, arguments)) delay(25)
        }
        instrumentation.waitForIdleSync()
    }
    private suspend fun click(text: String) = withTimeout(5000) {
        while (true) {
            var target = node(text)
            while (target != null && !target.isClickable) target = target.parent
            if (target?.isEnabled == true && target.performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                instrumentation.waitForIdleSync()
                instrumentation.uiAutomation.waitForIdle(100, 2000)
                return@withTimeout
            }
            delay(25)
        }
    }
    private suspend fun waitIme(scenario: ActivityScenario<MainActivity>, visible: Boolean) = withTimeout(5000) {
        while (true) {
            var matches = false
            scenario.onActivity { matches = it.window.decorView.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == visible }
            if (matches) return@withTimeout
            delay(25)
        }
    }
    private suspend fun waitDialogIme() = withTimeout(5000) {
        // Rotation finishes before the dialog's IME hide/show and inset animation.
        // Sample both window insets and accessibility bounds only after they settle.
        var previous: List<Int>? = null
        var stable = 0
        while (true) {
            var viewport: List<Int>? = null
            instrumentation.runOnMainSync {
                android.view.inspector.WindowInspector.getGlobalWindowViews().lastOrNull { it.hasWindowFocus() }?.let { root ->
                    val insets = root.rootWindowInsets
                    if (insets?.isVisible(WindowInsets.Type.ime()) == true) {
                        val position = IntArray(2); root.getLocationOnScreen(position)
                        viewport = listOf(root.width, root.height, position[1], insets.getInsets(WindowInsets.Type.ime()).bottom)
                    }
                }
            }
            val send = node("전송")?.let(::bounds)
            val option = node("Enter 함께 보내기")?.let(::bounds)
            val sample = viewport?.takeIf { send != null && option != null }?.plus(listOf(send!!.top, send.bottom, option!!.bottom))
            stable = if (sample != null && sample == previous) stable + 1 else 0
            if (stable >= 5) return@withTimeout
            previous = sample
            delay(100)
        }
    }
    private suspend fun waitOrientation(scenario: ActivityScenario<MainActivity>, landscape: Boolean) = withTimeout(5000) {
        while (true) {
            var matches = false
            scenario.onActivity {
                val orientation = it.resources.configuration.orientation
                matches = (orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE) == landscape &&
                    (it.window.decorView.width > it.window.decorView.height) == landscape
            }
            if (matches) break
            delay(25)
        }
        instrumentation.uiAutomation.waitForIdle(200, 3000)
    }
}
