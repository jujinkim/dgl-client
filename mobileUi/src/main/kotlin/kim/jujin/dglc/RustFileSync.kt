package kim.jujin.dglc

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import kim.jujin.dglc.*
import kim.jujin.dglc.core.LocalState
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.json.Json
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.util.UUID
import uniffi.dgl_core.exchangeSync
import uniffi.dgl_core.acknowledgeSync
import uniffi.dgl_core.newSyncDocument
import uniffi.dgl_core.exportSettings
import uniffi.dgl_core.importSettings

/** Uses only the user's persisted SAF grant. No Google API, OAuth or account access. */
class RustFileSync(context: Context, private val directory: File, private val model: AppModel) : CloudSyncController {
    private val contextForDocuments = context.applicationContext
    private val resolver = context.applicationContext.contentResolver
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val mutex = Mutex()
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
    private val checkpoint = File(directory, "cloud-sync.json")
    private var config: Config? = null
    private var selecting = false
    @Volatile private var pendingImport: String? = null
    @Volatile private var importing = false
    @Volatile private var importGeneration = 0L
    private fun unavailable() = status.value.busy || status.value.pendingImportCount != null
    private fun encoded(local: LocalState) = json.encodeToString(LocalState.serializer(), local)
    @Synchronized fun beginFileSelection(): Boolean {
        if (unavailable()) return false
        selecting = true
        status.update { it.copy(busy = true) }
        return true
    }
    @Synchronized fun finishFileSelection(failed: Boolean = false) {
        if (!selecting) return
        selecting = false
        status.update { it.copy(busy = false, message = if (failed) "sync.file_picker_failed" else it.message) }
    }
    @Synchronized fun exportFile(uri: Uri) {
        if (unavailable()) return
        status.update { it.copy(busy = true, message = "sync.processing_file") }
        scope.launch { mutex.withLock {
            try {
                require(uri.scheme == "content")
                // Export must never reset the causal journal of an attached document.
                val linked = config?.uri?.let(Uri::parse)
                require(linked == null || (uri != linked && !(uri.authority == linked.authority &&
                    android.provider.DocumentsContract.isDocumentUri(contextForDocuments, uri) &&
                    android.provider.DocumentsContract.isDocumentUri(contextForDocuments, linked) &&
                    android.provider.DocumentsContract.getDocumentId(uri) == android.provider.DocumentsContract.getDocumentId(linked))))
                write(uri, exportSettings(encoded(model.local.value)))
                status.update { it.copy(message = "sync.export_complete") }
            } catch (_: Exception) { status.update { it.copy(message = "sync.export_failed") } }
            finally { status.update { it.copy(busy = false) } }
        } }
    }
    @Synchronized fun importFile(uri: Uri) {
        if (unavailable()) return
        val generation = importGeneration
        importing = true
        status.update { it.copy(busy = true, message = "sync.processing_file") }
        scope.launch { mutex.withLock {
            try {
                require(uri.scheme == "content")
                val document = read(uri)
                val preview = json.decodeFromString<LocalState>(importSettings(encoded(LocalState()), document))
                synchronized(this@RustFileSync) {
                    if (generation == importGeneration) {
                        pendingImport = document
                        status.update { it.copy(pendingImportCount = preview.hosts.size, message = "sync.import_review") }
                    }
                }
            } catch (_: Exception) {
                synchronized(this@RustFileSync) {
                    if (generation == importGeneration) status.update { it.copy(message = "sync.import_failed") }
                }
            } finally { importing = false; status.update { it.copy(busy = false) } }
        } }
    }
    @Synchronized override fun confirmImport() {
        val document = pendingImport ?: return
        if (status.value.busy || model.hostEditing) return
        status.update { it.copy(busy = true) }
        scope.launch { mutex.withLock {
            try {
                model.modify { current -> json.decodeFromString<LocalState>(importSettings(encoded(current), document)) }
                status.update { it.copy(message = "sync.import_complete") }
            } catch (_: Exception) { status.update { it.copy(message = "sync.import_save_failed") } }
            finally {
                pendingImport = null
                status.update { it.copy(busy = false, pendingImportCount = null) }
            }
        } }
    }
    @Synchronized override fun cancelImport() {
        importGeneration++
        val canceled = pendingImport != null || importing
        pendingImport = null
        status.update { it.copy(pendingImportCount = null, message = if (canceled) "sync.import_canceled" else it.message) }
    }
    override val status = MutableStateFlow(CloudSyncStatus())

