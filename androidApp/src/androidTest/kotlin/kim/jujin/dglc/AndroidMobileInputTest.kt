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
        override fun encodeKeyboard(key: String, text: String?, modifiers: TerminalModifiers, applicationCursor: Boolean): ByteArray? =
            TerminalKey.entries.firstOrNull { it.semanticInput() == (key to modifiers) }?.bytes()
        override suspend fun sendInput(target: InputTarget, bytes: ByteArray): InputCompletion {
            assertEquals(captureInputTarget(), target); writes += bytes; return completion.await()
        }
        override fun input(bytes: ByteArray) { writes += bytes }
        override fun bindSurface(value: TerminalSurface?) { surface = value }
        override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) surface = null }
        override fun connect() { connects++ }
        override fun retryNow() {
            if (!status.value.retryExhausted) return
            connects++
            status.value = ConnectionStatus(Phase.Reconnecting, canStopRetrying = true)
        }
        override fun trust() {}
        override fun selectTarget(session: String, window: String?, pane: String?) {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        val sizes = java.util.Collections.synchronizedList(mutableListOf<Size>())
        override fun resize(value: Size) { sizes += value }
        override fun restore() {}
        override fun disconnect() {}
        override fun close() { closes++ }
    }

    @Suppress("DEPRECATION")
    @Test fun alwaysVisibleKeyboardRespectsEditorsLifecycleAndActiveTerminal() = runBlocking<Unit> { withLauncherDisplay(animateIme = true) {
        val first = Connection("always-first"); val second = Connection("always-second")
        lateinit var app: AppModel; lateinit var previous: LocalState; lateinit var workspace: WorkspaceState
        var oldConnections = emptyList<ClientConnection>(); var oldSelected: ClientConnection? = null
        var stage by kotlin.properties.Delegates.observable("setup") { _, _, next -> android.util.Log.i("DglKeyboardTest", next) }
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            suspend fun eventually(check: (MainActivity) -> Boolean) = withTimeout(5000) {
                while (true) {
                    var done = false; scenario.onActivity { done = check(it) }
                    if (done) return@withTimeout
                    delay(25)
                }
            }
            suspend fun js(renderer: kim.jujin.dglc.ui.AndroidTerminal, script: String): String {
                val reply = CompletableDeferred<String>()
                scenario.onActivity { renderer.view.evaluateJavascript(script) { reply.complete(it) } }
                return withTimeout(5000) { reply.await() }
            }
            suspend fun scrollTo(label: String) = withTimeout(5000) {
                while (node(label)?.isVisibleToUser != true) {
                    find(instrumentation.uiAutomation.rootInActiveWindow) { it.isScrollable }!!
                        .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)
                    delay(100)
                }
            }
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; workspace = app.workspace.value
                    oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", androidKeyboardMode = "pan",
                        androidKeyboardAlwaysVisible = false, mobilePaneToolbarVisible = false)) }
                    app.workspace.value = WorkspaceState()
                    app.connections.value = listOf(first, second); app.selected.value = first; app.reconcileWorkspace()
                }
                eventually { first.surface != null }
                val renderer = first.surface as kim.jujin.dglc.ui.AndroidTerminal
                stage = "enable in settings"
                openConnectionMenu(); click("설정"); scrollTo("키보드 항상 보이기"); click("키보드 항상 보이기")
                scrollTo("리사이즈")
                for (label in listOf("자동", "위로 이동", "리사이즈")) {
                    val chip = generateSequence(awaitNode(label)) { it.parent }.first { it.isClickable }
                    assertFalse("$label must be disabled", chip.isEnabled)
                    assertEquals("$label selection: $chip", label == "리사이즈", chip.isSelected || chip.isChecked)
                }
                assertEquals("pan", app.local.value.preferences.androidKeyboardMode)
                click("닫기"); waitIme(scenario, true)
                assertFalse(awaitNode("키보드 항상 보이기").isEnabled)
                stage = "DOM focus loss and system dismissal"
                js(renderer, "document.activeElement.blur()")
                withTimeout(5000) {
                    while (js(renderer, "document.activeElement.classList.contains('xterm-helper-textarea')") != "true") delay(25)
                }
                instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                delay(500); waitIme(scenario, true)
                stage = "recovery cannot send input"
                scenario.onActivity { first.status.value = ConnectionStatus(Phase.Reconnecting, canStopRetrying = true) }
                delay(100)
                js(renderer, "window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked-recovery'}))")
                assertTrue(first.writes.isEmpty()); waitIme(scenario, true)
                scenario.onActivity { first.status.value = ConnectionStatus(Phase.Ready) }
                stage = "composer retains its editor"
                click("텍스트 작성 열기"); awaitNode("전송할 텍스트"); waitDialogIme()
                scenario.onActivity { assertFalse(renderer.view.hasWindowFocus()) }
                assertTrue(first.writes.isEmpty())
                click("취소"); waitIme(scenario, true)
                stage = "pause and resume"
                scenario.moveToState(androidx.lifecycle.Lifecycle.State.STARTED)
                scenario.onActivity { it.window.insetsController?.hide(WindowInsets.Type.ime()) }
                waitIme(scenario, false); delay(300); waitIme(scenario, false)
                scenario.moveToState(androidx.lifecycle.Lifecycle.State.RESUMED); waitIme(scenario, true)
                stage = "active split terminal"
                instrumentation.uiAutomation.executeShellCommand("wm size 1200x1000").close()
                awaitNode("분할 작업 공간"); click("앱 화면 좌우 분할")
                eventually { app.workspace.value.groups.size == 2 }
                scenario.onActivity {
                    val group = app.workspace.value.activeGroup
                    app.changeWorkspace { it.move(app.tabId(second)!!, group) }
                }
                eventually { first.surface != null && second.surface != null }
                val secondRenderer = second.surface as kim.jujin.dglc.ui.AndroidTerminal
                eventually { app.selected.value === second && secondRenderer.view.hasFocus() && !renderer.view.hasFocus() }
                waitIme(scenario, true)
                scenario.onActivity { app.selectConnection(first) }
                eventually { renderer.view.hasFocus() && !secondRenderer.view.hasFocus() }
                stage = "disable restores mode and manual hiding"
                click("설정"); scrollTo("키보드 항상 보이기"); click("키보드 항상 보이기"); scrollTo("위로 이동")
                val pan = generateSequence(awaitNode("위로 이동")) { it.parent }.first { it.isClickable }
                assertTrue(pan.isEnabled); assertTrue(pan.isSelected || pan.isChecked)
                click("닫기")
                stage = "manual keyboard after disabling in settings"
                // Finish the modal window's IME-control handoff before preparing the manual-hide check.
                eventually { !app.inputBlocked() && renderer.view.hasWindowFocus() }
                delay(350)
                scenario.onActivity { renderer.keyboard(true) }
                waitIme(scenario, true)
                stage = "disabling on the terminal does not hide the keyboard"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(androidKeyboardAlwaysVisible = true)) } }
                awaitNode("키보드 항상 보이기")
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(androidKeyboardAlwaysVisible = false)) } }
                awaitNode("키보드 닫기"); delay(300); waitIme(scenario, true)
                stage = "manual hiding no longer reopens the keyboard"
                click("키보드 닫기"); waitIme(scenario, false); delay(300); waitIme(scenario, false)
                assertTrue(first.writes.isEmpty()); assertTrue(second.writes.isEmpty())
            } catch (failure: Throwable) {
                throw AssertionError("Always-visible keyboard stage: $stage", failure)
            } finally {
                // Preserve the original failure even if a broken Back handler destroyed the Activity.
                instrumentation.runOnMainSync {
                    app.connections.value = oldConnections; app.selected.value = oldSelected
                    app.workspace.value = workspace; app.modify { previous }
                }
            }
        }
    } }

    @Test fun exhaustedRetryIsCenteredAndRetainsTheTerminal() = runBlocking<Unit> {
        val client = Connection("retry")
        lateinit var app: AppModel
        lateinit var previous: LocalState
        lateinit var workspace: WorkspaceState
        var oldConnections = emptyList<ClientConnection>()
        var oldSelected: ClientConnection? = null
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; workspace = app.workspace.value
                    oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false)) }
                    app.connections.value = listOf(client); app.selected.value = client; app.reconcileWorkspace()
                }
                withTimeout(5000) { while (client.surface == null) delay(20) }
                val surface = client.surface
                scenario.onActivity {
                    client.status.value = ConnectionStatus(Phase.Failed, canRetryNow = true, retryExhausted = true)
                }
                val button = bounds(awaitNode("다시 재접속 시도"))
                awaitNode("재접속 실패")
                assertNull(node("새로고침 재시도"))
                scenario.onActivity {
                    val web = (surface as kim.jujin.dglc.ui.AndroidTerminal).view
                    val area = Rect(); assertTrue(web.getGlobalVisibleRect(area))
                    assertTrue(area.contains(button))
                    assertTrue(kotlin.math.abs(button.centerX() - area.centerX()) <= 2)
                    assertTrue(kotlin.math.abs(button.centerY() - area.centerY()) < 64 * it.resources.displayMetrics.density)
                }
                click("다시 재접속 시도")
                withTimeout(5000) { while (client.connects != 1) delay(20) }
                assertSame(surface, client.surface)
                assertEquals(1, app.connections.value.size)
                assertTrue(client.writes.isEmpty())
            } finally {
                scenario.onActivity {
                    app.connections.value = oldConnections; app.selected.value = oldSelected
                    app.workspace.value = workspace; app.modify { previous }
                }
            }
        }
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
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", theme = "light", mobilePaneToolbarVisible = false,
                        mobileShortcutBarTwoRows = true,
                        volumeKeysEnabled = true, volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.Enter))))) }
                    app.connections.value = clients; app.selected.value = first; app.reconcileWorkspace()
                }
                awaitNode("열린 연결 탭"); checkBar()
                withTimeout(5000) { while (first.surface == null) delay(20) }
                val renderer = first.surface as kim.jujin.dglc.ui.AndroidTerminal
                val density = instrumentation.targetContext.resources.displayMetrics.density
                val tabs = bounds(awaitNode("열린 연결 탭")); val tools = bounds(awaitNode("터미널 입력 도구"))
                assertTrue(kotlin.math.abs(tabs.height() - 36 * density) <= 2)
                assertEquals(tabs.bottom, tools.top)
                scenario.onActivity {
                    val xy = IntArray(2); renderer.view.getLocationOnScreen(xy)
                    assertEquals(xy[1] + renderer.view.height, tabs.top)
                }
                assertNull(node("서버 선택"))
                val menuLabel = "현재 호스트 ${first.host.alias} 연결 메뉴"
                val topRow = listOf("DaGaLi", "호스트", menuLabel, "알림 0").map { bounds(awaitNode(it)) }
                topRow.zipWithNext().forEach { (a, b) -> assertTrue(a.right <= b.left) }
                val currentHost = bounds(awaitNode("현재 호스트: ${first.host.alias}"))
                val tmux = bounds(awaitNode("tmux 관리"))
                assertTrue(currentHost.right <= tmux.left)
                assertTrue(topRow.maxOf { it.bottom } <= tmux.top)
                assertEquals(topRow.last().right, tmux.right)
                assertNull(node("설정"))
                click(menuLabel); awaitNode("설정")
                assertTrue(app.inputBlocked())
                click("설정"); awaitNode("닫기")
                assertTrue(app.inputBlocked())
                click("닫기")
                withTimeout(5000) { while (app.inputBlocked()) delay(20) }
                stage = "tab selection and retained renderers"
                scenario.onActivity { app.selectConnection(clients.last()) }
                awaitNode("현재 호스트: ${clients.last().host.alias}")
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
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(language = "en", theme = "dark")) } }
                awaitNode("Current host: ${first.host.alias}")
                screenshot("mobile-chrome-dark")
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(language = "ko", theme = "light")) } }
                awaitNode("현재 호스트: ${first.host.alias}")
                screenshot("mobile-chrome-tabs")
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
                click("질문이 발생했습니다")
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
                awaitNode("현재 호스트: —")
                assertTrue(generateSequence(awaitNode("터미널")) { it.parent }.any { !it.isEnabled })
                click("기타 메뉴"); awaitNode("설정")
                assertNull(node("재접속")); assertNull(node("연결 종료"))
                click("설정"); awaitNode("닫기"); click("닫기")
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
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", theme = "light", mobilePaneToolbarVisible = true, androidKeyboardMode = "pan",
                        mobileShortcutBarTwoRows = true, terminalShortcuts = TerminalShortcut.defaultShortcuts,
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
                stage = "shortest row placement, independent widths and start alignment"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(
                    terminalShortcuts = listOf(TerminalShortcut.ShiftTab, TerminalShortcut.Ctrl, TerminalShortcut.Enter))) } }
                instrumentation.waitForIdleSync(); checkBar()
                val top = shortcutBounds(TerminalShortcut.ShiftTab); val bottom = shortcutBounds(TerminalShortcut.Ctrl)
                val last = shortcutBounds(TerminalShortcut.Enter)
                val keyboard = bounds(awaitNode("키보드 열기")); val composer = bounds(awaitNode("텍스트 작성 열기"))
                val density = instrumentation.targetContext.resources.displayMetrics.density
                assertEquals(top.left, bottom.left)
                assertTrue("Short lower key must keep its own width", top.width() > bottom.width())
                assertEquals(keyboard.top, top.top); assertEquals(composer.top, bottom.top)
                assertTrue(kotlin.math.abs(top.left - keyboard.right - 4 * density) <= 2)
                assertTrue(kotlin.math.abs(last.left - bottom.right - 4 * density) <= 2)
                assertEquals("The third key must fill the shorter lower row", bottom.top, last.top)
                stage = "equal row lengths start with the upper row"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(
                    terminalShortcuts = listOf(TerminalShortcut.Ctrl, TerminalShortcut.Shift, TerminalShortcut.Alt))) } }
                instrumentation.waitForIdleSync(); checkBar()
                val equalTop = shortcutBounds(TerminalShortcut.Ctrl); val equalBottom = shortcutBounds(TerminalShortcut.Shift)
                val equalLast = shortcutBounds(TerminalShortcut.Alt)
                assertEquals(equalTop.width(), equalBottom.width())
                assertEquals(equalTop.left, equalBottom.left)
                assertEquals(keyboard.top, equalTop.top); assertEquals(composer.top, equalBottom.top)
                assertEquals(equalTop.top, equalLast.top)
                assertTrue(kotlin.math.abs(equalLast.left - equalTop.right - 4 * density) <= 2)
                stage = "independent rows share one horizontal scroll"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(terminalShortcuts = listOf(
                    TerminalShortcut.ShiftTab, TerminalShortcut.Ctrl, TerminalShortcut.ShiftLeft, TerminalShortcut.Alt,
                    TerminalShortcut.Enter, TerminalShortcut.Escape, TerminalShortcut.Tab, TerminalShortcut.Up, TerminalShortcut.PageDown,
                    TerminalShortcut.Down, TerminalShortcut.Left))) } }
                awaitNode(TerminalShortcut.Escape.label)
                val beforeEnter = shortcutBounds(TerminalShortcut.Enter); val beforeEscape = shortcutBounds(TerminalShortcut.Escape)
                assertTrue("Scroll regression must cover both rows", beforeEnter.top != beforeEscape.top)
                val sharedScroll = find(awaitNode("터미널 입력 도구")) { it.isScrollable }!!
                assertTrue(sharedScroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))
                withTimeout(5000) {
                    while (shortcutBounds(TerminalShortcut.Enter).left >= beforeEnter.left) delay(25)
                }
                instrumentation.uiAutomation.waitForIdle(100, 2000)
                assertEquals(beforeEnter.left - shortcutBounds(TerminalShortcut.Enter).left,
                    beforeEscape.left - shortcutBounds(TerminalShortcut.Escape).left)
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(
                    terminalShortcuts = listOf(TerminalShortcut.ShiftTab, TerminalShortcut.Ctrl, TerminalShortcut.Enter))) } }
                stage = "single row setting survives restart"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(mobileShortcutBarTwoRows = false)) } }
                instrumentation.waitForIdleSync(); checkBar(twoRows = false)
                assertEquals(shortcutBounds(TerminalShortcut.ShiftTab).top, shortcutBounds(TerminalShortcut.Ctrl).top)
                assertFalse(app.repository.read().preferences.mobileShortcutBarTwoRows)
                val singleRowRestart = createEngine(instrumentation.targetContext.applicationContext as android.app.Application)
                try { assertFalse(singleRowRestart.local.value.preferences.mobileShortcutBarTwoRows) } finally { singleRowRestart.close() }
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(mobileShortcutBarTwoRows = true, terminalShortcuts = emptyList())) } }
                instrumentation.waitForIdleSync(); checkBar()
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(terminalShortcuts = listOf(TerminalShortcut.Enter))) } }
                instrumentation.waitForIdleSync(); checkBar()
                assertEquals(bounds(awaitNode("키보드 열기")).top, shortcutBounds(TerminalShortcut.Enter).top)
                stage = "default shortcuts retain their visible anchor and scroll together"
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(terminalShortcuts = TerminalShortcut.defaultShortcuts)) } }
                awaitNode(TerminalShortcut.ShiftTab.label)
                val shortcuts = find(awaitNode("터미널 입력 도구")) { it.isScrollable }
                assertNotNull(shortcuts)
                // Lazy items outside the viewport have no accessibility node. Return to the start
                // after expanding the list, which keeps the previously visible Enter as its anchor.
                withTimeout(5000) {
                    while (node(TerminalShortcut.Ctrl.label) == null) {
                        assertTrue(shortcuts!!.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD))
                        delay(200)
                    }
                }
                instrumentation.waitForIdleSync()
                val fixedBeforeScroll = bounds(awaitNode("텍스트 작성 열기"))
                val keyboardBeforeScroll = bounds(awaitNode("키보드 열기")); val menuBeforeScroll = bounds(awaitNode("CLI 실행 메뉴"))
                withTimeout(5000) {
                    while (node(TerminalShortcut.End.label) == null) {
                        assertTrue(shortcuts!!.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))
                        delay(200)
                    }
                }
                screenshot("mobile-independent-shortcuts")
                assertEquals(fixedBeforeScroll, bounds(awaitNode("텍스트 작성 열기")))
                assertEquals(keyboardBeforeScroll, bounds(awaitNode("키보드 열기")))
                assertEquals(menuBeforeScroll, bounds(awaitNode("CLI 실행 메뉴")))
                for (shortcut in listOf(TerminalShortcut.Right, TerminalShortcut.Home, TerminalShortcut.End)) {
                    val scrolled = shortcutBounds(shortcut)
                    assertTrue("Scrolled key must stay in one of the two rows",
                        scrolled.top == keyboardBeforeScroll.top || scrolled.top == fixedBeforeScroll.top)
                    assertTrue("Scrolled key must stay inside the shortcut viewport",
                        scrolled.left >= keyboardBeforeScroll.right && scrolled.right <= menuBeforeScroll.left)
                }
                checkBar()
                var web: WebView? = null
                scenario.onActivity { web = webView(it.window.decorView) }
                val originalSizes = first.sizes.toList()
                repeat(3) {
                    stage = "keyboard open $it"
                    click("키보드 열기"); stage = "keyboard opened click $it"; waitIme(scenario, true); awaitNode("키보드 닫기")
                    var imeTop = 0
                    scenario.onActivity { activity -> imeTop = activity.window.decorView.height - activity.window.decorView.rootWindowInsets.getInsets(WindowInsets.Type.ime()).bottom }
                    assertTrue(bounds(awaitNode("CLI 실행 메뉴")).bottom <= imeTop + 2)
                    delay(200)
                    assertEquals("IME opening must preserve terminal dimensions", originalSizes, first.sizes.toList())
                    stage = "keyboard close $it"
                    click("키보드 닫기"); stage = "keyboard closed click $it"; waitIme(scenario, false)
                    scenario.onActivity { assertSame(web, webView(it.window.decorView)) }
                    delay(200)
                    assertEquals("IME closing must preserve terminal dimensions", originalSizes, first.sizes.toList())
                }
                screenshot("mobile-light-portrait")
                stage = "landscape and hidden shortcuts"
                scenario.onActivity { activity ->
                    // At 320dp height the expanded fixed chrome already exceeds the window
                    // (tracked in TASKS.md). Isolate empty shortcut layout with pane tools folded.
                    app.modify { it.copy(preferences = it.preferences.copy(theme = "dark", terminalShortcuts = emptyList(), mobilePaneToolbarVisible = false)) }
                    activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
                }
                waitOrientation(scenario, true)
                stage = "landscape composer tool"
                awaitNode("텍스트 작성 열기")
                stage = "landscape shortcut bar geometry"
                checkBar()
                screenshot("mobile-dark-landscape")
                scenario.onActivity {
                    assertSame(web, webView(it.window.decorView))
                    app.modify { state -> state.copy(preferences = state.preferences.copy(mobilePaneToolbarVisible = true)) }
                    it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                }
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
                // Paste completion precedes a separately encoded Enter; a failed paste never sends it.
                val paste = draft.replace('\n', '\r')
                assertEquals(paste, first.writes.last().decodeToString())
                assertTrue(app.composer(first).sending.value)
                val pending = awaitNode("전송 중…")
                pending.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                delay(100); assertEquals(count + 1, first.writes.size)
                first.completion.complete(InputCompletion("합성 전송 실패"))
                withTimeout(5000) { while (app.composer(first).sending.value) delay(20) }
                assertEquals(count + 1, first.writes.size)
                assertEquals(draft, app.composer(first).value.value.text)
                first.completion = CompletableDeferred()
                click("전송")
                withTimeout(5000) { while (first.writes.size < count + 2) delay(20) }
                first.completion.complete(InputCompletion())
                awaitNode("텍스트 작성 열기")
                assertEquals(count + 3, first.writes.size)
                assertEquals(paste, first.writes[count + 1].decodeToString())
                assertEquals("\r", first.writes.last().decodeToString())
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
                screenshot("mobile-input-failure")
                fun dump(root: AccessibilityNodeInfo?): String = if (root == null) "" else
                    "[${root.text}|${root.contentDescription}|${bounds(root)}]" +
                        (0 until root.childCount).joinToString { dump(root.getChild(it)) }
                throw AssertionError("Mobile input stage: $stage; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), failure)
            } finally {
                scenario.onActivity {
                    app.removeConnection(first); app.removeConnection(second)
                    app.connections.value = oldConnections; app.selected.value = oldSelected; app.update(previous)
                    it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                }
            }
        }
    }

    @Test fun cliLauncherKeepsImeAndSurvivesResizeWithoutLeakingInput() = runBlocking<Unit> { withLauncherDisplay(animateIme = true) {
        val client = Connection("launcher")
        lateinit var app: AppModel; lateinit var previous: LocalState; lateinit var workspace: WorkspaceState
        var oldConnections = emptyList<ClientConnection>(); var oldSelected: ClientConnection? = null
        var orientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        var stage = "setup"
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; workspace = app.workspace.value
                    oldConnections = app.connections.value; oldSelected = app.selected.value
                    orientation = it.requestedOrientation
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false,
                        mobileShortcutBarTwoRows = true,
                        volumeKeysEnabled = true, volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.Enter))))) }
                    app.connections.value = listOf(client); app.selected.value = client; app.reconcileWorkspace()
                    it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                }
                waitOrientation(scenario, false)
                awaitNode("CLI 실행 메뉴")
                withTimeout(5000) { while (client.surface == null) delay(20) }
                val renderer = client.surface as kim.jujin.dglc.ui.AndroidTerminal
                scenario.onActivity { renderer.focus() }
                suspend fun touch(text: String) {
                    val rect = bounds(awaitNode(text))
                    tap(rect.centerX().toFloat(), rect.centerY().toFloat())
                }
                suspend fun preservesEditor(visible: Boolean, action: suspend () -> Unit) {
                    waitIme(scenario, visible)
                    withTimeout(5000) {
                        var focused = false
                        while (!focused) { scenario.onActivity { focused = renderer.view.hasFocus() }; delay(25) }
                    }
                    lateinit var observation: MenuInputObservation
                    lateinit var callback: Window.Callback
                    scenario.onActivity {
                        callback = it.window.callback
                        observation = MenuInputObservation(it.window.decorView, renderer.view, visible)
                    }
                    println("DGL CLI $stage: observing IME=$visible")
                    var completed = false
                    try {
                        action()
                        delay(250) // Also catch a delayed IME hide/reopen after the menu disappears.
                        completed = true
                    } finally {
                        scenario.onActivity {
                            observation.close()
                            println("DGL CLI $stage: ${observation.frames} frames, failure=${observation.failure}")
                            assertNull("IME/focus changed during $stage: ${observation.failure}", observation.failure)
                            assertTrue("Observed the complete menu gesture", observation.frames > 0)
                            if (completed) assertSame("Menu key callback was removed", callback, it.window.callback)
                        }
                    }
                }
                suspend fun checkOpen(visible: Boolean, anchor: Rect) {
                    repeat(2) {
                        awaitNode("Claude Code 실행")
                        val menu = find(instrumentation.uiAutomation.rootInActiveWindow) { it.paneTitle?.toString() == "CLI 실행 메뉴" }!!
                        assertEquals("Menu stays in the Activity window", awaitNode("CLI 실행 메뉴").windowId, menu.windowId)
                        val panel = bounds(menu)
                        assertTrue("The entire menu stays above the shortcut column", panel.bottom <= anchor.top)
                        scenario.onActivity { activity ->
                            val insets = activity.window.decorView.rootWindowInsets
                            assertEquals(visible, insets.isVisible(WindowInsets.Type.ime()))
                            assertTrue(renderer.view.hasFocus())
                            assertTrue(renderer.view.hasWindowFocus())
                            assertTrue(panel.left >= insets.getInsets(WindowInsets.Type.systemBars()).left)
                            assertTrue(panel.right <= activity.window.decorView.width - insets.getInsets(WindowInsets.Type.systemBars()).right)
                            assertTrue(panel.top >= insets.getInsets(WindowInsets.Type.systemBars()).top)
                        }
                        assertTrue(app.inputBlocked())
                        delay(50)
                    }
                }
                suspend fun checkClosed(visible: Boolean) {
                    withTimeout(5000) { while (node("Claude Code 실행") != null || app.inputBlocked()) delay(25) }
                    waitIme(scenario, visible)
                    withTimeout(5000) {
                        var focused = false
                        while (!focused) { scenario.onActivity { focused = renderer.view.hasFocus() }; delay(25) }
                    }
                    assertSame(renderer, client.surface)
                }
                for (visible in listOf(false, true)) {
                    stage = "open and resize with IME=$visible"
                    if (visible) click("키보드 열기")
                    waitIme(scenario, visible)
                    val anchor = bounds(awaitNode("키보드 닫기".takeIf { visible } ?: "키보드 열기"))
                    val before = client.writes.size
                    preservesEditor(visible) {
                        touch("CLI 실행 메뉴"); checkOpen(visible, anchor)
                        scenario.onActivity {
                            renderer.view.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked'}))", null)
                            val now = android.os.SystemClock.uptimeMillis()
                            it.dispatchKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_VOLUME_UP, 0))
                            it.dispatchKeyEvent(KeyEvent(now, now + 1, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_VOLUME_UP, 0))
                            client.status.value = ConnectionStatus(Phase.Restoring, resizing = true)
                        }
                        instrumentation.sendKeyDownUpSync(KeyEvent.KEYCODE_ENTER)
                        instrumentation.waitForIdleSync(); checkOpen(visible, anchor)
                        assertEquals(before, client.writes.size)
                        scenario.onActivity { client.status.value = ConnectionStatus(Phase.Ready) }
                        instrumentation.waitForIdleSync(); checkOpen(visible, anchor)
                        scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(mobileShortcutBarTwoRows = false)) } }
                        checkBar(twoRows = false)
                        checkOpen(visible, bounds(awaitNode(if (visible) "키보드 닫기" else "키보드 열기")))
                        scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(mobileShortcutBarTwoRows = true)) } }
                        checkBar(); checkOpen(visible, anchor)
                        touch("Codex 실행"); checkClosed(visible)
                    }
                    assertEquals(before + 1, client.writes.size)
                    assertArrayEquals("codex\r".encodeToByteArray(), client.writes.last())
                    for (method in 0..4) {
                        stage = "dismiss method $method with IME=$visible"
                        val outside = bounds(awaitNode("텍스트 작성 열기"))
                        preservesEditor(visible) {
                            touch("CLI 실행 메뉴"); awaitNode("Claude Code 실행")
                            when (method) {
                                0 -> instrumentation.sendKeyDownUpSync(KeyEvent.KEYCODE_ESCAPE)
                                1 -> instrumentation.sendKeyDownUpSync(KeyEvent.KEYCODE_BACK)
                                2 -> instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                                3 -> tap(outside.centerX().toFloat(), outside.centerY().toFloat())
                                else -> {
                                    val terminal = bounds(awaitNode("터미널 입력 도구"))
                                    tap(terminal.left + 10f, anchor.top - 10f)
                                }
                            }
                            checkClosed(visible)
                        }
                        assertFalse(app.composer(client).opened.value)
                        assertEquals(before + 1, client.writes.size)
                    }
                }
                val sent = client.writes.size
                stage = "Back returns to IME handling after menu closes"
                instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                waitIme(scenario, false)
                click("키보드 열기"); waitIme(scenario, true)
                stage = "real recovery dismisses"
                click("CLI 실행 메뉴"); awaitNode("Claude Code 실행")
                scenario.onActivity { client.status.value = ConnectionStatus(Phase.Reconnecting) }
                withTimeout(5000) { while (node("Claude Code 실행") != null || app.inputBlocked()) delay(25) }
                assertEquals(sent, client.writes.size)
                scenario.onActivity { client.status.value = ConnectionStatus(Phase.Ready) }
                stage = "landscape menu scrolls above IME"
                scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE }
                waitOrientation(scenario, true)
                if (node("키보드 열기") != null) click("키보드 열기")
                waitIme(scenario, true)
                val anchor = bounds(awaitNode("키보드 닫기"))
                preservesEditor(true) {
                    touch("CLI 실행 메뉴"); checkOpen(true, anchor)
                    val menu = find(instrumentation.uiAutomation.rootInActiveWindow) { it.paneTitle?.toString() == "CLI 실행 메뉴" }
                    val scroll = find(menu) { it.isScrollable }
                    assertNotNull("Short IME viewport needs an internal menu scroll", scroll)
                    assertTrue(bounds(scroll!!).bottom <= anchor.top)
                    assertTrue(scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD))
                    click("Grok Build 실행 · 실험 지원"); checkClosed(true)
                }
                assertEquals(sent + 1, client.writes.size)
                assertArrayEquals("grok\r".encodeToByteArray(), client.writes.last())
                stage = "shortcut settings from menu"
                scenario.onActivity { it.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT }
                waitOrientation(scenario, false)
                click("CLI 실행 메뉴"); awaitNode("단축키 설정")
                assertTrue(bounds(awaitNode("단축키 설정")).top >= bounds(awaitNode("Grok Build 실행 · 실험 지원")).bottom)
                click("단축키 설정"); awaitNode("사용 중인 단축키")
                assertNull(node("Claude Code 실행"))
                assertTrue(app.inputBlocked())
                scenario.onActivity {
                    renderer.view.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked-settings'}))", null)
                }
                delay(100)
                assertEquals(sent + 1, client.writes.size)
                click("뒤로"); awaitNode("닫기")
                click("닫기")
                withTimeout(5000) { while (app.inputBlocked()) delay(25) }
                assertSame(renderer, client.surface)
            } catch (failure: Throwable) {
                screenshot("cli-launcher-ime-failure")
                throw AssertionError("CLI launcher IME stage: $stage", failure)
            } finally {
                scenario.onActivity {
                    it.window.insetsController?.hide(WindowInsets.Type.ime())
                    app.connections.value = oldConnections; app.selected.value = oldSelected
                    app.workspace.value = workspace; app.update(previous); it.requestedOrientation = orientation
                }
            }
        }
    } }

    /** Sample every frame and focus transition, so hiding then reopening the IME still fails. */
    private class MenuInputObservation(private val root: View, private val editor: WebView, private val imeVisible: Boolean) : AutoCloseable {
        var failure: String? = null
            private set
        var frames = 0
            private set
        private val observer = root.viewTreeObserver
        private val choreographer = Choreographer.getInstance()
        private val focus = ViewTreeObserver.OnGlobalFocusChangeListener { _, _ -> sample() }
        private val windowFocus = ViewTreeObserver.OnWindowFocusChangeListener { sample() }
        private val frame = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                frames++; sample(); choreographer.postFrameCallback(this)
            }
        }
        init {
            observer.addOnGlobalFocusChangeListener(focus)
            observer.addOnWindowFocusChangeListener(windowFocus)
            sample(); choreographer.postFrameCallback(frame)
        }
        private fun sample() {
            val visible = root.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true
            if (failure == null && (visible != imeVisible || !editor.hasFocus() || !editor.hasWindowFocus()))
                failure = "frame=$frames, IME=$visible (expected $imeVisible), editorFocus=${editor.hasFocus()}, windowFocus=${editor.hasWindowFocus()}"
        }
        override fun close() {
            sample(); choreographer.removeFrameCallback(frame)
            if (observer.isAlive) {
                observer.removeOnGlobalFocusChangeListener(focus)
                observer.removeOnWindowFocusChangeListener(windowFocus)
            }
        }
    }

    /** Short app-window diagnostic, not a SurfaceFlinger presentation benchmark. */
    @Test fun imeTransitionsRecordFrameDurations() = runBlocking<Unit> { withLauncherDisplay(animateIme = true) {
        val client = Connection("ime-frames")
        val frames = java.util.Collections.synchronizedList(mutableListOf<Long>())
        val viewSizes = java.util.Collections.synchronizedList(mutableListOf<String>())
        val thread = android.os.HandlerThread("ime-frame-metrics").apply { start() }
        var listening = false
        var dropped = 0
        val listener = Window.OnFrameMetricsAvailableListener { _, metrics, missed ->
            frames += metrics.getMetric(FrameMetrics.TOTAL_DURATION); dropped += missed
        }
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var app: AppModel
            lateinit var previous: LocalState
            var oldConnections = emptyList<ClientConnection>(); var oldSelected: ClientConnection? = null
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false, androidKeyboardMode = "pan")) }
                    app.connections.value = listOf(client); app.selected.value = client
                }
                awaitNode("키보드 열기")
                withTimeout(10_000) { while (client.surface == null || client.sizes.isEmpty()) delay(25) }
                val renderer = client.surface as kim.jujin.dglc.ui.AndroidTerminal
                renderer.write("IME transition diagnostic\r\n".encodeToByteArray())
                delay(750)
                val initialSizes = client.sizes.toList()
                scenario.onActivity { activity ->
                    renderer.view.addOnLayoutChangeListener { _, l, t, r, b, ol, ot, or, ob ->
                        if (r - l != or - ol || b - t != ob - ot) viewSizes += "${r-l}x${b-t}"
                    }
                    activity.window.addOnFrameMetricsAvailableListener(listener, android.os.Handler(thread.looper))
                    listening = true
                }
                repeat(5) {
                    scenario.onActivity { renderer.keyboard(true) }; waitIme(scenario, true); delay(650)
                    scenario.onActivity { renderer.keyboard(false) }; waitIme(scenario, false); delay(650)
                }
                scenario.onActivity { it.window.removeOnFrameMetricsAvailableListener(listener); listening = false }
                val flush = java.util.concurrent.CountDownLatch(1)
                android.os.Handler(thread.looper).post { flush.countDown() }
                assertTrue(flush.await(5, java.util.concurrent.TimeUnit.SECONDS))
                assertTrue("Frame metrics must actually be delivered", frames.isNotEmpty())
                assertEquals("IME must not change the remote grid", initialSizes, client.sizes.toList())
                val report = org.json.JSONObject().put("cycles", 5).put("metric", "app-window-total-duration-ns")
                    .put("framesNs", org.json.JSONArray(frames)).put("droppedMetricCallbacks", dropped)
                    .put("webViewSizeChanges", org.json.JSONArray(viewSizes)).put("remoteResizeCount", client.sizes.size - initialSizes.size)
                File(instrumentation.targetContext.cacheDir, "ime-frames.json").writeText(report.toString(2))
                assertTrue("IME must only clip the WebView: $viewSizes", viewSizes.isEmpty())
            } finally {
                scenario.onActivity {
                    if (listening) it.window.removeOnFrameMetricsAvailableListener(listener)
                    app.connections.value = oldConnections; app.selected.value = oldSelected; app.update(previous)
                }
                thread.quitSafely(); thread.join(5000)
            }
        }
    } }

    @Test fun aboutAppShowsBuildVersionAndReturnsToSettingsWithoutInput() = runBlocking<Unit> { withLauncherDisplay {
        val client = Connection("about")
        val opened = java.util.Collections.synchronizedList(mutableListOf<String>())
        val monitor = object : android.app.Instrumentation.ActivityMonitor() {
            override fun onStartActivity(intent: android.content.Intent): android.app.Instrumentation.ActivityResult? {
                if (intent.action != android.content.Intent.ACTION_VIEW) return null
                opened += intent.dataString.orEmpty()
                return android.app.Instrumentation.ActivityResult(android.app.Activity.RESULT_OK, null)
            }
        }
        var stage = "setup"
        lateinit var app: AppModel
        lateinit var previous: LocalState
        lateinit var workspace: WorkspaceState
        var oldConnections = emptyList<ClientConnection>()
        var oldSelected: ClientConnection? = null
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = (it.application as DglApplication).connections().app
                    previous = app.local.value; workspace = app.workspace.value
                    oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false)) }
                    app.connections.value = listOf(client); app.selected.value = client; app.reconcileWorkspace()
                }
                withTimeout(5000) { while (client.surface == null) delay(20) }
                val renderer = client.surface as kim.jujin.dglc.ui.AndroidTerminal
                stage = "open settings"
                openConnectionMenu(); click("설정"); awaitNode("닫기")
                stage = "scroll to About app"
                withTimeout(5000) {
                    while (node("About app")?.isVisibleToUser != true) {
                        val scroll = find(instrumentation.uiAutomation.rootInActiveWindow) { it.isScrollable }!!
                        scroll.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD); delay(100)
                    }
                }
                val entry = bounds(awaitNode("About app"))
                instrumentation.addMonitor(monitor)
                for (hardwareBack in listOf(false, true)) {
                    stage = "open About app, hardwareBack=$hardwareBack"
                    click("About app"); awaitNode("버전 $APP_VERSION")
                    assertEquals(BuildConfig.VERSION_NAME, APP_VERSION)
                    assertEquals(APP_VERSION, instrumentation.targetContext.packageManager
                        .getPackageInfo(instrumentation.targetContext.packageName, 0).versionName)
                    awaitNode("Reconnect less. Resume more.")
                    assertNotNull(find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.contains("https://ddal.ggak.life") == true })
                    assertNotNull(find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.contains("https://github.com/jujinkim/dgl-client") == true })
                    if (!hardwareBack) for (url in listOf("https://ddal.ggak.life", "https://github.com/jujinkim/dgl-client")) {
                        stage = "open browser link $url"
                        val link = find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.contains(url) == true }!!
                        val button = generateSequence(link) { it.parent }.first { it.isClickable }
                        assertTrue(button.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                        withTimeout(5000) { while (opened.lastOrNull() != url) delay(25) }
                    }
                    assertTrue(app.inputBlocked())
                    scenario.onActivity {
                        renderer.view.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',data:'blocked-about'}))", null)
                        it.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_VOLUME_UP))
                        it.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_VOLUME_UP))
                    }
                    delay(100); assertTrue(client.writes.isEmpty())
                    stage = "return from About app, hardwareBack=$hardwareBack"
                    if (hardwareBack) instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                    else click("뒤로")
                    awaitNode("닫기")
                    assertEquals(entry, bounds(awaitNode("About app")))
                    withTimeout(5000) {
                        while (true) {
                            instrumentation.uiAutomation.clearCache()
                            val focused = find(instrumentation.uiAutomation.rootInActiveWindow) { it.isFocused }
                            if (find(focused) { it.text?.toString()?.contains("About app") == true } != null) break
                            delay(25)
                        }
                    }
                    assertTrue(app.inputBlocked())
                }
                click("닫기")
                withTimeout(5000) { while (app.inputBlocked()) delay(20) }
                scenario.onActivity { assertSame(renderer, client.surface) }
                assertTrue(client.writes.isEmpty())
                assertEquals(0, client.connects)
                assertEquals(listOf("https://ddal.ggak.life", "https://github.com/jujinkim/dgl-client"), opened.toList())
            } catch (failure: Throwable) {
                fun dump(root: AccessibilityNodeInfo?): String = if (root == null) "" else
                    "[${root.text}|${root.contentDescription}|${bounds(root)}|focused=${root.isFocused}]" +
                        (0 until root.childCount).joinToString { dump(root.getChild(it)) }
                throw AssertionError("About app stage=$stage; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), failure)
            } finally {
                instrumentation.removeMonitor(monitor)
                scenario.onActivity {
                    app.connections.value = oldConnections; app.selected.value = oldSelected
                    app.workspace.value = workspace; app.modify { previous }
                }
            }
        }
    } }

    private suspend fun withLauncherDisplay(animateIme: Boolean = false, test: suspend () -> Unit) {
        fun shell(command: String) = instrumentation.uiAutomation.executeShellCommand(command).use {
            android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes().decodeToString().trim()
        }
        val original = listOf("size", "density").associateWith { key ->
            shell("wm $key").lineSequence().firstOrNull { it.startsWith("Override") }?.substringAfter(": ") ?: "reset"
        }
        val settings = if (animateIme) mapOf("global window_animation_scale" to "1", "global transition_animation_scale" to "1",
            "global animator_duration_scale" to "1", "secure show_ime_with_hard_keyboard" to "1") else emptyMap()
        val oldSettings = settings.keys.associateWith { shell("settings get $it") }
        try {
            // Landscape must leave room for the fixed header/tools, but less than four menu rows.
            shell("wm density 160"); shell("wm size 480x800")
            settings.forEach { (key, value) -> shell("settings put $key $value") }
            test()
        } finally {
            original.forEach { (key, value) -> shell("wm $key $value") }
            oldSettings.forEach { (key, value) -> shell(if (value == "null") "settings delete $key" else "settings put $key $value") }
        }
    }

    private suspend fun checkBar(twoRows: Boolean = true) {
        val density = instrumentation.targetContext.resources.displayMetrics.density
        // Flow publication and a Compose frame can follow Looper idle after a settings edit.
        val bar = withTimeout(5000) {
            var rect: Rect
            do {
                rect = bounds(awaitNode("터미널 입력 도구"))
                if (kotlin.math.abs(rect.height() - (if (twoRows) 70 else 36) * density) <= 2) break
                delay(25)
            } while (true)
            rect
        }
        val keyboard = bounds(awaitNode(if (node("키보드 닫기") != null) "키보드 닫기" else "키보드 열기"))
        val text = bounds(awaitNode("텍스트 작성 열기")); val menu = bounds(awaitNode("CLI 실행 메뉴"))
        fun near(actual: Int, dp: Int) = assertTrue("$actual px != $dp dp", kotlin.math.abs(actual - dp * density) <= 2)
        near(keyboard.height(), 30); near(text.height(), 30); near(menu.height(), 30); near(bar.height(), if (twoRows) 70 else 36)
        near(keyboard.left - bar.left, 4); near(keyboard.top - bar.top, 3)
        near(bar.bottom - menu.bottom, 3); near(bar.right - menu.right, 4)
        if (twoRows) {
            assertEquals(keyboard.left, text.left); assertEquals(text.top, menu.top)
            near(text.top - keyboard.bottom, 4); near(bar.bottom - text.bottom, 3)
        } else {
            near(bar.bottom - keyboard.bottom, 3); near(text.left - keyboard.right, 4)
            assertEquals(keyboard.top, text.top); assertEquals(text.top, menu.top)
        }
        assertTrue(text.right < menu.left)
    }
    private suspend fun shortcutBounds(shortcut: TerminalShortcut): Rect {
        val key = awaitNode(shortcut.label)
        return bounds(generateSequence(key) { it.parent }.first { it.isClickable })
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
