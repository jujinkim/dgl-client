package kim.jujin.rovessh.core

import kotlinx.serialization.json.Json
import kotlin.test.*

class TerminalInputTest {
    @Test fun binaryMouseReportsPreserveEveryByteAndRejectInvalidInput() {
        val data = (0..255).map { it.toChar() }.joinToString("")
        assertContentEquals(ByteArray(256) { it.toByte() }, TerminalInput.decodeBinary(data))
        assertContentEquals(byteArrayOf(27, 91, 77, 32, 133.toByte(), 53), TerminalInput.decodeBinary("\u001b[M \u00855"))
        assertEquals(65536, TerminalInput.decodeBinary("\u00ff".repeat(65536)).size)
        assertFailsWith<IllegalArgumentException> { TerminalInput.decodeBinary("a".repeat(65537)) }
        assertFailsWith<IllegalArgumentException> { TerminalInput.decodeBinary("한") }
        assertFailsWith<IllegalArgumentException> { TerminalInput.decodeBinary("😀") }
    }

    @Test fun everySupportedKeyAndToolbarOrder() {
        val sequences = mapOf(
            TerminalKey.Escape to "\u001b", TerminalKey.Tab to "\t", TerminalKey.Up to "\u001b[A",
            TerminalKey.ShiftTab to "\u001b[Z", TerminalKey.ShiftLeft to "\u001b[1;2D",
            TerminalKey.Down to "\u001b[B", TerminalKey.Left to "\u001b[D", TerminalKey.Right to "\u001b[C",
            TerminalKey.Home to "\u001b[H", TerminalKey.End to "\u001b[F", TerminalKey.Enter to "\r",
            TerminalKey.Backspace to "\u007f", TerminalKey.Delete to "\u001b[3~",
            TerminalKey.PageUp to "\u001b[5~", TerminalKey.PageDown to "\u001b[6~",
        ) + ('A'..'Z').associate { letter -> TerminalKey.valueOf("Ctrl$letter") to (letter.code - 64).toChar().toString() }
        assertEquals(TerminalKey.entries.toSet(), sequences.keys)
        sequences.forEach { (key, wire) ->
            assertContentEquals(wire.encodeToByteArray(), key.bytes())
            assertContentEquals(wire.encodeToByteArray(), VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(key))).bytes())
        }
        assertEquals(listOf("Esc", "Tab", "Enter", "Shift+Tab", "Shift+←", "Ctrl+C", "Ctrl+D", "Ctrl+L", "↑", "↓", "←", "→", "Home", "End"), TerminalKey.defaultShortcuts.map { it.label })
    }

    @Test fun mixedStepsUnicodeNewlinesAndNoImplicitEnter() {
        assertContentEquals("\u0002n".encodeToByteArray(), TerminalInput.encode(listOf(MacroStep.Key(TerminalKey.CtrlB), MacroStep.Text("n"))))
        assertContentEquals("git status\r".encodeToByteArray(), TerminalInput.encode(listOf(MacroStep.Text("git status"), MacroStep.Key(TerminalKey.Enter))))
        assertEquals("한글 日本語 😀\\n\r두\r세\r끝", TerminalInput.encode(listOf(MacroStep.Text("한글 日本語 😀\\n\n두\r\n세\r끝"))).decodeToString())
        assertEquals("git status", TerminalInput.encode(listOf(MacroStep.Text("git status"))).decodeToString())
        assertEquals("\n", TerminalInput.encode(listOf(MacroStep.Key(TerminalKey.CtrlJ))).decodeToString())
    }

    @Test fun emptyInvalidAndUtf8SizeLimits() {
        assertFailsWith<IllegalArgumentException> { TerminalInput.encode(emptyList()) }
        assertFailsWith<IllegalArgumentException> { TerminalInput.encode(listOf(MacroStep.Text(""))) }
        assertNull(VolumeBinding().bytes())
        assertFailsWith<IllegalArgumentException> { VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Text("a"))).bytes() }
        assertEquals(65536, TerminalInput.encode(listOf(MacroStep.Text("a".repeat(65536)))).size)
        assertFailsWith<IllegalArgumentException> { TerminalInput.encode(listOf(MacroStep.Text("a".repeat(65536)), MacroStep.Key(TerminalKey.Enter))) }
        assertEquals(65536, TerminalInput.encode(listOf(MacroStep.Text("한".repeat(21845)), MacroStep.Key(TerminalKey.Enter))).size)
        assertFailsWith<IllegalArgumentException> { TerminalInput.encode(listOf(MacroStep.Text("한".repeat(21846)))) }
        assertEquals(65536, TerminalInput.encode(listOf(MacroStep.Text("\r\n".repeat(65536)))).size)
    }

    @Test fun oldPreferencesAndIndependentBindingsRoundTrip() {
        val old = Json.decodeFromString<LocalState>("""{"preferences":{"theme":"dark","fontSize":18,"scrollback":1000}}""")
        assertFalse(old.preferences.volumeKeysEnabled)
        assertEquals(VolumeBinding(), old.preferences.volumeUp)
        assertEquals(VolumeBinding(), old.preferences.volumeDown)
        val state = old.copy(preferences = old.preferences.copy(volumeKeysEnabled = true,
            volumeUp = VolumeBinding(VolumeAction.Shortcut, listOf(MacroStep.Key(TerminalKey.CtrlB))),
            volumeDown = VolumeBinding(VolumeAction.Macro, listOf(MacroStep.Text("한글\\n\n"), MacroStep.Key(TerminalKey.Enter)))))
        assertEquals(state, Json.decodeFromString<LocalState>(Json.encodeToString(LocalState.serializer(), state)))
    }
}
