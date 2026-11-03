package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.CliCommand
import kim.jujin.dglc.ui.CliLaunchRequest
import kim.jujin.dglc.ui.CliMenuPlacement
import kim.jujin.dglc.ui.cliMenuPosition
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import kotlinx.coroutines.flow.MutableStateFlow
import kotlin.test.*

class CliLauncherTest {
    private class RecordingConnection : ClientConnection {
        val writes = mutableListOf<ByteArray>()
        override val agents = MutableStateFlow(AgentSnapshot())
        var inputTarget = InputTarget(1, "\$1", "connection:1:pane:1")
        override fun captureInputTarget() = inputTarget
        override val host = HostProfile("launcher", "launcher", "example.test", username = "test")
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready))
        override val remote = MutableStateFlow(RemoteState())
        override val session = MutableStateFlow<String?>("\$1")
        override val pane = MutableStateFlow<String?>("%1")
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val inputNotice = SnackbarState()
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        val encoded = mutableListOf<TerminalModifiers>()
        override fun encodeKeyboard(key: String, text: String?, modifiers: TerminalModifiers, applicationCursor: Boolean): ByteArray? {
            encoded += modifiers
            return if (modifiers.control && text == "한") null else byteArrayOf(3)
        }
        override fun input(bytes: ByteArray) { writes += bytes }
        override fun connect() {}
        override fun trust() {}
        override fun selectTarget(session: String, window: String?, pane: String?) {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        override fun bindSurface(value: TerminalSurface?) {}
        override fun unbindSurface(expected: TerminalSurface) {}
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun disconnect() {}
        override fun close() {}
    }

    @Test fun menuPlacementKeepsTheLauncherAboveAndFlipsSelectionMenusInsideTheOverlay() {
        val menu = IntSize(100, 40)
        fun place(placement: CliMenuPlacement, anchor: Rect) = cliMenuPosition(placement, anchor, menu, 400, 600, 8)
        // The launcher ends at its anchor's right edge, directly above it, whatever space is below.
        assertEquals(IntOffset(150, 160), place(CliMenuPlacement.AboveEnd, Rect(200f, 200f, 250f, 240f)))
        assertEquals(IntOffset(0, 0), place(CliMenuPlacement.AboveEnd, Rect(10f, 40f, 60f, 80f)))
        // A selection menu is centered above the selection, below it near the top, and clamped.
        assertEquals(IntOffset(100, 152), place(CliMenuPlacement.AroundCenter, Rect(100f, 200f, 200f, 220f)))
        assertEquals(IntOffset(0, 48), place(CliMenuPlacement.AroundCenter, Rect(0f, 0f, 20f, 40f)))
        assertEquals(IntOffset(300, 0), place(CliMenuPlacement.AroundCenter, Rect(380f, 0f, 400f, 600f)))
        assertEquals(IntOffset(150, 520), place(CliMenuPlacement.AroundCenter, Rect(150f, 568f, 250f, 600f)))
    }

    @Test fun modifiersToggleUnionConsumeAndRejectUnsupportedWithoutReplay() {
        val client = RecordingConnection()
        val input = ShortcutInput(client)
        input.toggle(TerminalShortcut.Ctrl); input.toggle(TerminalShortcut.Alt)
        input.encode("c", "c", TerminalModifiers(shift = true), false)
        assertEquals(TerminalModifiers(true, true, true), client.encoded.last())
        assertEquals(TerminalModifiers(), input.modifiers.value)
        input.toggle(TerminalShortcut.Shift)
        client.status.value = ConnectionStatus(Phase.Restoring, resizing = true)
        input.sync()
        assertTrue(input.modifiers.value.shift, "viewport changes retain the pending modifier")
        client.status.value = ConnectionStatus(Phase.Ready)
        input.sync()
        assertTrue(input.modifiers.value.shift)
        input.toggle(TerminalShortcut.Shift)
        assertEquals(TerminalModifiers(), input.modifiers.value)
        input.toggle(TerminalShortcut.Ctrl)
        assertNull(input.encode("text", "한", TerminalModifiers(), false))
        assertEquals(SnackbarDuration.Short, client.inputNotice.current.value!!.duration)
        assertNull(client.notice.value)
        assertTrue(input.modifiers.value.control)
        client.input(byteArrayOf(1)) // volume macro/paste/CLI bypass the keyboard encoder
        assertTrue(input.modifiers.value.control)
        input.encode("c", "c", TerminalModifiers(), false)
        assertEquals(TerminalModifiers(), input.modifiers.value)
        assertEquals(2, client.encoded.count { it.control && !it.alt })
    }

    @Test fun targetChangesRecoveryAndModalClearsCannotLeakModifiers() {
        val client = RecordingConnection()
        val input = ShortcutInput(client)
        for (change in 0..4) {
            input.toggle(TerminalShortcut.Ctrl)
            when (change) {
                0 -> client.inputTarget = client.inputTarget.copy(epoch = 2)
                1 -> client.pane.value = "%2"
                2 -> client.session.value = "$2"
                3 -> client.status.value = ConnectionStatus(Phase.Restoring)
                4 -> input.clear()
            }
            input.sync()
            assertEquals(TerminalModifiers(), input.modifiers.value)
            client.status.value = ConnectionStatus(Phase.Ready)
        }
        assertTrue(client.writes.isEmpty())
    }

    @Test fun exactCommandsOnceForActiveShellAndPane() {
        val expected = listOf("claude\r", "claude --dangerously-skip-permissions\r", "codex\r", "codex --yolo\r",
            "agy\r", "agy --dangerously-skip-permissions\r", "grok\r", "grok --always-approve\r")
        assertEquals(expected.size, CliCommand.entries.size)
        for (phase in listOf(Phase.Ready, Phase.Shell)) {
            val first = RecordingConnection()
            val second = RecordingConnection().apply { status.value = ConnectionStatus(phase) }
            for ((command, wire) in CliCommand.entries.zip(expected)) {
                val request = CliLaunchRequest(second)
                assertTrue(request.send(command, second, true))
                assertFalse(request.send(command, second, true))
                assertContentEquals(wire.encodeToByteArray(), second.writes.last())
            }
            assertEquals(expected.size, second.writes.size)
            assertTrue(first.writes.isEmpty())
        }
    }

    @Test fun targetChangesAndUnavailableInputNeverSendOrReplay() {
        for (command in CliCommand.entries) {
            for (phase in Phase.entries.filter { it !in listOf(Phase.Ready, Phase.Shell) }) {
                val client = RecordingConnection().apply { status.value = ConnectionStatus(phase) }
                val request = CliLaunchRequest(client)
                assertFalse(request.send(command, client, true))
                client.status.value = ConnectionStatus(Phase.Ready)
                assertFalse(request.send(command, client, true))
                assertTrue(client.writes.isEmpty())
            }
            for (change in 0..6) {
                val first = RecordingConnection()
                val second = RecordingConnection()
                val request = CliLaunchRequest(first)
                when (change) {
                    0 -> first.pane.value = "%2"
                    1 -> first.session.value = "\$2"
                    2 -> first.status.value = ConnectionStatus(Phase.Restoring)
                    5 -> first.inputTarget = first.inputTarget.copy(epoch = 2)
                    6 -> first.inputTarget = first.inputTarget.copy(identity = "connection:1:pane:2")
                }
                assertFalse(request.send(command, if (change == 3) second else first, change != 4))
                assertTrue(first.writes.isEmpty())
                assertTrue(second.writes.isEmpty())
            }
        }
    }

    @Test fun runningCliInTheInputPaneBlocksLaunchUntilItLeaves() {
        val now = System.currentTimeMillis()
        val claude = AgentObservation("\$1", "@1", "%1", "run", "parent", "Claude")
        for (command in CliCommand.entries) {
            val client = RecordingConnection()
            client.agents.value = AgentSnapshot(valid = true, observed_at = now, observations = listOf(claude))
            assertEquals("Claude", client.agents.value.agentInPane("%1", now))
            assertFalse(CliLaunchRequest(client).send(command, client, true))
            assertTrue(client.writes.isEmpty())
            // An event gap leaves the process observation, so the launch stays blocked.
            client.agents.value = client.agents.value.copy(gap = true)
            assertFalse(CliLaunchRequest(client).send(command, client, true))
            assertTrue(client.writes.isEmpty())
            // Another pane's CLI, a stale poll, or an exited CLI does not block the input pane.
            for (snapshot in listOf(
                AgentSnapshot(valid = true, observed_at = now, observations = listOf(claude.copy(pane = "%2"))),
                AgentSnapshot(valid = true, observed_at = now - 10_000, observations = listOf(claude)),
                AgentSnapshot(valid = false, observed_at = now, observations = listOf(claude)),
                AgentSnapshot(valid = true, observed_at = now),
            )) {
                client.agents.value = snapshot
                assertTrue(CliLaunchRequest(client).send(command, client, true))
            }
            assertEquals(4, client.writes.size)
        }
    }

    @Test fun viewportRestorationRetainsRequestsButRecoveryAndChangedTargetsRejectThem() {
        val ready = ConnectionStatus(Phase.Ready)
        val resizing = ConnectionStatus(Phase.Restoring, resizing = true)
        for ((before, after) in listOf(ready to resizing, resizing to ready, resizing to resizing)) {
            val client = RecordingConnection().apply { status.value = before }
            val request = CliLaunchRequest(client)
            client.status.value = after
            assertTrue(request.send(CliCommand.Codex, client, true))
            assertFalse(request.send(CliCommand.Codex, client, true))
            assertEquals(1, client.writes.size)
            assertContentEquals("codex\r".encodeToByteArray(), client.writes.single())
        }
        for (blocked in listOf(
            resizing.copy(checking = true), resizing.copy(recovering = true),
            resizing.copy(canStopRetrying = true), resizing.copy(resizing = false),
            ConnectionStatus(Phase.Reconnecting),
        )) {
            val client = RecordingConnection()
            val request = CliLaunchRequest(client)
            client.status.value = blocked
            assertFalse(request.send(CliCommand.Codex, client, true))
            client.status.value = ready
            assertFalse(request.send(CliCommand.Codex, client, true))
            assertTrue(client.writes.isEmpty())
        }
        for (change in 0..3) {
            val client = RecordingConnection()
            val request = CliLaunchRequest(client)
            client.status.value = resizing
            when (change) {
                0 -> client.inputTarget = client.inputTarget.copy(epoch = 2)
                1 -> client.inputTarget = client.inputTarget.copy(identity = "connection:1:pane:2")
                2 -> client.session.value = "\$2"
                3 -> client.pane.value = "%2"
            }
            assertFalse(request.send(CliCommand.Codex, client, true))
            client.status.value = ready
            assertFalse(request.send(CliCommand.Codex, client, true))
            assertTrue(client.writes.isEmpty())
        }
    }
}
