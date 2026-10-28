package kim.jujin.dglc

import kim.jujin.dglc.core.I18n

object RustLocalization {
    fun configure(preference: String, systemLocale: String) {
        // Core first: a state snapshot fetched for the new Kotlin language must already be localized.
        uniffi.dgl_core.configureLanguage(preference, systemLocale)
        I18n.configure(preference, systemLocale)
    }
}
