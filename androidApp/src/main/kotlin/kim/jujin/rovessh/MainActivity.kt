package kim.jujin.rovessh

import android.annotation.SuppressLint
import android.app.Application
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import androidx.compose.runtime.*
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.ViewModelProvider
import kim.jujin.rovessh.ui.App
import kim.jujin.rovessh.ui.VolumeKeyDispatcher
import kim.jujin.rovessh.ui.VolumeKeyHost

class ClientViewModel(application: Application) : AndroidViewModel(application) {
    val app = createEngine(application)
    override fun onCleared() { app.close() }
}

class MainActivity : ComponentActivity(), VolumeKeyHost {
    override val volumeKeys = VolumeKeyDispatcher()
    private lateinit var holder: ClientViewModel
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        holder = ViewModelProvider(this)[ClientViewModel::class.java]
        setContent {
            val local by holder.app.local.collectAsState()
            val dark = when (local.preferences.theme) {
                "dark" -> true; "light" -> false
                else -> resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK == android.content.res.Configuration.UI_MODE_NIGHT_YES
            }
            SideEffect {
                val style = if (dark) SystemBarStyle.dark(android.graphics.Color.TRANSPARENT)
                    else SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT)
                enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
            }
            App(holder.app, mobileLayout = true)
        }
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
