package kim.jujin.rovessh

import android.app.Application
import kim.jujin.rovessh.storage.LocalRepository

internal fun createEngine(application: Application): AppModel {
    val directory = java.io.File(application.noBackupFilesDir, "rust-candidate-v1")
    return AppModel(LocalRepository(directory), AndroidCredentialStore(application, directory), "Android Keystore · 별도 테스트 저장소", kim.jujin.dgl.RustKeyTools, kim.jujin.dgl.RustAgentInbox()) { host, credentials, local, update ->
        kim.jujin.dgl.RustClientConnection(host, credentials, local, update)
    }.also { model -> model.cloudSync = kim.jujin.dgl.RustFileSync(application, directory, model) }
}
