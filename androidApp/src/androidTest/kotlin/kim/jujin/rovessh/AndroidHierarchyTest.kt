package kim.jujin.rovessh

import android.graphics.Rect
import android.view.accessibility.AccessibilityNodeInfo
import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.*
import org.junit.Test

/** Real Compose semantics and geometry with isolated, deterministic connections. */
class AndroidHierarchyTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private class Connection(alias: String) : ClientConnection {
        override val host = HostProfile("same-host", alias, "example.test", username = "test")
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready))
        override val remote = MutableStateFlow(RemoteState(
            listOf(RemoteSession("$1", "개발", 1)),
            listOf(RemoteWindow("@1", "$1", "편집", true, "layout", 3)),
            listOf(RemotePane("%1", "@1", 80, 24, true, "", "sh", "작업"))))
        override val session = MutableStateFlow<String?>("$1")
        override val pane = MutableStateFlow<String?>("%1")
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        override val agents = MutableStateFlow(AgentSnapshot())
        var connects = 0; var disconnects = 0
        val targets = mutableListOf<List<String?>>()
        val writes = mutableListOf<ByteArray>()
        val commands = mutableListOf<TmuxCommand>()
        var surface: TerminalSurface? = null
        override fun captureInputTarget() = InputTarget(1, session.value, pane.value!!)
        override fun selectTarget(session: String, window: String?, pane: String?) { targets += listOf(session, window, pane) }
        override fun connect() { connects++ }
        override fun disconnect() { disconnects++ }
        override fun input(bytes: ByteArray) { writes += bytes }
        override fun manage(command: TmuxCommand) { commands += command }
        override fun trust() {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun refresh() {}
        override fun bindSurface(value: TerminalSurface?) { surface = value }
        override fun unbindSurface(expected: TerminalSurface) { if (surface === expected) surface = null }
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun close() {}
    }
    @Test fun compactRowsExpansionPanelDetailsAndSelectedHostActions() = runBlocking<Unit> {
        val first = Connection("연결 하나"); val second = Connection("연결 둘")
        fun observation(run: String, pane: String, agent: String, kind: String): AgentObservation {
            val e = AgentEvent(run, 1, "server", run, "parent", "$1", "@1", pane, agent, kind, "Test", System.currentTimeMillis())
            return AgentObservation(e.session, e.window, e.pane, run, e.actor, agent, e)
        }
        fun freshAgents() { first.agents.value = AgentSnapshot(server = "server", valid = true, observed_at = System.currentTimeMillis(), observations = listOf(
            observation("one", "%1", "Claude", "Question"), observation("two", "%1", "Claude", "Response"), observation("other", "%2", "Codex", "Error"))) }
        lateinit var app: AppModel; lateinit var previous: LocalState
        var oldConnections = emptyList<ClientConnection>(); var oldSelected: ClientConnection? = null
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            try {
                scenario.onActivity {
                    app = ViewModelProvider(it)[ClientViewModel::class.java].app
                    previous = app.local.value; oldConnections = app.connections.value; oldSelected = app.selected.value
                    app.modify { state -> state.copy(preferences = state.preferences.copy(theme = "light", showAllHierarchy = false, mobilePaneToolbarVisible = false)) }
                    freshAgents(); app.connections.value = listOf(first, second); app.selected.value = first
                }
                click("tmux 관리")
                val compact = "개발\n3: 편집 · 패널 1개"
                await(compact)
                assertNull(node("$compact 펼치기"))
                click("$compact 메뉴")
                listOf("창 만들기", "세션 이름 변경", "세션 삭제…", "창 이름 변경", "창 삭제…").forEach { await(it) }
                instrumentation.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
                await(compact)
                val badge = awaitPrefix("창 에이전트 상태 ·")
                val density = instrumentation.targetContext.resources.displayMetrics.density
                assertTrue(bounds(badge).width() >= (48 * density).toInt() - 1)
                assertTrue(bounds(badge).height() >= (48 * density).toInt() - 1)
                assertTrue(bounds(await(compact)).right <= bounds(await("$compact 메뉴")).left)
                assertTrue(bounds(await("$compact 메뉴")).right <= bounds(badge).left)
                clickNode(badge)
                await("수신 확인 사건: 미확인")
                assertEquals(3, first.agents.value.panelState("@1", first.agentEndpoint, emptyList(), System.currentTimeMillis()).observations.size)
                // Large system fonts put the third observation below the dialog viewport.
                withTimeout(5000) {
                    while (true) {
                        instrumentation.uiAutomation.clearCache()
                        val root = instrumentation.uiAutomation.rootInActiveWindow
                        if (find(root) { it.text?.contains("Codex") == true } != null) break
                        find(root) { it.isScrollable }?.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)
                        delay(30)
                    }
                }
                assertNotNull(node("패널로 이동"))
                click("닫기"); await(compact)
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(showAllHierarchy = true, theme = "dark")) } }
                val session = "개발\n창 1개 · 패널 1개"; val window = "편집\n창 3 · 패널 1개"; val pane = "작업\nsh · 80×24"
                click("$session 펼치기")
                await(window)
                val sx = bounds(await(session)).left; val wx = bounds(await(window)).left
                assertTrue(kotlin.math.abs((20 * density).toInt() - (wx - sx)) <= 1)
                assertNull(node(pane)); assertNull(node("$window 펼치기"))
                fun terminalBounds(): Rect {
                    lateinit var rect: Rect
                    scenario.onActivity {
                        val view = (first.surface as kim.jujin.rovessh.ui.AndroidTerminal).view
                        val location = IntArray(2); view.getLocationOnScreen(location)
                        rect = Rect(location[0], location[1], location[0] + view.width, location[1] + view.height)
                    }
                    return rect
                }
                suspend fun geometry() = listOf(bounds(await(session)), bounds(await(window)), terminalBounds())
                val beforeRestore = geometry()
                assertTrue(beforeRestore.last().width() > 0 && beforeRestore.last().height() > 0)
                for (phase in listOf(Phase.Restoring, Phase.Ready, Phase.Restoring, Phase.Ready)) {
                    scenario.onActivity { first.status.value = ConnectionStatus(phase) }
                    instrumentation.waitForIdleSync()
                    instrumentation.uiAutomation.waitForIdle(100, 5000)
                    assertEquals("Restoration must preserve expanded rows and terminal bounds: $phase", beforeRestore, geometry())
                    assertNull(node("목록 대기 중"))
                }
                scenario.onActivity { first.remote.value = first.remote.value.copy(panes = first.remote.value.panes.map { it.copy(title = "갱신 제목") }) }
                instrumentation.waitForIdleSync()
                await(window)
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(showAllHierarchy = false)) } }
                await("개발\n3: 편집 · 패널 1개")
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(showAllHierarchy = true)) } }
                await(window)
                assertTrue(first.targets.isEmpty()); assertTrue(first.writes.isEmpty()); assertTrue(first.commands.isEmpty())
                scenario.onActivity { app.modify { it.copy(preferences = it.preferences.copy(showAllHierarchy = false)) } }
                click("개발\n3: 편집 · 패널 1개")
                assertEquals(listOf(listOf("$1", "@1", null)), first.targets)
                click("터미널로 돌아가기")
                // Check the unobscured terminal too: the fixed progress slot must not shift it.
                val visibleTerminal = terminalBounds()
                for (phase in listOf(Phase.Restoring, Phase.Ready)) {
                    scenario.onActivity { first.status.value = ConnectionStatus(phase) }
                    instrumentation.waitForIdleSync()
                    instrumentation.uiAutomation.waitForIdle(100, 5000)
                    assertEquals(visibleTerminal, terminalBounds())
                }
                click("현재 호스트 연결 하나 연결 메뉴"); click("재접속")
                assertEquals(1, first.connects); assertEquals(0, second.connects)
                scenario.onActivity { app.selected.value = second }
                click("현재 호스트 연결 둘 연결 메뉴"); click("연결 종료")
                assertEquals(1, second.disconnects); assertEquals(0, first.disconnects)
                assertTrue(first.writes.isEmpty()); assertTrue(second.writes.isEmpty())
            } finally {
                scenario.onActivity { app.connections.value = oldConnections; app.selected.value = oldSelected; app.update(previous) }
            }
        }
    }
    private fun find(root: AccessibilityNodeInfo?, predicate: (AccessibilityNodeInfo) -> Boolean): AccessibilityNodeInfo? {
        if (root == null) return null
        if (predicate(root)) return root
        for (i in 0 until root.childCount) find(root.getChild(i), predicate)?.let { return it }
        return null
    }
    private fun count(root: AccessibilityNodeInfo?, predicate: (AccessibilityNodeInfo) -> Boolean): Int =
        if (root == null) 0 else (if (predicate(root)) 1 else 0) + (0 until root.childCount).sumOf { count(root.getChild(it), predicate) }
    private fun node(text: String): AccessibilityNodeInfo? {
        instrumentation.uiAutomation.clearCache()
        return find(instrumentation.uiAutomation.rootInActiveWindow) { it.text?.toString() == text || it.contentDescription?.toString() == text }
    }
    private suspend fun await(text: String): AccessibilityNodeInfo = try { withTimeout(5000) {
        while (true) { node(text)?.let { return@withTimeout it }; delay(30) }; error("unreachable")
    } } catch (e: TimeoutCancellationException) { instrumentation.uiAutomation.takeScreenshot()?.let { bitmap -> java.io.File(instrumentation.targetContext.cacheDir, "hierarchy-failure.png").outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) } }; throw AssertionError("Missing node: $text; tree=" + dump(instrumentation.uiAutomation.rootInActiveWindow), e) }
    private fun dump(root: AccessibilityNodeInfo?): String = if (root == null) "" else "[${root.text}|${root.contentDescription}|${bounds(root)}|click=${root.isClickable}]" + (0 until root.childCount).joinToString { dump(root.getChild(it)) }

    private suspend fun awaitPrefix(text: String): AccessibilityNodeInfo = withTimeout(5000) {
        while (true) { find(instrumentation.uiAutomation.rootInActiveWindow) { it.contentDescription?.startsWith(text) == true }?.let { return@withTimeout it }; delay(30) }; error("unreachable")
    }
    private fun bounds(node: AccessibilityNodeInfo) = Rect().also { node.getBoundsInScreen(it) }
    private fun clickNode(node: AccessibilityNodeInfo) {
        var target: AccessibilityNodeInfo? = node
        while (target != null && !target.isClickable) target = target.parent
        assertTrue(target?.performAction(AccessibilityNodeInfo.ACTION_CLICK) == true)
        instrumentation.waitForIdleSync()
    }
    private suspend fun click(text: String) = clickNode(await(text))
}
