package kim.jujin.dglc.core

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

enum class UiLanguage { Korean, English;
    companion object {
        fun resolve(preference: String, systemLocale: String): UiLanguage {
            val tag = when (preference) { "ko", "en" -> preference; else -> systemLocale }
            return if (tag.substringBefore('-').substringBefore('_').substringBefore('.').substringBefore('@').lowercase() == "ko") Korean else English
        }
    }
}

/** Observed by Compose, including static labels in otherwise skippable children. */
object I18n {
    var language by mutableStateOf(UiLanguage.Korean)
        private set
    fun configure(preference: String, systemLocale: String) { language = UiLanguage.resolve(preference, systemLocale) }
}
fun textIn(language: UiLanguage, source: String): String = if (language == UiLanguage.English) EnglishCatalog.messages[source] ?: source else source
fun t(source: String): String = textIn(I18n.language, source)
fun interpolate(template: String, arguments: List<String>): String = Regex("\\{([0-9]+)\\}").replace(template) {
    arguments.getOrNull(it.groupValues[1].toIntOrNull() ?: -1) ?: it.value
}
fun tr(source: String, vararg arguments: Any?): String = interpolate(t(source), arguments.map { it.toString() })
