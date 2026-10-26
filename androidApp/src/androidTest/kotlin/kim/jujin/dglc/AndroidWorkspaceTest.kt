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
                awaitNode("서버 선택"); assertNull(node("분할 작업 공간"))
                eventually { first.surface != null }
                val firstRenderer = first.surface as AndroidTerminal
                stage = "840dp"
                resize(scenario, 840, 1000); awaitNode("분할 작업 공간")
                eventually { first.surface != null }
                assertSame(firstRenderer, first.surface)
                assertNull(node("서버 선택"))
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
                stage = "narrow selector covers every group"
                click("키보드 열기"); waitIme(scenario, true)
                resize(scenario, 839, 1000); awaitNode("서버 선택")
                stage = "narrow IME preservation"
                waitIme(scenario, true)
                assertSame(secondRenderer, second.surface)
                click("키보드 닫기"); waitIme(scenario, false)
                click("서버 선택"); click("first · one")
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
