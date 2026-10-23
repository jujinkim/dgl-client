package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.LocalRepository
import java.nio.file.Files
import kotlin.test.*

class PreferencesTest {
    @Test fun repositoryRestoresMacrosAndLegacySettings() {
        val directory = Files.createTempDirectory("dgl-preferences-").toFile()
        try {
            directory.resolve("settings.json").writeText("""{"preferences":{"fontSize":19}}""")
            val legacy = LocalRepository(directory).read()
            assertEquals(Preferences(fontSize = 19), legacy.preferences)
            val state = legacy.copy(preferences = legacy.preferences.copy(volumeKeysEnabled = true,
                volumeUp = VolumeBinding(VolumeAction.Macro, List(65536) { MacroStep.Key(TerminalKey.Enter) }),
                volumeDown = VolumeBinding(VolumeAction.Macro, listOf(MacroStep.Text("한".repeat(21845)), MacroStep.Key(TerminalKey.Enter)))))
            LocalRepository(directory).save(state)
            assertEquals(state, LocalRepository(directory).read())
        } finally { directory.deleteRecursively() }
    }
}
