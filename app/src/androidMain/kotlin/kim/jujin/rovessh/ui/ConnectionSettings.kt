package kim.jujin.rovessh.ui

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
import kim.jujin.rovessh.AppModel
import kim.jujin.rovessh.core.t

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
        Text(t("백그라운드에서 연결 유지"), Modifier.weight(1f))
        AccentSwitch(local.preferences.keepConnectionsInBackground, onCheckedChange = null)
    }
    Text(t("홈 이동·화면 잠금 중에도 연결을 유지합니다. 최근 앱에서 닫으면 모든 연결을 종료하며 원격 tmux 작업은 유지합니다."), style = MaterialTheme.typography.bodySmall)
    Text(t(if (unrestricted) "배터리 최적화 제외됨" else "배터리 최적화 적용 중 · 기기 설정에 따라 연결이 제한될 수 있습니다."), style = MaterialTheme.typography.bodySmall)
    TextButton({
        val intent = Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
        try { context.startActivity(intent) }
        catch (_: android.content.ActivityNotFoundException) {
            context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${context.packageName}")))
        }
    }) { Text(t("배터리 설정 열기")) }
}
