package kim.jujin.rovessh

import android.app.Application
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModelProvider
import kim.jujin.rovessh.storage.LocalRepository
import kim.jujin.rovessh.ui.App

class ClientViewModel(application: Application) : AndroidViewModel(application) {
    val app = AppModel(LocalRepository(application.noBackupFilesDir), AndroidCredentialStore(application), "자격 증명: Android Keystore · 백업 제외")
    override fun onCleared() { app.close() }
}

class MainActivity : ComponentActivity() {
    private lateinit var holder: ClientViewModel
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        holder = ViewModelProvider(this)[ClientViewModel::class.java]
        setContent { App(holder.app) }
    }
    override fun onResume() {
        super.onResume()
        if (::holder.isInitialized) holder.app.connections.value.forEach {
            if (it.status.value.phase == kim.jujin.rovessh.core.Phase.Ready) it.restore()
        }
    }
}
