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
    // The page header already shows the sync description. The detail column scrolls.
    SettingsGroup(t("sync.file_transfer")) {
        SettingsActions {
            SettingsActionButton(t("sync.export_ellipsis"), { model.chooseSettingsFile(SettingsFileAction.Export) }, enabled = !busy)
            SettingsActionButton(t("sync.import_ellipsis"), { model.chooseSettingsFile(SettingsFileAction.Import) }, enabled = !busy)
        }
        state.pendingImportCount?.let { count ->
            SettingsDivider()
            Text(tr("sync.import_confirm", count), Modifier.fillMaxWidth().padding(start = SettingsSpacing.rowH, end = SettingsSpacing.rowH, top = SettingsSpacing.rowV),
                style = MaterialTheme.typography.bodyLarge)
            SettingsNote(t("sync.import_note"))
            SettingsActions {
                SettingsActionButton(t("common.import"), { sync.confirmImport() }, enabled = !state.busy, style = SettingsActionStyle.Primary)
                SettingsActionButton(t("common.cancel"), { sync.cancelImport() }, enabled = !state.busy)
            }
        }
    }
    SettingsGroup(t("sync.file_sync") + " (experiment)", t("sync.connect_hint")) {
        SettingsNote(t("sync.first_connection_note"), Modifier.padding(top = 6.dp))
        SettingsActions {
            SettingsActionButton(t("sync.connect_file"), { model.chooseSettingsFile(SettingsFileAction.OpenSync) }, enabled = !busy)
            SettingsActionButton(t("sync.new_sync_file"), { model.chooseSettingsFile(SettingsFileAction.CreateSync) }, enabled = !busy)
        }
        SettingsNote(t("sync.shared_items_note"))
        if (state.connected) {
            SettingsDivider()
            SettingsNote(state.location, Modifier.padding(top = 6.dp))
            SettingsActions {
                SettingsActionButton(t("sync.sync_now"), { sync.sync() }, enabled = !busy, style = SettingsActionStyle.Primary)
                SettingsActionButton(t("sync.disconnect"), { sync.disconnect() }, enabled = !busy, style = SettingsActionStyle.Danger)
            }
        }
        SettingsDivider()
        SettingsNote(t(state.message), Modifier.padding(top = 6.dp))
        state.conflicts.forEach { conflict ->
            SettingsSubheading(t("sync.conflicts_title"), Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = 6.dp))
            Column(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH), verticalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
                conflict.choices.forEach { choice ->
                    SettingsActionButton(choice.label, { sync.sync(SyncResolution(conflict.key, choice.revision)) }, enabled = !busy,
                        modifier = Modifier.fillMaxWidth())
                }
            }
        }
        SettingsNote(t("sync.provider_note"), Modifier.padding(bottom = 6.dp))
    }
}
