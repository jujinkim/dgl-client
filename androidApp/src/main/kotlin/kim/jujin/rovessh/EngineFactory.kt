package kim.jujin.rovessh

import android.app.Application
import kim.jujin.rovessh.storage.LocalRepository

internal fun createEngine(application: Application): AppModel {
    val directory = java.io.File(application.noBackupFilesDir, "rust-candidate-v1")
    fun configureLanguage(preference: String) {
        val systemLocale = android.content.res.Resources.getSystem().configuration.locales[0].toLanguageTag()
        kim.jujin.dgl.RustLocalization.configure(preference, systemLocale)
    }
    configureLanguage(LocalRepository(directory).read().preferences.language)
    return AppModel(LocalRepository(directory), AndroidCredentialStore(application, directory), "Android Keystore", kim.jujin.dgl.RustKeyTools, kim.jujin.dgl.RustAgentInbox()) { host, credentials, local, update ->
        kim.jujin.dgl.RustClientConnection(host, credentials, local, update)
    }.also { model ->
        model.languageChanged = ::configureLanguage
        model.cloudSync = kim.jujin.dgl.RustFileSync(application, directory, model)
    }
}
