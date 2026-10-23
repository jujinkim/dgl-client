package kim.jujin.rovessh

import android.app.Activity
import android.os.SystemClock
import android.view.KeyEvent
import android.view.accessibility.AccessibilityNodeInfo
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.MemoryCredentialStore
import kim.jujin.rovessh.ui.VolumeKeyDispatcher
import kim.jujin.rovessh.ssh.TmuxDiscoveryException
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test

class AndroidVolumeKeyTest {
    private fun event(key: Int, action: Int = KeyEvent.ACTION_DOWN, repeat: Int = 0, time: Long = 10) =
        KeyEvent(time, time + repeat, action, key, repeat)

    @Test fun independentKeysConsumeOnlyTheInitialPress() {
        val dispatcher = VolumeKeyDispatcher().apply { resumed = true; focused = true }
        val sent = mutableListOf<Boolean>()
        dispatcher.register(Any()) { sent += it; true }
        val up = KeyEvent.KEYCODE_VOLUME_UP; val down = KeyEvent.KEYCODE_VOLUME_DOWN
        assertTrue(dispatcher.dispatch(event(up)))
        assertTrue(dispatcher.dispatch(event(up))) // duplicate zero-repeat event
        (1..20).forEach { assertTrue(dispatcher.dispatch(event(up, repeat = it))) }
        assertTrue(dispatcher.dispatch(event(down)))
        assertTrue(dispatcher.dispatch(event(down, KeyEvent.ACTION_UP)))
        assertTrue(dispatcher.dispatch(event(up, KeyEvent.ACTION_UP)))
        assertFalse(dispatcher.dispatch(event(up, KeyEvent.ACTION_UP)))
        assertEquals(listOf(true, false), sent)
        assertTrue(dispatcher.dispatch(event(up, time = 20)))
        assertEquals(listOf(true, false, true), sent)
        assertFalse(dispatcher.dispatch(event(KeyEvent.KEYCODE_A)))
    }

    @Test fun defaultHandlingAndLifecycleNeverReplayHeldKeys() {
        val dispatcher = VolumeKeyDispatcher().apply { resumed = true; focused = true }
        val up = KeyEvent.KEYCODE_VOLUME_UP
        val old = Any(); var calls = 0
        assertFalse(dispatcher.dispatch(event(up)))
        dispatcher.register(old) { calls++; true }
        assertFalse(dispatcher.dispatch(event(up, repeat = 1))) // began with default handling
        assertFalse(dispatcher.dispatch(event(up, KeyEvent.ACTION_UP)))
        assertTrue(dispatcher.dispatch(event(up, time = 20)))
        dispatcher.unregister(old)
        val next = Any()
        dispatcher.register(next) { calls++; true }
        dispatcher.unregister(old) // an old disposal cannot clear the next target
        assertTrue(dispatcher.dispatch(event(up, repeat = 1, time = 20)))
        assertTrue(dispatcher.dispatch(event(up, KeyEvent.ACTION_UP, time = 20)))
        assertEquals(1, calls)
        dispatcher.focused = false
        assertFalse(dispatcher.dispatch(event(up, time = 30)))
        dispatcher.focused = true
        assertFalse(dispatcher.dispatch(event(up, repeat = 1, time = 30)))
        assertTrue(dispatcher.dispatch(event(up, time = 40)))
        dispatcher.resumed = false
        assertFalse(dispatcher.dispatch(event(up, time = 50)))
        dispatcher.resumed = true
        assertFalse(dispatcher.dispatch(event(up, repeat = 1, time = 40)))
        assertFalse(dispatcher.dispatch(event(up, KeyEvent.ACTION_UP, time = 40)))
        dispatcher.clear()
        assertFalse(dispatcher.dispatch(event(up, time = 60)))
        assertEquals(2, calls)
    }

