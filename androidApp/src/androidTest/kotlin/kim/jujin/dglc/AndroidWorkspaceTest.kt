package kim.jujin.dglc

import android.graphics.Rect
import android.view.*
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.AndroidTerminal
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.*
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** One bounded API36 check, with real Compose, WebViews, pointer gestures and IME. */
class AndroidWorkspaceTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private class Connection(id: String, name: String) : ClientConnection {
        override val host = HostProfile(id, id, "example.test", username = "test")
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready, "연결됨 · tmux"))
        override val remote = MutableStateFlow(RemoteState(sessions = listOf(RemoteSession("\$1", name, 1))))
        override val session = MutableStateFlow<String?>("\$1")
        override val pane = MutableStateFlow<String?>(null)
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val inputNotice = SnackbarState()
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        @Volatile var surface: TerminalSurface? = null
        val writes = java.util.Collections.synchronizedList(mutableListOf<String>())
        var connects = 0
        override fun bindSurface(value: TerminalSurface?) { surface = value }
        override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) surface = null }
        override fun input(bytes: ByteArray) { writes += bytes.decodeToString() }
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

    @Test fun adaptiveGroupsKeepRenderersImeAndActiveInput() = runBlocking<Unit> { withWorkspaceDisplay {
        val first = Connection("first", "one"); val second = Connection("second", "two")
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var model: AppModel
            lateinit var previous: LocalState
            lateinit var previousWorkspace: WorkspaceState
            var oldConnections = emptyList<ClientConnection>()
            var oldSelected: ClientConnection? = null
            var stage = "setup"
            try {
                scenario.onActivity {
                    model = (it.application as DglApplication).connections().app
                    previous = model.local.value; previousWorkspace = model.workspace.value
                    oldConnections = model.connections.value; oldSelected = model.selected.value
                    model.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", mobilePaneToolbarVisible = false,
                        volumeKeysEnabled = true, volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.Tab))))) }
                    model.workspace.value = WorkspaceState()
                    model.connections.value = listOf(first, second); model.selected.value = first
                    model.reconcileWorkspace()
                }
                stage = "839dp"
                awaitNode("열린 연결 탭"); assertNull(node("분할 작업 공간"))
                eventually { first.surface != null }
                val firstRenderer = first.surface as AndroidTerminal
                click("알림 0"); awaitNode("수신한 알림이 없습니다.")
                assertTrue(model.inputBlocked())
                stage = "840dp"
                resize(scenario, 840, 1000); awaitNode("분할 작업 공간")
                eventually { !model.inputBlocked() }
                eventually { first.surface != null }
                assertSame(firstRenderer, first.surface)
                assertNull(node("열린 연결 탭"))
                assertNotNull(node("키보드 열기"))
                stage = "wide split"
                resize(scenario, 1200, 1000)
                click("앱 화면 좌우 분할")
                eventually { model.workspace.value.groups.size == 2 }
                val secondGroup = model.workspace.value.activeGroup
                scenario.onActivity { model.changeWorkspace { it.move(model.tabId(second)!!, secondGroup) } }
                eventually { first.surface != null && second.surface != null }
                val secondRenderer = second.surface as AndroidTerminal
                assertSame(firstRenderer, first.surface)
                stage = "active pointer and input"
                scenario.onActivity { touch(firstRenderer.view) }
                eventually { model.selected.value === first }
                stage = "active bridge"
                post(firstRenderer, "first"); eventually { first.writes.contains("first") }
                post(secondRenderer, "inactive"); delay(100)
                assertTrue(second.writes.isEmpty())
                stage = "active volume key"
                scenario.onActivity { activity -> volume(activity) }
                eventually { first.writes.contains("\t") }
                stage = "global modal block"
                click("설정"); awaitNode("닫기")
                val count = first.writes.size
                post(firstRenderer, "blocked"); post(secondRenderer, "blocked")
                scenario.onActivity { volume(it) }
                delay(150)
                assertEquals("Modal writes: ${first.writes}", count, first.writes.size); assertTrue(second.writes.isEmpty())
                click("닫기")
                stage = "tab menu move"
                click("second · two 탭 메뉴"); click("그룹 1으로 이동")
                eventually { model.workspace.value.groupOf(model.tabId(second)!!) != secondGroup && second.surface != null }
                assertSame(secondRenderer, second.surface)
                stage = "drag cancellation"
                val before = model.workspace.value
                val tab = bounds(awaitNode("second · two"))
                val destination = bounds(awaitNode("터미널 그룹 2"))
                gesture(tab.centerX().toFloat(), tab.centerY().toFloat(), destination.centerX().toFloat(), destination.centerY().toFloat(), cancel = true)
                assertEquals(before, model.workspace.value)
                stage = "drag commit"
                gesture(tab.centerX().toFloat(), tab.centerY().toFloat(), destination.centerX().toFloat(), destination.centerY().toFloat(), cancel = false)
                eventually { model.workspace.value.groupOf(model.tabId(second)!!) == secondGroup && second.surface != null }
                assertSame(secondRenderer, second.surface)
                stage = "vertical layout"
                // Pointer activation may open the IME; establish the no-keyboard geometry first.
                if (node("키보드 닫기") != null) click("키보드 닫기")
                waitIme(scenario, false)
                scenario.onActivity {
                    model.changeWorkspace { state -> state.copy(root = (state.root as WorkspaceNode.Split).copy(axis = SplitAxis.Vertical, ratio = .5f)) }
                }
                resize(scenario, 1200, 650)
                eventually { first.surface != null && second.surface != null }
                assertSame("Height change must retain the Activity renderer", secondRenderer, second.surface)
                val geometry = model.workspace.value.root
                stage = "IME temporary expansion"
                click("키보드 열기"); waitIme(scenario, true)
                stage = "IME projection surface detach"
                eventually { first.surface == null && second.surface != null }
                assertEquals(geometry, model.workspace.value.root)
                assertSame(secondRenderer, second.surface)
                // The active view and keyboard survive the temporary single-group projection.
                waitIme(scenario, true)
                click("키보드 닫기"); waitIme(scenario, false)
                resize(scenario, 1200, 1000)
                eventually { first.surface != null && second.surface != null }
                assertSame(firstRenderer, first.surface); assertSame(secondRenderer, second.surface)
                stage = "narrow tabs cover every group"
                click("키보드 열기"); waitIme(scenario, true)
                resize(scenario, 839, 1000); awaitNode("열린 연결 탭")
                stage = "narrow IME preservation"
                waitIme(scenario, true)
                assertSame(secondRenderer, second.surface)
                click("키보드 닫기"); waitIme(scenario, false)
                click("first · one")
                eventually { model.selected.value === first && first.surface != null }
                assertSame(firstRenderer, first.surface)
                resize(scenario, 1200, 1000); awaitNode("분할 작업 공간")
                eventually { first.surface != null && second.surface != null }
                assertSame(firstRenderer, first.surface); assertSame(secondRenderer, second.surface)
                assertEquals(geometry, model.workspace.value.root)
                assertEquals(0, first.connects + second.connects)
                scenario.onActivity { model.removeConnection(second) }
                eventually { second.surface == null }
                assertEquals(2, model.workspace.value.groups.size)
            } catch (failure: Throwable) {
                throw AssertionError("Workspace stage: $stage; first=${first.surface}; second=${second.surface}; workspace=${model.workspace.value}; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), failure)
            } finally {
                scenario.onActivity {
                    model.removeConnection(first); model.removeConnection(second)
                    model.connections.value = oldConnections; model.selected.value = oldSelected
                    model.changeWorkspace { WorkspaceState(previousWorkspace.root) }
                    model.selected.value = oldSelected; model.reconcileWorkspace(); model.update(previous)
                }
            }
        }
    } }
    @Test fun pinchFontAndScrollBoundsStayWithTheSession() = runBlocking<Unit> { withWorkspaceDisplay {
        // Use phone density and physical pixels: native scale thresholds are
        // physical distances, unlike WebView's CSS viewport dimensions.
        shell("wm density 420"); shell("wm size 1080x2400")
        val first = Connection("font-first", "one"); val second = Connection("font-second", "two")
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var model: AppModel
            lateinit var previous: LocalState
            lateinit var previousWorkspace: WorkspaceState
            var oldConnections = emptyList<ClientConnection>()
            var oldSelected: ClientConnection? = null
            var stage = "setup"
            try {
                scenario.onActivity {
                    model = (it.application as DglApplication).connections().app
                    previous = model.local.value; previousWorkspace = model.workspace.value
                    oldConnections = model.connections.value; oldSelected = model.selected.value
                    model.modify { state -> state.copy(preferences = state.preferences.copy(language = "ko", fontSize = 14, mobilePaneToolbarVisible = false)) }
                    model.workspace.value = WorkspaceState()
                    model.connections.value = listOf(first, second); model.selected.value = first; model.reconcileWorkspace()
                }
                eventually { first.surface != null }
                val renderer = first.surface as AndroidTerminal
                renderer.reset("SHELL_MEASUREMENT".encodeToByteArray())
                val remote = RemoteState(windows = listOf(RemoteWindow("@1", "\$1", "one", true, "", width = 20, height = 10)),
                    panes = listOf(RemotePane("%1", "@1", 20, 10, true, "", "sh")))
                renderer.layout(remote, "@1", "%1", "target")
                renderer.outputPane("%1", "target"); renderer.resizeRemote(Size(20, 10)); renderer.reset("VISIBLE".encodeToByteArray())
                stage = "hidden shell overflow reproduction"
                assertTrue(js(renderer, """(() => {
                    const r=document.querySelector('#terminal'), c=document.querySelector('#window'), s=document.querySelector('#shell');
                    // Give the hidden measurement shell a known overflow instead of relying on its initial fit.
                    c.style.overflow='visible'; s.style.overflow='visible'; s.style.width=(r.clientWidth+200)+'px';
                    const reproduced=r.scrollWidth > r.clientWidth + 100;
                    c.style.removeProperty('overflow'); s.style.removeProperty('overflow'); s.style.removeProperty('width');
                    return reproduced;
                })()""").contains("true"))
                assertTrue(js(renderer, """(() => { const r=document.querySelector('#terminal'); r.scrollLeft=100000;
                    return r.scrollWidth<=r.clientWidth+1 && r.scrollLeft===0; })()""").contains("true"))
                stage = "real wider remote grid"
                renderer.layout(remote.copy(windows = remote.windows.map { it.copy(width = 120) }), "@1", "%1", "target")
                assertTrue(js(renderer, """(() => { const r=document.querySelector('#terminal'), c=document.querySelector('#window'); r.scrollLeft=100000;
                    return r.scrollLeft>0 && Math.abs(r.scrollLeft+r.clientWidth-c.getBoundingClientRect().width)<2; })()""").contains("true"))
                renderer.layout(remote, "@1", "%1", "target")
                assertEquals("0", js(renderer, "document.querySelector('#terminal').scrollLeft"))
                val previousWidth = js(renderer, "document.querySelector('.pane .xterm-screen').getBoundingClientRect().width").toDouble()
                stage = "pinch and reset control"
                pinch(renderer, .30f, .46f)
                val zoomed = renderer.fontSizeOverride.value
                assertTrue("Native pinch must increase the font: $zoomed", zoomed != null && zoomed > 14)
                eventually { js(renderer, "document.querySelector('.pane .xterm-screen').getBoundingClientRect().width").toDouble() > previousWidth * 1.1 }
                awaitNode("세션 글자 크기 초기화")
                val fab = bounds(awaitNode("세션 글자 크기 초기화"))
                val location = IntArray(2)
                instrumentation.runOnMainSync { renderer.view.getLocationOnScreen(location) }
                val edge = (96 * renderer.view.resources.displayMetrics.density).toInt()
                assertTrue("Font reset belongs at the terminal's top right", fab.top >= location[1] &&
                    fab.top < location[1] + edge && fab.right <= location[0] + renderer.view.width &&
                    fab.right > location[0] + renderer.view.width - edge)
                assertEquals(14, model.local.value.preferences.fontSize)
                assertEquals(14, model.repository.read().preferences.fontSize)
                assertTrue(first.writes.isEmpty())
                stage = "default font changes do not clear override"
                scenario.onActivity { model.modify { state -> state.copy(preferences = state.preferences.copy(fontSize = 16)) } }
                eventually { model.local.value.preferences.fontSize == 16 }
                assertEquals(zoomed, model.terminalFont(first).effective)
                stage = "tab isolation and reattachment"
                scenario.onActivity { model.selectConnection(second) }
                eventually { second.surface != null && first.surface == null }
                assertNull(node("세션 글자 크기 초기화"))
                assertNull(model.terminalFont(second).overrideSize.value)
                scenario.onActivity { model.selectConnection(first) }
                eventually { first.surface != null }
                assertSame(renderer, first.surface)
                awaitNode("세션 글자 크기 초기화")
                stage = "Activity recreation retains tab font"
                scenario.recreate()
                eventually { first.surface != null && first.surface !== renderer }
                val recreated = first.surface as AndroidTerminal
                recreated.reset("RECREATED".encodeToByteArray())
                awaitNode("세션 글자 크기 초기화")
                assertEquals(zoomed, model.terminalFont(first).effective)
                stage = "cancelled shrinking pinch and latest-default reset"
                pinch(recreated, .46f, .30f, cancel = true)
                assertTrue(model.terminalFont(first).effective < zoomed!!)
                click("키보드 열기"); waitIme(scenario, true)
                assertTrue(recreated.view.hasFocus())
                click("세션 글자 크기 초기화")
                eventually { model.terminalFont(first).overrideSize.value == null }
                assertEquals(16, model.terminalFont(first).effective)
                assertNull(node("세션 글자 크기 초기화"))
                assertEquals(16, model.repository.read().preferences.fontSize)
                waitIme(scenario, true)
                assertTrue("Font reset preserves the WebView input focus", recreated.view.hasFocus())
                assertTrue("Pinch must never send terminal input", first.writes.isEmpty() && second.writes.isEmpty())
                stage = "keyboard after pinch"
                click("키보드 닫기"); waitIme(scenario, false)
            } catch (failure: Throwable) {
                throw AssertionError("Font/scroll stage: $stage; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), failure)
            } finally {
                scenario.onActivity {
                    model.connections.value = oldConnections; model.selected.value = oldSelected
                    model.workspace.value = previousWorkspace; model.update(previous)
                }
            }
        }
    } }
    private suspend fun pinch(renderer: AndroidTerminal, start: Float, end: Float, cancel: Boolean = false) {
        val time = android.os.SystemClock.uptimeMillis()
        val view = renderer.view
        val location = IntArray(2)
        instrumentation.runOnMainSync { view.getLocationOnScreen(location) }
        fun dispatch(action: Int, half: Float, count: Int = 2) {
            val properties = Array(count) { index -> MotionEvent.PointerProperties().apply { id = index; toolType = MotionEvent.TOOL_TYPE_FINGER } }
            val coords = Array(count) { index -> MotionEvent.PointerCoords().apply {
                x = location[0] + view.width / 2f + view.width * if (index == 0) -half else half
                y = location[1] + view.height / 2f; pressure = 1f; size = 1f
            } }
            val event = MotionEvent.obtain(time, android.os.SystemClock.uptimeMillis(), action, count, properties, coords,
                0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0)
            try { assertTrue(instrumentation.uiAutomation.injectInputEvent(event, true)) } finally { event.recycle() }
        }
        dispatch(MotionEvent.ACTION_DOWN, start, 1)
        dispatch(MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), start)
        repeat(10) { index -> dispatch(MotionEvent.ACTION_MOVE, start + (end - start) * (index + 1) / 10); delay(20) }
        if (cancel) dispatch(MotionEvent.ACTION_CANCEL, end)
        else {
            dispatch(MotionEvent.ACTION_POINTER_UP or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), end)
            dispatch(MotionEvent.ACTION_MOVE, end, 1)
            dispatch(MotionEvent.ACTION_UP, end, 1)
        }
        instrumentation.waitForIdleSync()
    }
    private fun js(renderer: AndroidTerminal, script: String): String {
        val latch = CountDownLatch(1); var result = ""
        instrumentation.runOnMainSync { renderer.view.evaluateJavascript(script) { result = it; latch.countDown() } }
        check(latch.await(5, TimeUnit.SECONDS))
        return result
    }
    private suspend fun withWorkspaceDisplay(test: suspend () -> Unit) {
        fun override(command: String) = shell(command).decodeToString().lineSequence()
            .firstOrNull { it.startsWith("Override") }?.substringAfter(": ") ?: "reset"
        val size = override("wm size"); val density = override("wm density")
        val settings = mapOf("system font_scale" to "1.0", "secure show_ime_with_hard_keyboard" to "1")
        val original = settings.keys.associateWith { shell("settings get $it").decodeToString().trim() }
        try {
            shell("wm density 160"); shell("wm size 839x1000")
            settings.forEach { (key, value) -> shell("settings put $key $value") }
            test()
        } finally {
            shell("wm size $size"); shell("wm density $density")
            original.forEach { (key, value) -> shell(if (value == "null") "settings delete $key" else "settings put $key $value") }
        }
    }
    private suspend fun resize(scenario: ActivityScenario<MainActivity>, width: Int, height: Int) {
        shell("wm size ${width}x$height")
        eventually {
            var measured = false
            scenario.onActivity { measured = it.window.decorView.width == width && it.window.decorView.height == height }
            measured
        }
        instrumentation.waitForIdleSync()
        instrumentation.uiAutomation.waitForIdle(100, 5000)
    }
    private fun shell(command: String) = instrumentation.uiAutomation.executeShellCommand(command).use {
        android.os.ParcelFileDescriptor.AutoCloseInputStream(it).readBytes()
    }
    private fun touch(view: View) {
        val time = android.os.SystemClock.uptimeMillis()
        for (action in listOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP)) {
            MotionEvent.obtain(time, time + action, action, view.width / 2f, view.height / 2f, 0).let { view.dispatchTouchEvent(it); it.recycle() }
        }
    }
    private fun volume(activity: MainActivity) {
        val time = android.os.SystemClock.uptimeMillis()
        activity.dispatchKeyEvent(KeyEvent(time, time, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_VOLUME_UP, 0))
        activity.dispatchKeyEvent(KeyEvent(time, time + 1, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_VOLUME_UP, 0))
    }
    private fun post(renderer: AndroidTerminal, text: String) = instrumentation.runOnMainSync {
        renderer.view.evaluateJavascript("window.dgl.postMessage(JSON.stringify({type:'input',pane:'shell',data:'$text'}))", null)
    }
    private suspend fun gesture(x: Float, y: Float, endX: Float, endY: Float, cancel: Boolean) {
        val time = android.os.SystemClock.uptimeMillis()
        fun inject(action: Int, px: Float, py: Float) {
            val event = MotionEvent.obtain(time, android.os.SystemClock.uptimeMillis(), action, px, py, 0)
            event.source = InputDevice.SOURCE_TOUCHSCREEN
            try { assertTrue(instrumentation.uiAutomation.injectInputEvent(event, true)) } finally { event.recycle() }
        }
        inject(MotionEvent.ACTION_DOWN, x, y)
        delay(ViewConfiguration.getLongPressTimeout() + 100L)
        repeat(10) { index -> inject(MotionEvent.ACTION_MOVE, x + (endX - x) * (index + 1) / 10, y + (endY - y) * (index + 1) / 10); delay(20) }
        awaitNode("탭 배치 미리보기")
        inject(if (cancel) MotionEvent.ACTION_CANCEL else MotionEvent.ACTION_UP, endX, endY)
        instrumentation.waitForIdleSync(); delay(100)
    }
    private suspend fun eventually(predicate: () -> Boolean) = withTimeout(7000) { while (!predicate()) delay(30) }
    private suspend fun waitIme(scenario: ActivityScenario<MainActivity>, visible: Boolean) = eventually {
        var value = false
        scenario.onActivity { value = it.window.decorView.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == visible }
        value
    }
    private fun find(root: AccessibilityNodeInfo?, predicate: (AccessibilityNodeInfo) -> Boolean): AccessibilityNodeInfo? {
        if (root == null) return null
        if (predicate(root)) return root
        for (i in 0 until root.childCount) find(root.getChild(i), predicate)?.let { return it }
        return null
    }
    private fun node(text: String): AccessibilityNodeInfo? {
        instrumentation.uiAutomation.clearCache()
        return find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.toString() == text || it.contentDescription?.toString() == text }
    }
    private suspend fun awaitNode(text: String): AccessibilityNodeInfo = withTimeout(7000) {
        while (true) { node(text)?.let { return@withTimeout it }; delay(30) }; error("unreachable")
    }
    private fun bounds(node: AccessibilityNodeInfo) = Rect().also(node::getBoundsInScreen)
    private suspend fun click(text: String) {
        var target: AccessibilityNodeInfo? = awaitNode(text)
        while (target != null && !target.isClickable) target = target.parent
        assertTrue("Click $text", target?.performAction(AccessibilityNodeInfo.ACTION_CLICK) == true)
        instrumentation.waitForIdleSync(); delay(100)
    }
    private fun dump(node: AccessibilityNodeInfo?): String = if (node == null) "" else
        "[${node.text}|${node.contentDescription}|${bounds(node)}]" + (0 until node.childCount).joinToString { dump(node.getChild(it)) }
}
