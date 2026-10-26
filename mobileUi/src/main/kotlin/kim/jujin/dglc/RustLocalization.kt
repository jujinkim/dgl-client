package kim.jujin.dglc

import kim.jujin.dglc.core.I18n

object RustLocalization {
    fun configure(preference: String, systemLocale: String) {
        I18n.configure(preference, systemLocale)
        uniffi.dgl_core.configureLanguage(preference, systemLocale)
    }
}