    private class ShellTransport : SshTransport {
        val writes = Channel<ByteArray>(Channel.UNLIMITED)
        override suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?) {}
        override suspend fun execute(command: String) = byteArrayOf() // no tmux: exercise ClientModel's shell path
        override suspend fun tmuxVersion(): String = throw TmuxDiscoveryException(TmuxDiagnostics(listOf(
            TmuxProbeAttempt("SSH 기본 PATH", TmuxProbeOutcome.NotFound),
            TmuxProbeAttempt("로그인 셸 PATH", TmuxProbeOutcome.Failed, exitStatus = 126),
            TmuxProbeAttempt("대표 설치 경로", TmuxProbeOutcome.NotFound, listOf("/opt/homebrew/bin/tmux", "/usr/bin/tmux", "~/.local/bin/tmux")),
        )))
        override suspend fun openControl(sessionId: String): SshTransport.Stream = error("No tmux")
        override suspend fun openShell(size: Size) = object : SshTransport.Stream() {
            override suspend fun read(): ByteArray? = awaitCancellation()
            override suspend fun write(bytes: ByteArray) { writes.send(bytes) }
            override suspend fun resize(size: Size) {}
            override fun close() {}
        }
        override fun close() {}
    }

    @Test fun visibleShellDialogsSelectionRecoveryAndRecreation() = runBlocking<Unit> {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var app: AppModel; lateinit var previous: LocalState
            lateinit var first: ClientModel; lateinit var second: ClientModel
            val firstWire = ShellTransport(); val secondWire = ShellTransport()
            var initialized = false
            try {
                scenario.onMainActivity { activity ->
                    app = ViewModelProvider(activity)[ClientViewModel::class.java].app
                    previous = app.local.value
                    val store = MemoryCredentialStore().apply { write("volume-test", Credential("test")) }
                    val host = HostProfile("volume-test", "Volume test", "example.test", username = "test")
                    first = ClientModel(host, store, app.local, app::modify) { firstWire }
                    second = ClientModel(host.copy(alias = "Second"), store, app.local, app::modify) { secondWire }
                    app.connections.value = listOf(first, second); app.selected.value = first
                    app.update(previous.copy(hosts = listOf(host), preferences = Preferences(volumeKeysEnabled = true,
                        volumeUp = VolumeBinding(VolumeAction.Macro, listOf(MacroStep.Text("한글\\n"), MacroStep.Key(TerminalKey.Enter))),
                        volumeDown = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.CtrlB))))))
                    first.connect(); second.connect()
                    initialized = true
                }
                withTimeout(10000) { first.status.first { it.phase == Phase.Shell }; second.status.first { it.phase == Phase.Shell } }
                waitForText("Esc"); delay(200)
                waitForText("현재 세션: 없음 · 일반 SSH")
                waitForText("목록 확인 불가")
                listOf("+ 세션", "+ 창", "좌우 분할", "상하 분할").forEach { label ->
                    val text = waitForText(label)
                    assertTrue("Tmux actions must remain visible: $label", text.isVisibleToUser)
                    // Compose exposes the label separately from the disabled action node.
                    val ancestors = generateSequence(text) { it.parent }.toList()
                    assertTrue("Unavailable tmux actions must be disabled: $label; " +
                        ancestors.joinToString { "${it.className}(enabled=${it.isEnabled}, clickable=${it.isClickable})" },
                        ancestors.any { !it.isEnabled })
                }
                click("tmux 접기")
                waitForText("현재 세션: 없음 · 일반 SSH")
                waitForText("일반 SSH · tmux 탐지 실패")
                click("tmux 실패 상세 정보")
                waitForText("이 호스트에서 실행할 수 있는 tmux를 찾지 못했습니다.")
                waitForText("호스트: Volume test\ntest@example.test:22")
                press(scenario, false)
                scrollToText("command -v tmux\ntmux -V")
                click("닫기")
                assertEquals(Phase.Shell, first.status.value.phase)
                assertNull(withTimeoutOrNull(200) { firstWire.writes.receive() })
                click("tmux 관리")
                waitForText("좌우 분할")
                press(scenario, true)
                assertArrayEquals("한글\\n\r".encodeToByteArray(), withTimeout(3000) { firstWire.writes.receive() })
                press(scenario, true, key = KeyEvent.KEYCODE_VOLUME_DOWN)
                assertArrayEquals(byteArrayOf(2), withTimeout(3000) { firstWire.writes.receive() })
                assertNull(withTimeoutOrNull(200) { firstWire.writes.receive() })

                click("설정"); waitForText("터미널 글자 크기: 14")
                press(scenario, false); click("닫기"); delay(200)
                click("호스트"); delay(200); press(scenario, false)
                click("삭제"); waitForText("호스트 삭제"); press(scenario, false); click("취소")
                click("+ 호스트"); waitForText("SSH 호스트 추가"); press(scenario, false); click("취소")
                click("터미널"); waitForText("Esc"); delay(200)
                scenario.onMainActivity { first.notice.value = "Volume notice" }
                waitForText("Volume notice"); press(scenario, false); click("닫기"); delay(200)
                scenario.onMainActivity { first.status.value = ConnectionStatus(Phase.Reconnecting) }
                press(scenario, false)
                scenario.onMainActivity { first.status.value = ConnectionStatus(Phase.Restoring) }
                press(scenario, false)
                scenario.onMainActivity { first.status.value = ConnectionStatus(Phase.Shell) }

                scenario.onMainActivity { app.update(app.local.value.copy(preferences = app.local.value.preferences.copy(volumeKeysEnabled = false))) }
                delay(150); press(scenario, false)
                scenario.onMainActivity { app.update(app.local.value.copy(preferences = app.local.value.preferences.copy(volumeKeysEnabled = true, volumeDown = VolumeBinding()))) }
                delay(150); press(scenario, false, key = KeyEvent.KEYCODE_VOLUME_DOWN)
                scenario.onMainActivity { app.selected.value = second }
                delay(300); press(scenario, true)
                assertArrayEquals("한글\\n\r".encodeToByteArray(), withTimeout(3000) { secondWire.writes.receive() })
                assertNull(withTimeoutOrNull(200) { firstWire.writes.receive() })

                val heldTime = SystemClock.uptimeMillis()
                scenario.onMainActivity { assertTrue((it as Activity).dispatchKeyEvent(event(KeyEvent.KEYCODE_VOLUME_UP, time = heldTime))) }
                withTimeout(3000) { secondWire.writes.receive() }
                scenario.recreate(); waitForText("Esc"); delay(300)
                scenario.onMainActivity { assertFalse(it.volumeKeys.dispatch(event(KeyEvent.KEYCODE_VOLUME_UP, repeat = 1, time = heldTime))) }
                assertNull(withTimeoutOrNull(200) { secondWire.writes.receive() })
                scenario.moveToState(Lifecycle.State.CREATED)
                scenario.onMainActivity { assertFalse(it.volumeKeys.dispatch(event(KeyEvent.KEYCODE_VOLUME_UP, time = heldTime + 1))) }
                scenario.moveToState(Lifecycle.State.RESUMED); delay(300)
                press(scenario, true)
                withTimeout(3000) { secondWire.writes.receive() }
                scenario.onMainActivity { second.disconnect() }
                press(scenario, false)
            } finally {
                if (initialized) {
                    first.close(); second.close()
                    scenario.onMainActivity { app.connections.value = emptyList(); app.selected.value = null; app.update(previous) }
                }
            }
        }
    }

    @Test fun editorSaveCancelValidationAndStepOrdering() = runBlocking<Unit> {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            lateinit var app: AppModel; lateinit var previous: LocalState
            scenario.onMainActivity { app = ViewModelProvider(it)[ClientViewModel::class.java].app; previous = app.local.value; app.update(previous.copy(preferences = Preferences())) }
            try {
                click("설정")
                click("볼륨 키로 단축키 실행")
                withTimeout(3000) { while (!app.local.value.preferences.volumeKeysEnabled) delay(50) }
                click("볼륨 위: 기본 음량 조절")
                click("매크로"); click("저장"); waitForText("실행할 텍스트나 키를 추가하세요.")
                click("+ 텍스트")
                setEditorText("git status")
                click("+ 키")
                click("2단계 위로")
                click("저장")
                withTimeout(3000) { while (app.local.value.preferences.volumeUp.action != VolumeAction.Macro) delay(50) }
                assertEquals(listOf(MacroStep.Key(TerminalKey.Enter), MacroStep.Text("git status")), app.local.value.preferences.volumeUp.steps)
                assertEquals(VolumeBinding(), app.local.value.preferences.volumeDown)
                click("볼륨 위: 매크로 · 2단계")
                click("1단계 아래로"); click("2단계 삭제"); setEditorText("changed")
                click("취소")
                assertEquals(listOf(MacroStep.Key(TerminalKey.Enter), MacroStep.Text("git status")), app.local.value.preferences.volumeUp.steps)
                click("볼륨 아래: 기본 음량 조절"); click("단축키"); click("키: Esc"); click("Ctrl+B"); click("저장")
                withTimeout(3000) { while (app.local.value.preferences.volumeDown.action != VolumeAction.Shortcut) delay(50) }
                assertEquals(listOf(MacroStep.Key(TerminalKey.CtrlB)), app.local.value.preferences.volumeDown.steps)
                assertEquals(app.local.value, app.repository.read())
                click("닫기")
            } finally { scenario.onMainActivity { app.update(previous) } }
        }
    }

    private suspend fun press(scenario: ActivityScenario<MainActivity>, consumed: Boolean, key: Int = KeyEvent.KEYCODE_VOLUME_UP) {
        val time = SystemClock.uptimeMillis()
        scenario.onMainActivity { activity ->
            if (consumed) {
                // Enter through the actual Activity hook, with the normal AndroidX fallback intact.
                assertTrue((activity as Activity).dispatchKeyEvent(event(key, time = time)))
                repeat(5) { assertTrue((activity as Activity).dispatchKeyEvent(event(key, repeat = it + 1, time = time))) }
                assertTrue((activity as Activity).dispatchKeyEvent(event(key, KeyEvent.ACTION_UP, time = time)))
            } else {
                assertFalse(activity.volumeKeys.dispatch(event(key, time = time)))
                assertFalse(activity.volumeKeys.dispatch(event(key, repeat = 1, time = time)))
                assertFalse(activity.volumeKeys.dispatch(event(key, KeyEvent.ACTION_UP, time = time)))
            }
        }
    }

    // Recovery animations keep the UI busy. Dispatch directly on Main instead of
    // ActivityScenario's worker-thread waitForIdleSync, just as the resource probe does.
    private suspend fun ActivityScenario<MainActivity>.onMainActivity(action: (MainActivity) -> Unit) {
        withContext(Dispatchers.Main) { onActivity { action(it) } }
    }

    private val automation get() = InstrumentationRegistry.getInstrumentation().uiAutomation
    private fun find(node: AccessibilityNodeInfo?, predicate: (AccessibilityNodeInfo) -> Boolean): AccessibilityNodeInfo? {
        if (node == null) return null
        if (predicate(node)) return node
        for (i in 0 until node.childCount) find(node.getChild(i), predicate)?.let { return it }
        return null
    }
    private suspend fun waitForText(text: String): AccessibilityNodeInfo = withTimeout(5000) {
        while (true) {
            find(automation.rootInActiveWindow) { it.text?.toString() == text || it.contentDescription?.toString() == text }?.let { return@withTimeout it }
            delay(50)
        }
        @Suppress("UNREACHABLE_CODE") error(text)
    }
    private suspend fun click(text: String) = withTimeout(5000) {
        while (true) {
            val root = automation.rootInActiveWindow
            var node = find(root) { it.text?.toString() == text || it.contentDescription?.toString() == text }
            while (node != null && !node.isClickable) node = node.parent
            if (node?.isEnabled == true && node.performAction(AccessibilityNodeInfo.ACTION_CLICK)) break
            find(root) { it.isScrollable && it.className?.toString() != "android.widget.HorizontalScrollView" }
                ?.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)
            delay(100)
        }
    }
    private suspend fun setEditorText(value: String) = withTimeout(5000) {
        while (true) {
            val editor = find(automation.rootInActiveWindow) { it.isEditable }
            if (editor?.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, android.os.Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, value)
            }) == true) break
            delay(50)
        }
    }
    private suspend fun scrollToText(text: String) = withTimeout(5000) {
        while (true) {
            val root = automation.rootInActiveWindow
            if (find(root) { it.text?.toString() == text }?.isVisibleToUser == true) break
            find(root) { it.isScrollable && it.className?.toString() != "android.widget.HorizontalScrollView" }
                ?.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)
            delay(100)
        }
    }
}
