package kim.jujin.dglc

import kim.jujin.dglc.core.HostProfile
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable

data class FileTabKey(val host: String, val session: String?)
@Serializable data class FileEntry(val name: String, val directory: Boolean, val regular: Boolean, val size: Long? = null)
@Serializable data class FileListing(val path: String = "", val entries: List<FileEntry> = emptyList(), val fallback: String? = null)
@Serializable data class FileProgress(val item: String = "", val bytes: Long = 0, val size: Long? = null, val completed: Long = 0,
    val conflict: String? = null, val done: Boolean = false, val error: String? = null)
data class FileTabState(val title: String, val listing: FileListing = FileListing(), val loading: Boolean = true,
    val error: String? = null, val fingerprint: String? = null, val selected: Set<String> = emptySet(), val transfers: Map<Long, FileProgress> = emptyMap())
data class FileCopy(val upload: Boolean, val sources: List<String>, val destination: String)

interface FileTabs {
    val tabs: StateFlow<Map<FileTabKey, FileTabState>>
    val selected: StateFlow<FileTabKey?>
    fun open(host: HostProfile, session: String?, pane: String?, name: String?)
    fun select(key: FileTabKey?)
    fun close(key: FileTabKey)
    fun browse(key: FileTabKey, path: String?)
    fun trust(key: FileTabKey)
    fun toggle(key: FileTabKey, name: String)
    fun copy(key: FileTabKey, request: FileCopy)
    fun cancel(key: FileTabKey, job: Long)
    fun overwrite(key: FileTabKey, job: Long, expected: String)
    fun retry(key: FileTabKey, job: Long)
    fun shutdown()
}
