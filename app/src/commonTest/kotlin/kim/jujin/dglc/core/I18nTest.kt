package kim.jujin.dglc.core

import kotlin.test.*

class I18nTest {
    @Test fun systemLanguageOverridesAndUnsupportedFallback() {
        for (tag in listOf("ko", "ko-KR", "KO_kr.UTF-8", "ko_KR@variant")) {
            assertEquals(UiLanguage.Korean, UiLanguage.resolve("system", tag))
            assertEquals(UiLanguage.English, UiLanguage.resolve("en", tag))
        }
        for (tag in listOf("en-US", "ja-JP", "", "C")) {
            assertEquals(UiLanguage.English, UiLanguage.resolve("system", tag))
            assertEquals(UiLanguage.Korean, UiLanguage.resolve("ko", tag))
        }
        assertEquals(UiLanguage.Korean, UiLanguage.resolve("future-language", "ko-KR"))
    }
    @Test fun translationsReorderArgumentsWithoutInterpretingUserData() {
        val name = "한글 {1} \$HOME 😀"
        assertEquals("Delete $name", interpolate(textIn(UiLanguage.English, "{0} 삭제"), listOf(name)))
        assertEquals("$name 삭제", interpolate(textIn(UiLanguage.Korean, "{0} 삭제"), listOf(name)))
        assertEquals("$name / a / $name", interpolate("{1} / {0} / {1}", listOf("a", name)))
    }
    @Test fun switchingLanguageRefreshesLabelsAndKeepsInputLiteral() {
        val previous = I18n.language
        val input = MacroStep.Text("설정 {0}\n😀")
        val bytes = TerminalInput.encode(listOf(input))
        try {
            I18n.configure("ko", "en-US")
            assertEquals("설정", t("설정"))
            assertEquals("파랑", accentPresets.first().first)
            I18n.configure("en", "ko-KR")
            assertEquals("Settings", t("설정"))
            assertEquals("Blue", accentPresets.first().first)
            assertEquals("Connected", Phase.Ready.displayLabel())
            assertContentEquals(bytes, TerminalInput.encode(listOf(input)))
        } finally { I18n.configure(if (previous == UiLanguage.Korean) "ko" else "en", "") }
    }
}
