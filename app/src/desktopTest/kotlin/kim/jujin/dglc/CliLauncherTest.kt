package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.CliCommand
import kim.jujin.dglc.ui.CliLaunchRequest
import kotlinx.coroutines.flow.MutableStateFlow
import kotlin.test.*

class CliLauncherTest {
    private class RecordingConnection : ClientConnection {
        val writes = mutableListOf<ByteArray>()
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
}
