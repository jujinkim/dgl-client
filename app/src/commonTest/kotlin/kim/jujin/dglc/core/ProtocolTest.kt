package kim.jujin.dglc.core

import kotlin.test.*

class ProtocolTest {
    @Test fun everyByteBoundaryAndUtf8Preserved() {
        val wire = "%begin 1 2 1\nrow\n%end 1 2 1\n%output %3 안녕\\015\\012\\134\n%output %4 日本語\n"
        val expected = ControlParser().feed(wire.encodeToByteArray())
        for (cut in 0..wire.encodeToByteArray().size) {
            val bytes = wire.encodeToByteArray(); val parser = ControlParser()
            val events = parser.feed(bytes.copyOfRange(0, cut)) + parser.feed(bytes.copyOfRange(cut, bytes.size))
            assertEquals(3, events.size)
            assertContentEquals((expected[1] as ControlEvent.Output).bytes, (events[1] as ControlEvent.Output).bytes)
            assertContentEquals("안녕\r\n\\".encodeToByteArray(), (events[1] as ControlEvent.Output).bytes)
            parser.finish()
        }
    }
    @Test fun commandBodyCannotForgeNotification() {
        val events = ControlParser().feed("%begin 3 4 1\n%output %1 forged\n%end 0 9 1\n%begin 3 5 1\n%error 3 4 1\n".encodeToByteArray())
        val reply = events.single() as ControlEvent.Response
        assertTrue(reply.failed)
        assertEquals(3, reply.lines.size)
    }
    @Test fun unicodeMaySpanOutputEvents() {
        val events = ControlParser().feed("%output %0 \\354\\225\n%output %0 \\210\n".encodeToByteArray())
        assertEquals("안", ((events[0] as ControlEvent.Output).bytes + (events[1] as ControlEvent.Output).bytes).decodeToString())
    }
    @Test fun extendedOutputAndEmptyOutput() {
        val events = ControlParser().feed("%extended-output %5 10 future : x\\000\n%output %7 \n%layout-change @1 layout\n".encodeToByteArray())
        assertContentEquals(byteArrayOf(120, 0), (events[0] as ControlEvent.Output).bytes)
        assertEquals(0, (events[1] as ControlEvent.Output).bytes.size)
        assertEquals("%layout-change", (events[2] as ControlEvent.Notification).name)
    }
    @Test fun malformedAndOversizedMessagesFailClosed() {
        for (wire in listOf("%end 1 1 1\n", "%begin bad\n", "%output %x a\n", "%output %1 \\999\n", "%output %1 \\400\n", "%extended-output %1 bad\n")) {
            assertFailsWith<ProtocolException> { ControlParser().feed(wire.encodeToByteArray()) }
        }
        assertFailsWith<ProtocolException> { ControlParser(8).feed("123456789".encodeToByteArray()) }
        assertFailsWith<ProtocolException> { ControlParser(maxResponse = 2).feed("%begin 1 1 1\nabc\n".encodeToByteArray()) }
        val partial = ControlParser(); partial.feed("%begin 1 1 1\n".encodeToByteArray())
        assertFailsWith<ProtocolException> { partial.finish() }
    }
    @Test fun inputIsHexAndNamesAreQuoted() {
        val payload = "a'; kill-server; \$(touch nope)\r\n한글"
        val commands = payload.encodeToByteArray().asList().chunked(256).map { TmuxCommand.send("%0", it.toByteArray()).wire }
        assertTrue(commands.all { it.matches(Regex("send-keys -H -t '%0' (?:[0-9a-f]{2} ?)+")) })
        assertEquals("new-session -d -s 'safe'\\''; kill-server; ##'", TmuxCommand.newSession("safe'; kill-server; #").wire)
        assertFailsWith<IllegalArgumentException> { TmuxCommand.newSession("x\nkill-server") }
        assertFailsWith<IllegalArgumentException> { TmuxCommand.killPane("%1; kill-server") }
        for (direction in "LRUD") {
            val command = TmuxCommand.focusPane("%12", direction)
            assertEquals("select-pane -t '%12' -$direction", command.wire)
            assertEquals("focusPane", command.nativeOperation!!["op"].toString().trim('"'))
            assertEquals("resize-pane -t '%12' -$direction 3", TmuxCommand.resizePane("%12", direction).wire)
        }
        assertFailsWith<IllegalArgumentException> { TmuxCommand.focusPane("%1", 'X') }
        assertFailsWith<IllegalArgumentException> { TmuxCommand.focusPane("%1; kill-server", 'L') }
    }
    @Test fun captureEscapingDiffersFromLiveOutput() {
        assertEquals("\\033literal\u001b[31mred", decodeCapture("\\\\033literal\\033[31mred".encodeToByteArray()).decodeToString())
        assertTrue(TmuxCommand.renameSession("\$1", "#(touch forbidden)").wire.contains("##(touch forbidden)"))
    }
    @Test fun retryAndInputGates() {
        assertTrue(RetryPolicy.retryable(Failure.Network))
        for (failure in Failure.entries.filter { it != Failure.Network }) assertFalse(RetryPolicy.retryable(failure))
        assertEquals(0, RetryPolicy.delayMillis(0))
        assertEquals(1000, RetryPolicy.delayMillis(20))
        for (phase in Phase.entries) assertEquals(phase == Phase.Ready || phase == Phase.Shell, ConnectionStatus(phase).acceptsInput)
    }
    @Test fun supportedVersionsAndAuthoritativeState() {
        assertTrue(supportsTmux("tmux 3.3a\n")); assertTrue(supportsTmux("tmux 3.7c"))
        assertFalse(supportsTmux("tmux 3.2")); assertFalse(supportsTmux("tmux next-3.8")); assertFalse(supportsTmux("tmux 9.1"))
        val sessions = RemoteStateDecoder.sessions(listOf("\$1|my\\ session\\|name|2".encodeToByteArray()))
        assertEquals(RemoteSession("\$1", "my session|name", 2), sessions.single())
    }
}
