package kim.jujin.dglc.ui

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*

@Composable internal fun CloudSyncSettings(model: AppModel) {
    val sync = model.cloudSync ?: return
    val state by sync.status.collectAsState()
    DisposableEffect(sync) { onDispose { sync.cancelImport() } }
    val busy = state.busy || state.pendingImportCount != null
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(t("sync.description"))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.Export) }, enabled = !busy) { Text(t("sync.export_ellipsis")) }
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.Import) }, enabled = !busy) { Text(t("sync.import_ellipsis")) }
        }
        state.pendingImportCount?.let { count ->
            Text(tr("sync.import_confirm", count))
            Text(t("sync.import_note"), style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                BorderedButton({ sync.confirmImport() }, enabled = !state.busy) { Text(t("common.import")) }
                BorderedButton({ sync.cancelImport() }, enabled = !state.busy) { Text(t("common.cancel")) }
            }
        }
        HorizontalDivider()
        Text(t("sync.file_sync"), style = MaterialTheme.typography.titleSmall)
        Text(t("sync.connect_hint"))
        Text(t("sync.first_connection_note"), style = MaterialTheme.typography.bodySmall)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.OpenSync) }, enabled = !busy) { Text(t("sync.connect_file")) }
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.CreateSync) }, enabled = !busy) { Text(t("sync.new_sync_file")) }
        }
        Text(t("sync.shared_items_note"), style = MaterialTheme.typography.bodySmall)
        if (state.connected) {
            Text(state.location, style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                BorderedButton({ sync.sync() }, enabled = !busy) { Text(t("sync.sync_now")) }
                BorderedButton({ sync.disconnect() }, enabled = !busy) { Text(t("sync.disconnect")) }
            }
        }
        Text(t(state.message), style = MaterialTheme.typography.bodySmall)
        state.conflicts.forEach { conflict ->
            Text(t("sync.conflicts_title"), style = MaterialTheme.typography.titleSmall)
            conflict.choices.forEach { choice ->
                OutlinedButton({ sync.sync(SyncResolution(conflict.key, choice.revision)) }, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text(choice.label) }
            }
        }
        Text(t("sync.provider_note"), style = MaterialTheme.typography.bodySmall)
    }
}