    @Serializable private data class Config(val uri: String, val name: String, val cache: String? = null)
    @Serializable private data class Exchange(val cache: String, val local: LocalState, val document: String, val conflicts: List<SyncConflict>)
    init {
        try {
            if (checkpoint.exists()) {
                require(checkpoint.length() <= MAX_BYTES * 2)
                config = json.decodeFromString<Config?>(checkpoint.readText())
            }
            status.value = CloudSyncStatus(connected = config != null, location = config?.name.orEmpty(), message = if (config == null) "common.disconnected" else "sync.waiting_check")
        } catch (_: Exception) { status.value = CloudSyncStatus(message = "sync.connection_read_failed_reselect_file") }
        scope.launch {
            while (isActive) {
                delay(15_000)
                if (model.foreground) sync()
            }
        }
    }
    private fun save(value: Config?) {
        directory.mkdirs()
        val temp = File(directory, ".sync-${UUID.randomUUID()}.tmp")
        try {
            temp.outputStream().use { stream -> stream.write(json.encodeToString(Config.serializer().nullable, value).toByteArray()); stream.fd.sync() }
            try { Files.move(temp.toPath(), checkpoint.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING) }
            catch (_: java.nio.file.AtomicMoveNotSupportedException) { Files.move(temp.toPath(), checkpoint.toPath(), StandardCopyOption.REPLACE_EXISTING) }
            config = value
        } finally { temp.delete() }
    }
    private fun read(uri: Uri): String {
        val bytes = requireNotNull(resolver.openInputStream(uri)).use { it.readNBytes(MAX_BYTES + 1) }
        require(bytes.size <= MAX_BYTES && bytes.isNotEmpty())
        return bytes.decodeToString(throwOnInvalidSequence = true)
    }
    private fun write(uri: Uri, document: String) {
        // "wt" truncates explicitly; "w" is provider-dependent. Keep document URI.
        requireNotNull(resolver.openOutputStream(uri, "wt")).use { it.write(document.toByteArray(Charsets.UTF_8)) }
    }
    private fun exchange(value: Config, local: LocalState, remote: String, resolution: SyncResolution?): Exchange = json.decodeFromString(
        exchangeSync(value.cache, json.encodeToString(LocalState.serializer(), local), remote,
            json.encodeToString(kotlinx.serialization.builtins.ListSerializer(SyncResolution.serializer()), listOfNotNull(resolution))))

    @Synchronized fun connect(uri: Uri, flags: Int, create: Boolean) {
        if (unavailable()) return
        status.update { it.copy(busy = true, message = "sync.connecting_file") }
        scope.launch { mutex.withLock {
            var granted = false
            try {
                require(uri.scheme == "content")
                val access = flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
                require(access == (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION))
                resolver.takePersistableUriPermission(uri, access)
                granted = true
                if (create) write(uri, newSyncDocument())
                val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                    if (cursor.moveToFirst()) cursor.getString(0) else null
                } ?: "dgl-sync.json"
                val candidate = Config(uri.toString(), name, config?.takeIf { it.uri == uri.toString() }?.cache)
                // Validate before replacing an existing connection or touching data.
                exchange(candidate, model.local.value, read(uri), null)
                val previous = config
                save(candidate)
                if (previous != null && previous.uri != candidate.uri) release(Uri.parse(previous.uri))
                status.value = CloudSyncStatus(connected = true, busy = true, location = name)
                synchronize(null)
            } catch (_: Exception) {
                if (granted && config?.uri != uri.toString()) release(uri)
                status.update { it.copy(connected = config != null, location = config?.name.orEmpty(), message = "sync.connect_failed") }
            } finally { status.update { it.copy(busy = false) } }
        } }
    }
    @Synchronized override fun sync(resolution: SyncResolution?) {
        if (unavailable() || config == null || model.hostEditing) return
        status.update { it.copy(busy = true, message = "sync.checking_file") }
        scope.launch { mutex.withLock {
            try { synchronize(resolution) }
            catch (_: Exception) { status.update { it.copy(message = "sync.sync_failed") } }
            finally { status.update { it.copy(busy = false) } }
        } }
    }
    private fun synchronize(resolution: SyncResolution?) {
        var selected = resolution
        repeat(3) {
            val value = requireNotNull(config)
            val uri = Uri.parse(value.uri)
            val before = read(uri)
            val out = exchange(value, model.local.value, before, selected)
            save(value.copy(cache = out.cache)) // persist before any cloud write
            selected = null
            if (read(uri) != before) return@repeat
            if (out.document != before) write(uri, out.document)
            if (model.hostEditing) {
                status.update { it.copy(message = "sync.pending_until_edit_done") }
                return
            }
            // Include local edits made during provider I/O and preserve local-only
            // fields. A checkpoint retained before apply permits safe retry.
            var applied: LocalState? = null
            model.modify { current ->
                val final = exchange(requireNotNull(config), current, out.document, null)
                save(requireNotNull(config).copy(cache = final.cache))
                status.value = status.value.copy(conflicts = final.conflicts,
                    message = if (final.conflicts.isEmpty()) "sync.file_updated" else "sync.conflicts_hint")
                applied = final.local
                final.local
            }
            val saved = requireNotNull(config)
            save(saved.copy(cache = acknowledgeSync(requireNotNull(saved.cache), json.encodeToString(LocalState.serializer(), requireNotNull(applied)))))
            return
        }
        error("Concurrent file change")
    }
    private fun release(uri: Uri) { runCatching { resolver.releasePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION) } }
    @Synchronized override fun disconnect() {
        if (unavailable()) return
        status.update { it.copy(busy = true) }
        scope.launch { mutex.withLock {
            try {
                val previous = config
                save(null)
                previous?.let { release(Uri.parse(it.uri)) }
                status.value = CloudSyncStatus(message = "sync.disconnected_file_kept")
            } catch (_: Exception) { status.update { it.copy(busy = false, message = "sync.disconnect_save_failed") } }
        } }
    }
    override fun close() { scope.cancel() }
    private companion object { const val MAX_BYTES = 16 * 1024 * 1024 }
}
