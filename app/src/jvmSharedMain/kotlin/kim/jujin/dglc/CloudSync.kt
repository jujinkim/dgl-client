package kim.jujin.dglc

import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable

@Serializable data class SyncChoice(val revision: String, val label: String)
@Serializable data class SyncConflict(val key: String, val choices: List<SyncChoice>)
@Serializable data class SyncResolution(val key: String, val revision: String)
// Keep app-owned source copy until rendering, so existing status follows language changes.
data class CloudSyncStatus(
    val connected: Boolean = false, val busy: Boolean = false,
    val location: String = "", val message: String = "common.disconnected",
    val pendingImportCount: Int? = null,
    val conflicts: List<SyncConflict> = emptyList(),
)
enum class SettingsFileAction { Export, Import, OpenSync, CreateSync }
interface CloudSyncController {
    val status: StateFlow<CloudSyncStatus>
    fun confirmImport()
    fun cancelImport()
    fun sync(resolution: SyncResolution? = null)
    fun disconnect()
    fun close()
}
