package kim.jujin.dglc.ui

import android.content.Intent
import android.net.Uri
import android.os.PowerManager
import android.provider.Settings
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import kim.jujin.dglc.AppModel
import kim.jujin.dglc.core.t

@Composable actual fun PlatformConnectionSettings(model: AppModel) {
    val local by model.local.collectAsState()
    val context = LocalContext.current
    val view = LocalView.current
    val power = remember(context) { context.getSystemService(PowerManager::class.java) }
    var unrestricted by remember { mutableStateOf(power.isIgnoringBatteryOptimizations(context.packageName)) }
    DisposableEffect(view) {
        val listener = android.view.ViewTreeObserver.OnWindowFocusChangeListener { focused ->
            if (focused) unrestricted = power.isIgnoringBatteryOptimizations(context.packageName)
        }
        val observer = view.viewTreeObserver
        observer.addOnWindowFocusChangeListener(listener)
        onDispose { if (observer.isAlive) observer.removeOnWindowFocusChangeListener(listener) }
    }
    Row(Modifier.fillMaxWidth().toggleable(local.preferences.keepConnectionsInBackground, role = Role.Switch,
        onValueChange = { keep -> model.modify { it.copy(preferences = it.preferences.copy(keepConnectionsInBackground = keep)) } }),
        verticalAlignment = Alignment.CenterVertically) {
        Text(t("settings.keep_connections_background"), Modifier.weight(1f))
        AccentSwitch(local.preferences.keepConnectionsInBackground, onCheckedChange = null)
    }
    Text(t("settings.keep_connections_description"), style = MaterialTheme.typography.bodySmall)
    Text(t("settings.recents_dismiss_note"), style = MaterialTheme.typography.bodySmall)
    Text(t(if (unrestricted) "settings.battery_optimization_disabled" else "settings.battery_optimization_enabled"), style = MaterialTheme.typography.bodySmall)
    BorderedButton({
        val intent = Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
        try { context.startActivity(intent) }
        catch (_: android.content.ActivityNotFoundException) {
            context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${context.packageName}")))
        }
    }) { Text(t("settings.open_battery_settings")) }
}
