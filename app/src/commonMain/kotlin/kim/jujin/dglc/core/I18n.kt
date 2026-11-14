package kim.jujin.dglc.core

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** A supported UI language. Instances are unique per code, in the picker order of [UiCatalog.languages]. */
class UiLanguage private constructor(val code: String) {
    /** The language's name written in that language, for language pickers. */
    val nativeName: String get() = textIn(this, "language.name")
    override fun toString() = code

    companion object {
        val all: List<UiLanguage> = UiCatalog.languages.map(::UiLanguage)
        fun of(code: String): UiLanguage? = all.firstOrNull { it.code == code }
        val Korean: UiLanguage = of("ko")!!
        val English: UiLanguage = of("en")!!
        val Japanese: UiLanguage = of("ja")!!
        val base: UiLanguage = of(UiCatalog.BASE)!!

        /** Explicit supported preferences win; anything else follows the system locale, then the base language. */
        fun resolve(preference: String, systemLocale: String): UiLanguage = of(preference)
            ?: of(systemLocale.substringBefore('-').substringBefore('_').substringBefore('.').substringBefore('@').lowercase())
            ?: base
    }
}

/** Observed by Compose, including static labels in otherwise skippable children. */
object I18n {
    var language by mutableStateOf(UiLanguage.Korean)
        private set
    fun configure(preference: String, systemLocale: String) { language = UiLanguage.resolve(preference, systemLocale) }
}

/** Looks up [key] in [language], then the base language. Unknown keys (or already localized text) pass through. */
fun textIn(language: UiLanguage, key: String): String =
    UiCatalog.messages(language.code)?.get(key) ?: UiCatalog.messages(UiLanguage.base.code)?.get(key) ?: key
fun t(key: String): String = textIn(I18n.language, key)
fun interpolate(template: String, arguments: List<String>): String = Regex("\\{([0-9]+)\\}").replace(template) {
    arguments.getOrNull(it.groupValues[1].toIntOrNull() ?: -1) ?: it.value
}
fun tr(key: String, vararg arguments: Any?): String = interpolate(t(key), arguments.map { it.toString() })
