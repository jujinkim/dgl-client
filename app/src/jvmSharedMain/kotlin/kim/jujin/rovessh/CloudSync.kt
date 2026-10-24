package kim.jujin.rovessh

import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable

@Serializable data class SyncChoice(val revision: String, val label: String)
@Serializable data class SyncConflict(val key: String, val choices: List<SyncChoice>)
@Serializable data class SyncResolution(val key: String, val revision: String)
data class CloudSyncStatus(
    val connected: Boolean = false, val busy: Boolean = false,
    val location: String = "", val message: String = "연결 안 됨",
    val conflicts: List<SyncConflict> = emptyList(),
)
interface CloudSyncController {
    val status: StateFlow<CloudSyncStatus>
    fun sync(resolution: SyncResolution? = null)
    fun disconnect()
    fun close()
}
