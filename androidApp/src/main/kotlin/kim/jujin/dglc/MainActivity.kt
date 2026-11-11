package kim.jujin.dglc

import android.annotation.SuppressLint
import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import androidx.activity.result.contract.ActivityResultContracts
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import androidx.compose.runtime.*
import androidx.core.graphics.drawable.toDrawable
import kim.jujin.dglc.ui.App
import kim.jujin.dglc.ui.VolumeKeyDispatcher
import kim.jujin.dglc.ui.VolumeKeyHost

class MainActivity : ComponentActivity(), VolumeKeyHost {
    override val volumeKeys = VolumeKeyDispatcher()
    private lateinit var holder: AndroidConnectionRuntime
    private var notificationPermissionAsked = false
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }
    private val openSyncFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { fileResult(it, SettingsFileAction.OpenSync) }
    private val createSyncFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { fileResult(it, SettingsFileAction.CreateSync) }
    private val exportSettingsFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { fileResult(it, SettingsFileAction.Export) }
    private val importSettingsFile = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { fileResult(it, SettingsFileAction.Import) }
    private fun fileResult(result: androidx.activity.result.ActivityResult, action: SettingsFileAction) {
        val sync = holder.app.cloudSync as? RustFileSync ?: return
        sync.finishFileSelection()
        if (result.resultCode != RESULT_OK) return
        val data = result.data ?: return
        val uri = data.data ?: return
        when (action) {
            SettingsFileAction.OpenSync -> sync.connect(uri, data.flags, false)
            SettingsFileAction.CreateSync -> sync.connect(uri, data.flags, true)
            SettingsFileAction.Export -> sync.exportFile(uri)
            SettingsFileAction.Import -> sync.importFile(uri)
        }
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
        holder = (application as DglApplication).connections()
        holder.app.languageChanged(holder.app.local.value.preferences.language)
        holder.app.chooseSettingsFile = { action ->
            val sync = holder.app.cloudSync as? RustFileSync
            if (sync?.beginFileSelection() == true) {
                val create = action == SettingsFileAction.CreateSync || action == SettingsFileAction.Export
                val intent = Intent(if (create) Intent.ACTION_CREATE_DOCUMENT else Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = if (create) "application/json" else "*/*"
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    if (action != SettingsFileAction.Import) addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
                    if (action == SettingsFileAction.OpenSync || action == SettingsFileAction.CreateSync) addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                    if (create) putExtra(Intent.EXTRA_TITLE, if (action == SettingsFileAction.Export) "dgl-export.json" else "dgl-sync.json")
                }
                try {
                    when (action) {
                        SettingsFileAction.OpenSync -> openSyncFile.launch(intent)
                        SettingsFileAction.CreateSync -> createSyncFile.launch(intent)
                        SettingsFileAction.Export -> exportSettingsFile.launch(intent)
                        SettingsFileAction.Import -> importSettingsFile.launch(intent)
                    }
                } catch (_: Exception) { sync.finishFileSelection(failed = true) }
            }
        }
        openNotice(intent)
        setContent {
            val local by holder.app.local.collectAsState()
            LaunchedEffect(local.preferences.agentNotifications.os, local.preferences.keepConnectionsInBackground) {
                if ((local.preferences.agentNotifications.os || local.preferences.keepConnectionsInBackground) && !notificationPermissionAsked && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
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
                window.setBackgroundDrawable((if (dark) 0xff1e1f22.toInt() else 0xfff7f7f8.toInt()).toDrawable())
                if (android.os.Build.VERSION.SDK_INT >= 35) {
                    val mask = android.view.WindowInsetsController.APPEARANCE_TRANSPARENT_CAPTION_BAR_BACKGROUND or
                        android.view.WindowInsetsController.APPEARANCE_LIGHT_CAPTION_BARS
                    val appearance = android.view.WindowInsetsController.APPEARANCE_TRANSPARENT_CAPTION_BAR_BACKGROUND or
                        (if (dark) 0 else android.view.WindowInsetsController.APPEARANCE_LIGHT_CAPTION_BARS)
                    window.insetsController?.setSystemBarsAppearance(appearance, mask)
                }
            }
            kim.jujin.dglc.ui.AndroidTerminalResources(holder.app) { App(holder.app) }
        }
    }
    override fun onStart() { super.onStart(); if (::holder.isInitialized) holder.setVisible(true) }
    override fun onStop() {
        if (::holder.isInitialized && !isChangingConfigurations) holder.setVisible(false)
        super.onStop()
    }
    override fun onResume() {
        super.onResume()
        volumeKeys.resumed = true
        if (::holder.isInitialized) holder.app.foreground = hasWindowFocus()
        if (::holder.isInitialized) holder.app.languageChanged(holder.app.local.value.preferences.language)
        if (::holder.isInitialized) holder.app.cloudSync?.sync()
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
        if (::holder.isInitialized) holder.app.chooseSettingsFile = {}
        volumeKeys.clear()
        if (::holder.isInitialized && isFinishing)
            (application as DglApplication).removeTask(holder)
        super.onDestroy()
    }
}
