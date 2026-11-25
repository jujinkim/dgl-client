package kim.jujin.dglc

import kim.jujin.dglc.core.UiCatalog
import kim.jujin.dglc.ui.SettingsBack
import kim.jujin.dglc.ui.SettingsCategory
import kim.jujin.dglc.ui.SettingsLaunch
import kim.jujin.dglc.ui.settingsBack
import kim.jujin.dglc.ui.settingsClusters
import kim.jujin.dglc.ui.shownCategory
import kotlin.test.*

class SettingsNavigationTest {
    @Test fun compactBackReturnsToTheListThenCloses() {
        // Opened from the menu: the category list, then a detail page.
        val opened = SettingsLaunch(null).category
        assertNull(shownCategory(opened, twoPane = false))
        assertEquals(SettingsBack.Close, settingsBack(opened, twoPane = false))
        for (category in SettingsCategory.entries) {
            assertEquals(category, shownCategory(category, twoPane = false))
            assertEquals(SettingsBack.List, settingsBack(category, twoPane = false))
        }
    }

    @Test fun directShortcutEntryBacksToTheList() {
        val route = SettingsLaunch(SettingsCategory.Shortcuts).category
        assertEquals(SettingsCategory.Shortcuts, shownCategory(route, twoPane = false))
        assertEquals(SettingsBack.List, settingsBack(route, twoPane = false))
        // Back clears the route; a second back closes settings.
        assertNull(shownCategory(null, twoPane = false))
        assertEquals(SettingsBack.Close, settingsBack(null, twoPane = false))
        assertTrue(SettingsCategory.Shortcuts.scrollsItself)
        assertEquals(listOf(SettingsCategory.Shortcuts), SettingsCategory.entries.filter { it.scrollsItself })
    }

    @Test fun twoPaneShowsGeneralAndBackCloses() {
        assertEquals(SettingsCategory.General, shownCategory(null, twoPane = true))
        assertEquals(SettingsCategory.Shortcuts, shownCategory(SettingsCategory.Shortcuts, twoPane = true))
        assertEquals(SettingsBack.Close, settingsBack(null, twoPane = true))
        for (category in SettingsCategory.entries) assertEquals(SettingsBack.Close, settingsBack(category, twoPane = true))
    }

    @Test fun categoryTextKeysExistInEveryLanguage() {
        assertEquals(listOf("General", "Terminal", "Keyboard", "Shortcuts", "Connection", "Agents", "Launch", "Sync", "Security", "About"),
            SettingsCategory.entries.map { it.name })
        assertEquals(listOf(listOf("General", "Terminal", "Keyboard", "Shortcuts", "Connection"), listOf("Agents", "Launch"),
            listOf("Sync", "Security", "About")), settingsClusters().map { cluster -> cluster.map { it.name } })
        assertEquals(SettingsCategory.entries.size, SettingsCategory.entries.map { it.titleKey }.toSet().size)
        for (code in UiCatalog.languages) {
            val messages = assertNotNull(UiCatalog.messages(code), code)
            for (category in SettingsCategory.entries) {
                for (key in listOfNotNull(category.titleKey, category.descriptionKey)) {
                    assertTrue(messages[key]?.isNotBlank() == true, "$code is missing $key")
                }
            }
        }
        // Texts the instrumentation tests navigate by.
        val korean = assertNotNull(UiCatalog.messages("ko"))
        assertEquals(listOf("일반", "터미널", "키보드·입력", "단축키", "연결", "에이전트", "CLI 바로 실행", "내보내기 및 동기화", "보안·저장", "앱 정보"),
            SettingsCategory.entries.map { korean.getValue(it.titleKey) })
    }
}
