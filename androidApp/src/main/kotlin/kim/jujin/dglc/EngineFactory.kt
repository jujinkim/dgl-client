package kim.jujin.dglc

import android.app.Application
import kim.jujin.dglc.storage.LocalRepository

internal fun createEngine(application: Application): AppModel {
    val directory = java.io.File(application.noBackupFilesDir, "rust-candidate-v1")
    fun configureLanguage(preference: String) {
        val systemLocale = android.content.res.Resources.getSystem().configuration.locales[0].toLanguageTag()
        kim.jujin.dglc.RustLocalization.configure(preference, systemLocale)
    }
    configureLanguage(LocalRepository(directory).read().preferences.language)
    return AppModel(LocalRepository(directory), AndroidCredentialStore(application, directory), "Android Keystore", kim.jujin.dglc.RustKeyTools, kim.jujin.dglc.RustAgentInbox()) { host, credentials, local, update ->
        kim.jujin.dglc.RustClientConnection(host, credentials, local, update)
    }.also { model ->
        model.workspaceEnabled = true
        model.languageChanged = ::configureLanguage
        model.cloudSync = kim.jujin.dglc.RustFileSync(application, directory, model)
    }
}
