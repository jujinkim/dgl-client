package kim.jujin.dglc.core

import kotlin.test.*

class TerminalFontSizeTest {
    @Test fun pinchAccumulatesSmallChangesClampsAndIgnoresInvalidFactors() {
        val font = TerminalFontSize(14)
        assertFalse(font.scale(2.0))
        font.beginGesture()
        repeat(4) { font.scale(1.01) }
        assertEquals(15, font.effective, "sub-integer changes accumulate across events")
        for (factor in listOf(Double.NaN, Double.POSITIVE_INFINITY, 0.0, -1.0)) assertFalse(font.scale(factor))
        assertEquals(15, font.effective)
        font.scale(100.0); assertEquals(28, font.effective)
        font.scale(.9); assertEquals(25, font.effective, "reversing at the limit responds immediately")
        font.scale(.01); assertEquals(10, font.effective)
        font.endGesture(); assertFalse(font.scale(2.0))
        font.beginGesture(); font.scale(1.5); assertEquals(15, font.effective)
    }

    @Test fun overridesStayIndependentAndResetUsesTheLatestDefault() {
        val first = TerminalFontSize(14); val second = TerminalFontSize(14)
        first.configure(16); assertEquals(16, first.effective)
        first.beginGesture(); first.scale(1.25); first.endGesture()
        assertEquals(20, first.effective); assertEquals(20, first.overrideSize.value)
        assertNull(second.overrideSize.value); assertEquals(14, second.effective)
        first.configure(18); assertEquals(20, first.effective)
        first.configure(20); assertEquals(20, first.overrideSize.value, "matching defaults do not discard an explicit override")
        first.configure(17); first.reset()
        assertNull(first.overrideSize.value); assertEquals(17, first.effective)
        first.configure(19); assertEquals(19, first.effective)
        assertNull(TerminalFontSize(19).overrideSize.value, "a new tab starts without an override")
    }
}
