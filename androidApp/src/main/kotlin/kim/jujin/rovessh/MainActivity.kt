package kim.jujin.rovessh

import android.annotation.SuppressLint
import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.result.contract.ActivityResultContracts
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
    val app = createEngine(application).also { it.sendAgentNotification = AgentNotifications(application)::show }
    override fun onCleared() { app.close() }
}

class MainActivity : ComponentActivity(), VolumeKeyHost {
    override val volumeKeys = VolumeKeyDispatcher()
    private lateinit var holder: ClientViewModel
    private var notificationPermissionAsked = false
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }
    private val openSyncFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == RESULT_OK) result.data?.let { data -> data.data?.let { uri ->
            (holder.app.cloudSync as? kim.jujin.dgl.RustFileSync)?.connect(uri, data.flags, false)
        } }
    }
    private val createSyncFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == RESULT_OK) result.data?.let { data -> data.data?.let { uri ->
            (holder.app.cloudSync as? kim.jujin.dgl.RustFileSync)?.connect(uri, data.flags, true)
        } }
    }
    private fun openNotice(intent: Intent?) {
        intent?.getStringExtra(AgentNotifications.NOTICE)?.let { id ->
            holder.app.openAgentNotice(id)
            intent.removeExtra(AgentNotifications.NOTICE)
        }
    }
    override fun onNewIntent(intent: Intent) { super.onNewIntent(intent); setIntent(intent); openNotice(intent) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        holder = ViewModelProvider(this)[ClientViewModel::class.java]
        holder.app.chooseSyncFile = { create ->
            val intent = Intent(if (create) Intent.ACTION_CREATE_DOCUMENT else Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = if (create) "application/json" else "*/*"
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                if (create) putExtra(Intent.EXTRA_TITLE, "dgl-sync.json")
            }
            if (create) createSyncFile.launch(intent) else openSyncFile.launch(intent)
        }
        openNotice(intent)
        setContent {
            val local by holder.app.local.collectAsState()
            LaunchedEffect(local.preferences.agentNotifications.os) {
                if (local.preferences.agentNotifications.os && !notificationPermissionAsked && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                    notificationPermissionAsked = true
                    notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
                }
            }
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
        if (::holder.isInitialized) holder.app.foreground = hasWindowFocus()
        if (::holder.isInitialized) holder.app.cloudSync?.sync()
        if (::holder.isInitialized) holder.app.connections.value.forEach {
            if (it.status.value.phase == kim.jujin.rovessh.core.Phase.Ready) it.restore()
        }
    }
    override fun onPause() {
        volumeKeys.resumed = false
        if (::holder.isInitialized) holder.app.foreground = false
        super.onPause()
    }
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        volumeKeys.focused = hasFocus
        if (::holder.isInitialized) holder.app.foreground = hasFocus && volumeKeys.resumed
    }
    // Activity's public interception hook. Core ComponentActivity marks its override restricted;
    // forwarding unhandled keys to super preserves AndroidX's normal event dispatch.
    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean = volumeKeys.dispatch(event) || super.dispatchKeyEvent(event)
    override fun onDestroy() {
        if (::holder.isInitialized) holder.app.chooseSyncFile = {}
        volumeKeys.clear()
        super.onDestroy()
    }
}
