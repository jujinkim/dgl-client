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
        override val inputNotice = SnackbarState()
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        override val agents = MutableStateFlow(AgentSnapshot())
        var surface: TerminalSurface? = null
        val writes = java.util.Collections.synchronizedList(mutableListOf<ByteArray>())
        var completion = CompletableDeferred<InputCompletion>()
        var connects = 0
        var closes = 0
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
        override fun close() { closes++ }
    }

    @Test fun compactTabsAndNotificationPopupPreserveInputAndFocus() = runBlocking<Unit> {
        val clients = (1..8).map { Connection("tab-$it") }
        val first = clients.first()
        fun title(client: Connection) = "${client.host.alias} · \$1"
        lateinit var app: AppModel; lateinit var previous: LocalState
        lateinit var workspace: WorkspaceState
        var oldConnections = emptyList<ClientConnection>(); var oldSelected: ClientConnection? = null
        var stage = "setup"
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; workspace = app.workspace.value
                    oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false,
                        volumeKeysEnabled = true, volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.Enter))))) }
                    app.connections.value = clients; app.selected.value = first; app.reconcileWorkspace()
                }
                awaitNode("열린 연결 탭"); checkBar()
                withTimeout(5000) { while (first.surface == null) delay(20) }
                val renderer = first.surface as kim.jujin.dglc.ui.AndroidTerminal
                val density = instrumentation.targetContext.resources.displayMetrics.density
                val tabs = bounds(awaitNode("열린 연결 탭")); val tools = bounds(awaitNode("터미널 입력 도구"))
                screenshot("mobile-chrome-tabs")
                assertTrue(kotlin.math.abs(tabs.height() - 36 * density) <= 2)
                assertEquals(tabs.bottom, tools.top)
                scenario.onActivity {
                    val xy = IntArray(2); renderer.view.getLocationOnScreen(xy)
                    assertEquals(xy[1] + renderer.view.height, tabs.top)
                }
                assertNull(node("서버 선택"))
                listOf("호스트", "tmux 관리", "알림 0", "설정").map { bounds(awaitNode(it)) }.zipWithNext().forEach { (a, b) -> assertTrue(a.right <= b.left) }
                stage = "tab selection and retained renderers"
                scenario.onActivity { app.selectConnection(clients.last()) }
                withTimeout(5000) {
                    while (true) {
                        val last = node(title(clients.last()))
                        if (last != null && generateSequence(last) { it.parent }.any { it.isSelected } && bounds(last).right <= tabs.right) break
                        delay(25)
                    }
                }
                val tabScroll = find(awaitNode("열린 연결 탭")) { it.isScrollable }!!
                withTimeout(5000) {
                    while (node(title(first)) == null) {
                        assertTrue(tabScroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD))
                        delay(100)
                    }
                }
                assertSame(clients.last(), app.selected.value)
                click(title(first))
                withTimeout(5000) { while (first.surface == null) delay(20) }
                assertSame(renderer, first.surface)
                assertEquals(0, clients.sumOf { it.connects })
                stage = "empty popup, keyboard and focus"
                click("키보드 열기"); waitIme(scenario, true)
                scenario.onActivity { assertTrue(renderer.view.hasFocus()) }
                val notificationButton = bounds(awaitNode("알림 0"))
                click("알림 0"); awaitNode("수신한 알림이 없습니다.")
                waitIme(scenario, true)
                val popup = find(instrumentation.uiAutomation.rootInActiveWindow) { it.paneTitle?.toString() == "알림" }!!
                val panel = bounds(popup)
                screenshot("mobile-chrome-popup")
                assertTrue(panel.top >= notificationButton.bottom)
                assertTrue(panel.left >= (8 * density).toInt())
                assertTrue(panel.width() <= (360 * density).toInt())
                scenario.onActivity {
                    val decor = it.window.decorView
                    val imeTop = decor.height - decor.rootWindowInsets.getInsets(WindowInsets.Type.ime()).bottom
                    assertTrue(panel.bottom <= imeTop)
                    renderer.view.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked'}))", null)
                    val time = android.os.SystemClock.uptimeMillis()
                    it.dispatchKeyEvent(KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_VOLUME_UP, 0))
                    it.dispatchKeyEvent(KeyEvent(time, time + 1, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_VOLUME_UP, 0))
                }
                delay(100); assertTrue(first.writes.isEmpty()); assertTrue(app.inputBlocked())
                click("닫기")
                withTimeout(5000) { while (app.inputBlocked()) delay(20) }
                scenario.onActivity { assertTrue(renderer.view.hasFocus()) }
                waitIme(scenario, true)
                click("키보드 닫기"); waitIme(scenario, false)
                stage = "outside tap is consumed"
                val composer = bounds(awaitNode("텍스트 작성 열기"))
                click("알림 0"); awaitNode("수신한 알림이 없습니다.")
                tap(composer.centerX().toFloat(), composer.centerY().toFloat())
                withTimeout(5000) { while (app.inputBlocked()) delay(20) }
                assertFalse(app.composer(first).opened.value)
                click("알림 0"); awaitNode("수신한 알림이 없습니다.")
                instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                withTimeout(5000) { while (app.inputBlocked()) delay(20) }
                stage = "long list and notice navigation"
                val events = (1..30).map { n -> AgentEvent("mobile-chrome-$n", n.toLong(), "chrome-server", "chrome-run", "parent", "\$1", "@1", "%1",
                    "Claude", "Question", "RawEventMustStayHidden", System.currentTimeMillis()) }
                scenario.onActivity { first.agents.value = AgentSnapshot(server = "chrome-server", valid = true, observed_at = System.currentTimeMillis(), events = events,
                    observations = listOf(AgentObservation("\$1", "@1", "%1", "chrome-run", "parent", "Claude", events.last()))) }
                withTimeout(5000) { while (app.agentNotifications.value.size < 30) delay(20) }
                click("알림 30"); awaitNode("최근 알림 · 30 읽지 않음")
                val scroll = find(instrumentation.uiAutomation.rootInActiveWindow) { it.isScrollable }!!
                assertTrue(bounds(scroll).height() < (480 * density).toInt())
                val header = bounds(awaitNode("최근 알림 · 30 읽지 않음"))
                assertTrue(scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)); delay(100)
                assertEquals(header.top, bounds(awaitNode("최근 알림 · 30 읽지 않음")).top)
                assertNull(node("RawEventMustStayHidden"))
                click("? 질문이 발생했습니다")
                withTimeout(5000) { while (app.agentNavigation.value == 0L || app.agentNotifications.value.none { it.read }) delay(20) }
                assertNull(app.agentNavigationError.value)
                assertTrue(first.writes.isEmpty())
                stage = "closing tabs uses existing removal"
                scenario.onActivity { clients.drop(1).forEach(app::removeConnection) }
                awaitNode(title(first))
                click("${title(first)} 닫기")
                awaitNode("+ 호스트")
                assertTrue(app.connections.value.isEmpty())
                assertEquals(1, first.closes)
                assertTrue(generateSequence(awaitNode("tmux 관리")) { it.parent }.any { !it.isEnabled })
            } catch (failure: Throwable) {
                screenshot("mobile-chrome-failure")
                fun dump(root: AccessibilityNodeInfo?): String = if (root == null) "" else
                    "[${root.text}|${root.contentDescription}|selected=${root.isSelected}|${bounds(root)}]" +
                        (0 until root.childCount).joinToString { dump(root.getChild(it)) }
                throw AssertionError("Mobile chrome stage: $stage; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), failure)
            } finally {
                scenario.onActivity {
                    clients.filter { client -> client in app.connections.value }.forEach(app::removeConnection)
                    app.connections.value = oldConnections; app.selected.value = oldSelected
                    app.changeWorkspace { WorkspaceState(workspace.root) }; app.selected.value = oldSelected; app.reconcileWorkspace(); app.update(previous)
                }
            }
        }
    }

    private fun tap(x: Float, y: Float) {
        val time = android.os.SystemClock.uptimeMillis()
        for (action in listOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP)) {
            val event = MotionEvent.obtain(time, android.os.SystemClock.uptimeMillis(), action, x, y, 0)
            event.source = InputDevice.SOURCE_TOUCHSCREEN
            try { assertTrue(instrumentation.uiAutomation.injectInputEvent(event, true)) } finally { event.recycle() }
        }
        instrumentation.waitForIdleSync()
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
                clickToolbarAction("패널 도구 접기"); assertToolbarAction("패널 도구 펼치기")
                assertFalse(app.local.value.preferences.mobilePaneToolbarVisible)
                assertFalse(app.repository.read().preferences.mobilePaneToolbarVisible)
                val restarted = createEngine(instrumentation.targetContext.applicationContext as android.app.Application)
                try { assertFalse(restarted.local.value.preferences.mobilePaneToolbarVisible) } finally { restarted.close() }
                scenario.onActivity { app.selected.value = second }
                assertToolbarAction("패널 도구 펼치기")
                clickToolbarAction("패널 도구 펼치기"); assertToolbarAction("패널 도구 접기")
                scenario.onActivity { app.selected.value = first }
                assertToolbarAction("패널 도구 접기")
                scenario.recreate()
                assertToolbarAction("패널 도구 접기")
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
                // Let the native IME finish its selection update after committing composition.
                instrumentation.waitForIdleSync()
                instrumentation.uiAutomation.waitForIdle(300, 3000)
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
        near(keyboard.height(), 30); near(menu.height(), 30); near(bar.height(), 36)
        near(keyboard.left - bar.left, 4); near(keyboard.top - bar.top, 3)
        near(bar.bottom - keyboard.bottom, 3); near(bar.right - menu.right, 4)
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
    private suspend fun openConnectionMenu() {
        withTimeout(5000) {
            while (true) {
                instrumentation.uiAutomation.clearCache()
                val label = find(instrumentation.uiAutomation.rootInActiveWindow) {
                    it.contentDescription?.toString()?.let { text -> text.startsWith("현재 호스트 ") && text.endsWith(" 연결 메뉴") } == true
                }?.contentDescription?.toString()
                if (label != null) { click(label); return@withTimeout }
                delay(25)
            }
        }
    }
    private suspend fun clickToolbarAction(label: String) { openConnectionMenu(); click(label) }
    private suspend fun assertToolbarAction(label: String) {
        openConnectionMenu(); awaitNode(label)
        instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
        instrumentation.waitForIdleSync()
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
