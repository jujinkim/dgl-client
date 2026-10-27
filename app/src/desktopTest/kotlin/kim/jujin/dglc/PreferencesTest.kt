package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.storage.LocalRepository
import java.nio.file.Files
import kotlin.test.*

class PreferencesTest {
    @Test fun languageChoiceSurvivesRestartAndLegacySettingsUseSystem() {
        val status = CloudSyncStatus()
        val previous = I18n.language
        try {
            I18n.configure("en", "ko")
            assertEquals("Disconnected", t(status.message))
            I18n.configure("ko", "en")
            assertEquals("연결 안 됨", t(status.message))
        } finally { I18n.configure(if (previous == UiLanguage.Korean) "ko" else "en", "") }
        val directory = Files.createTempDirectory("dgl-language-").toFile()
        try {
            directory.resolve("settings.json").writeText("""{"preferences":{"fontSize":19}}""")
            val repository = LocalRepository(directory)
            assertEquals("system", repository.read().preferences.language)
            for (choice in listOf("ko", "en", "system", "future-language")) {
                val state = repository.read().let { it.copy(preferences = it.preferences.copy(language = choice)) }
                repository.save(state)
                assertEquals(state, LocalRepository(directory).read())
            }
        } finally { directory.deleteRecursively() }
    }
    @Test fun accentValidationFallbackContrastAndStoredChoice() {
        val directory = Files.createTempDirectory("dgl-accent-").toFile()
        try {
            val repository = LocalRepository(directory)
            for (invalid in listOf("null", "42", "[]", """{"mode":"unknown","customColor":"#123456"}""",
                """{"mode":"custom","customColor":"#ZZ1234"}""", """{"mode":42}""")) {
                directory.resolve("settings.json").writeText("""{"preferences":{"fontSize":20,"accent":$invalid}}""")
                assertEquals(AccentSettings(), repository.read().preferences.accent)
                assertEquals(20, repository.read().preferences.fontSize)
            }
            assertEquals("#ABC123", normalizeAccentHex(" #aBc123 "))
            listOf("123456", "#123", "#12345678", "#abcdeg", "#é0000").forEach { assertNull(normalizeAccentHex(it)) }
            for (mode in listOf("custom", "system", "default")) {
                val choice = AccentSettings(mode, "#E6BB24")
                val state = LocalState(preferences = Preferences(accent = choice))
                repository.save(state)
                assertEquals(state, repository.read())
                assertEquals("#E6BB24", repository.read().preferences.accent.customColor)
            }
            for (dark in listOf(false, true)) {
                assertEquals(if (dark) 0x75aaff else 0x2468c9, AccentSettings().resolve(dark, null))
                assertEquals(0x123456, AccentSettings().resolve(dark, 0x123456))
                val bg = if (dark) 0x1e1f22 else 0xf7f7f8
                val panel = if (dark) 0x25262a else 0xeeeef0
                for (raw in accentPresets.map { parseAccentHex(it.second)!! } + listOf(0, 0xffffff, 0x777777)) {
                    assertTrue(accentContrast(onAccentColor(raw), raw) >= 4.5)
                    val selection = blendAccent(raw, bg, 0.22)
                    val text = readableAccent(raw, listOf(bg, panel, selection))
                    assertTrue(listOf(bg, panel, selection).all { accentContrast(text, it) >= 4.5 })
                }
            }
        } finally { directory.deleteRecursively() }
    }
    @Test fun repositoryRestoresMacrosAndLegacySettings() {
        val directory = Files.createTempDirectory("dgl-preferences-").toFile()
        try {
            assertTrue(LocalRepository(directory).read().preferences.mobileShortcutBarTwoRows)
            directory.resolve("settings.json").writeText("""{"preferences":{"fontSize":19}}""")
            val legacy = LocalRepository(directory).read()
            assertEquals(Preferences(fontSize = 19), legacy.preferences)
            assertTrue(legacy.preferences.mobilePaneToolbarVisible)
            assertTrue(legacy.preferences.mobileShortcutBarTwoRows)
            assertTrue(legacy.preferences.keepConnectionsInBackground)
            assertFalse(legacy.preferences.showAllHierarchy)
            val state = legacy.copy(preferences = legacy.preferences.copy(keepConnectionsInBackground = false, showAllHierarchy = true, volumeKeysEnabled = true, smoothTerminalTransitions = true, mobilePaneToolbarVisible = false, mobileShortcutBarTwoRows = false,
                volumeUp = VolumeBinding(VolumeAction.Macro, List(65536) { MacroStep.Key(TerminalKey.Enter) }),
                volumeDown = VolumeBinding(VolumeAction.Macro, listOf(MacroStep.Text("한".repeat(21845)), MacroStep.Key(TerminalKey.Enter)))))
            LocalRepository(directory).save(state)
            assertEquals(state, LocalRepository(directory).read())
        } finally { directory.deleteRecursively() }
    }
}
