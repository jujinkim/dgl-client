package kim.jujin.rovessh

import android.annotation.SuppressLint
import android.app.Application
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModelProvider
import kim.jujin.rovessh.storage.LocalRepository
import kim.jujin.rovessh.ui.App
import kim.jujin.rovessh.ui.VolumeKeyDispatcher
import kim.jujin.rovessh.ui.VolumeKeyHost

class ClientViewModel(application: Application) : AndroidViewModel(application) {
    val app = AppModel(LocalRepository(application.noBackupFilesDir), AndroidCredentialStore(application), "자격 증명: Android Keystore · 백업 제외")
    override fun onCleared() { app.close() }
}

class MainActivity : ComponentActivity(), VolumeKeyHost {
    override val volumeKeys = VolumeKeyDispatcher()
    private lateinit var holder: ClientViewModel
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        holder = ViewModelProvider(this)[ClientViewModel::class.java]
        setContent { App(holder.app) }
    }
    override fun onResume() {
        super.onResume()
        volumeKeys.resumed = true
        if (::holder.isInitialized) holder.app.connections.value.forEach {
            if (it.status.value.phase == kim.jujin.rovessh.core.Phase.Ready) it.restore()
        }
    }
    override fun onPause() {
        volumeKeys.resumed = false
        super.onPause()
    }
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        volumeKeys.focused = hasFocus
    }
    // Activity's public interception hook. Core ComponentActivity marks its override restricted;
    // forwarding unhandled keys to super preserves AndroidX's normal event dispatch.
    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean = volumeKeys.dispatch(event) || super.dispatchKeyEvent(event)
    override fun onDestroy() {
        volumeKeys.clear()
        super.onDestroy()
    }
}
