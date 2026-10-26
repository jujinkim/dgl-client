package kim.jujin.dglc

import com.jediterm.core.util.TermSize
import com.jediterm.terminal.*
import com.jediterm.terminal.emulator.JediEmulator
import com.jediterm.terminal.model.*
import kim.jujin.dglc.tmux.restoreScreen
import java.lang.reflect.Proxy
import kotlin.test.*

internal class HeadlessTerminal(columns: Int = 80, rows: Int = 24) {
    val style = StyleState()
    val buffer = TerminalTextBuffer(columns, rows, style, 5000)
    private val display = Proxy.newProxyInstance(TerminalDisplay::class.java.classLoader, arrayOf(TerminalDisplay::class.java)) { _, method, _ ->
        when (method.returnType) { Boolean::class.javaPrimitiveType -> false; Int::class.javaPrimitiveType -> 0; String::class.java -> ""; else -> null }
    } as TerminalDisplay
    val terminal = JediTerminal(display, buffer, style)
    @Synchronized fun feed(bytes: ByteArray) {
        val stream = ArrayTerminalDataStream(bytes.decodeToString().toCharArray())
        val emulator = JediEmulator(stream, terminal)
        while (emulator.hasNext()) emulator.next()
    }
    @Synchronized fun text() = buffer.historyLinesStorage.joinToString("\n") { it.text } + "\n" + buffer.getScreenLines()
}

class RendererTest {
    @Test fun unicodeColorsCursorAlternateScreenAndResize() {
        val renderer = HeadlessTerminal()
        renderer.feed("\u001b[31m한글 日本語\u001b[0m\u001b[3;4Hcursor".encodeToByteArray())
        assertTrue(renderer.text().contains("한")); assertTrue(renderer.text().contains("日"))
        assertEquals(TerminalColor.index(1), renderer.buffer.getStyleAt(0, 0)!!.foreground)
        assertEquals(3, renderer.terminal.cursorY)
        renderer.feed("\u001b[?1049h\u001b[2J\u001b[Halternate".encodeToByteArray())
        assertTrue(renderer.buffer.isUsingAlternateBuffer)
        assertTrue(renderer.text().contains("alternate"))
        renderer.feed("\u001b[?1049l".encodeToByteArray())
        assertFalse(renderer.buffer.isUsingAlternateBuffer)
        assertTrue(renderer.text().contains("cursor"))
        renderer.terminal.resize(TermSize(100, 40), RequestOrigin.User)
        assertEquals(100, renderer.buffer.width)
        assertEquals(40, renderer.buffer.height)
    }
    @Test fun snapshotRestoresRowsWithoutNewlineDuplicationAndCursor() {
        val renderer = HeadlessTerminal(10, 4)
        val snapshot = restoreScreen(listOf("1234567890", "abcdefghij", "\\033[31mred", "").map { it.encodeToByteArray() }, "3|2|0|1|1|0|0|0|0|0|0|1|0|3|0|10|4", emptyList())
        renderer.feed(snapshot)
        assertEquals("1234567890", renderer.buffer.getLine(0).text)
        assertEquals("abcdefghij", renderer.buffer.getLine(1).text)
        assertEquals("red", renderer.buffer.getLine(2).text)
        assertEquals(4, renderer.terminal.cursorX)
        assertEquals(3, renderer.terminal.cursorY)
        renderer.feed("!".encodeToByteArray())
        assertEquals("red!", renderer.buffer.getLine(2).text)
    }
    @Test fun scrollbackIsBoundedAndLargeOutputFinishes() {
        val renderer = HeadlessTerminal()
        renderer.feed((0..15000).joinToString("\r\n") { "line $it" }.encodeToByteArray())
        assertTrue(renderer.buffer.historyLinesCount <= 5000)
        assertTrue(renderer.text().contains("line 15000"))
    }
}
