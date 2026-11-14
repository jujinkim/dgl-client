package kim.jujin.dglc.core

import kotlin.test.*

class I18nTest {
    @Test fun systemLanguageOverridesAndUnsupportedFallback() {
        for (tag in listOf("ko", "ko-KR", "KO_kr.UTF-8", "ko_KR@variant")) {
            assertEquals(UiLanguage.Korean, UiLanguage.resolve("system", tag))
            assertEquals(UiLanguage.English, UiLanguage.resolve("en", tag))
        }
        for (tag in listOf("en-US", "zz-ZZ", "", "C")) {
            assertEquals(UiLanguage.English, UiLanguage.resolve("system", tag))
            assertEquals(UiLanguage.Korean, UiLanguage.resolve("ko", tag))
        }
        assertEquals(UiLanguage.Korean, UiLanguage.resolve("future-language", "ko-KR"))
        assertEquals(UiLanguage.English, UiLanguage.base)
    }
    @Test fun everyLanguageIsListedWithItsOwnNameAndSharesKeys() {
        assertEquals(UiCatalog.languages, UiLanguage.all.map { it.code })
        assertEquals("한국어", UiLanguage.Korean.nativeName)
        assertEquals("English", UiLanguage.English.nativeName)
        val base = UiCatalog.messages(UiCatalog.BASE)!!
        for (language in UiLanguage.all) {
            assertSame(language, UiLanguage.of(language.code))
            assertEquals(base.keys, UiCatalog.messages(language.code)!!.keys)
        }
    }
    @Test fun unknownKeysAndFinishedTextPassThrough() {
        assertEquals("missing.key", textIn(UiLanguage.Korean, "missing.key"))
        assertEquals("이미 번역된 문구", textIn(UiLanguage.English, "이미 번역된 문구"))
    }
    @Test fun translationsReorderArgumentsWithoutInterpretingUserData() {
        val name = "한글 {1} \$HOME 😀"
        assertEquals("Delete $name", interpolate(textIn(UiLanguage.English, "common.delete_named"), listOf(name)))
        assertEquals("$name 삭제", interpolate(textIn(UiLanguage.Korean, "common.delete_named"), listOf(name)))
        assertEquals("$name / a / $name", interpolate("{1} / {0} / {1}", listOf("a", name)))
    }
    @Test fun switchingLanguageRefreshesLabelsAndKeepsInputLiteral() {
        val previous = I18n.language
        val input = MacroStep.Text("설정 {0}\n😀")
        val bytes = TerminalInput.encode(listOf(input))
        try {
            I18n.configure("ko", "en-US")
            assertEquals("설정", t("common.settings"))
            assertEquals("파랑", accentPresets.first().first)
            I18n.configure("en", "ko-KR")
            assertEquals("Settings", t("common.settings"))
            assertEquals("Blue", accentPresets.first().first)
            assertEquals("Connected", Phase.Ready.displayLabel())
            assertContentEquals(bytes, TerminalInput.encode(listOf(input)))
        } finally { I18n.configure(previous.code, "") }
    }
}
