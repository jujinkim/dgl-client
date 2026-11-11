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
        status.update { it.copy(busy = false, message = if (failed) "파일 선택창을 열지 못했습니다." else it.message) }
    }
    @Synchronized fun exportFile(uri: Uri) {
        if (unavailable()) return
        status.update { it.copy(busy = true, message = "파일 처리 중…") }
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
                status.update { it.copy(message = "내보내기 완료") }
            } catch (_: Exception) { status.update { it.copy(message = "내보내기 실패. 파일 권한을 확인하고 연결 중인 동기화 파일과 다른 파일을 선택하세요.") } }
            finally { status.update { it.copy(busy = false) } }
        } }
    }
    @Synchronized fun importFile(uri: Uri) {
        if (unavailable()) return
        val generation = importGeneration
        importing = true
        status.update { it.copy(busy = true, message = "파일 처리 중…") }
        scope.launch { mutex.withLock {
            try {
                require(uri.scheme == "content")
                val document = read(uri)
                val preview = json.decodeFromString<LocalState>(importSettings(encoded(LocalState()), document))
                synchronized(this@RustFileSync) {
                    if (generation == importGeneration) {
                        pendingImport = document
                        status.update { it.copy(pendingImportCount = preview.hosts.size, message = "가져올 파일을 확인하세요.") }
                    }
                }
            } catch (_: Exception) {
                synchronized(this@RustFileSync) {
                    if (generation == importGeneration) status.update { it.copy(message = "가져오기 실패. 파일 형식·버전·크기·미해결 충돌을 확인하세요.") }
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
                status.update { it.copy(message = "가져오기 완료") }
            } catch (_: Exception) { status.update { it.copy(message = "설정을 저장하지 못했습니다. 가져오기를 적용하지 않았습니다.") } }
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
        status.update { it.copy(pendingImportCount = null, message = if (canceled) "가져오기를 취소했습니다." else it.message) }
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
            status.value = CloudSyncStatus(connected = config != null, location = config?.name.orEmpty(), message = if (config == null) "연결 안 됨" else "연결한 파일 확인 대기")
        } catch (_: Exception) { status.value = CloudSyncStatus(message = "연결 정보를 읽지 못했습니다. 파일을 다시 선택하세요.") }
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
        status.update { it.copy(busy = true, message = "파일 연결 중…") }
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
                status.update { it.copy(connected = config != null, location = config?.name.orEmpty(), message = "파일 연결 실패. 읽기·쓰기가 가능한 dgl-sync.json을 선택하세요.") }
            } finally { status.update { it.copy(busy = false) } }
        } }
    }
    @Synchronized override fun sync(resolution: SyncResolution?) {
        if (unavailable() || config == null || model.hostEditing) return
        status.update { it.copy(busy = true, message = "파일 확인 중…") }
        scope.launch { mutex.withLock {
            try { synchronize(resolution) }
            catch (_: Exception) { status.update { it.copy(message = "파일 동기화 실패. 동기화 앱·파일 권한·파일 형식을 확인하세요. 변경 기록은 기기에 보존됩니다.") } }
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
                status.update { it.copy(message = "호스트 편집이 끝나면 받은 변경을 적용합니다.") }
                return
            }
            // Include local edits made during provider I/O and preserve local-only
            // fields. A checkpoint retained before apply permits safe retry.
            var applied: LocalState? = null
            model.modify { current ->
                val final = exchange(requireNotNull(config), current, out.document, null)
                save(requireNotNull(config).copy(cache = final.cache))
                status.value = status.value.copy(conflicts = final.conflicts,
                    message = if (final.conflicts.isEmpty()) "파일 반영 완료 · 클라우드 전송은 동기화 앱에서 확인하세요." else "동시 수정된 항목을 아래에서 선택하세요.")
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
                status.value = CloudSyncStatus(message = "연결 해제됨 · 공유 파일은 유지됩니다.")
            } catch (_: Exception) { status.update { it.copy(busy = false, message = "연결 해제 정보를 저장하지 못했습니다.") } }
        } }
    }
    override fun close() { scope.cancel() }
    private companion object { const val MAX_BYTES = 16 * 1024 * 1024 }
}
