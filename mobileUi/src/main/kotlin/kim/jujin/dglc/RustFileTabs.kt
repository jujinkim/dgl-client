package kim.jujin.dglc

import android.content.Context
import kim.jujin.dglc.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.serialization.json.*
import uniffi.dgl_core.CoreException
import uniffi.dgl_core.NativeFileTransfer
import uniffi.dgl_core.NativeFiles
import java.security.MessageDigest

internal fun HostProfile.trustReference() = "trust-" + MessageDigest.getInstance("SHA-256")
    .digest(endpoint.toByteArray()).joinToString("") { "%02x".format(it) }

internal class TrustRecordMismatch : Exception(t("connection.trust_record_mismatch"))

internal fun HostProfile.verifiedFingerprint(credentials: CredentialStore, local: LocalState): String? =
    local.trust[endpoint]?.also {
        if (credentials.read(trustReference())?.password != it) throw TrustRecordMismatch()
    }

class RustFileTabs(context: Context, private val model: AppModel) : FileTabs {
    private val documents = AndroidDocuments(context.applicationContext.contentResolver)
    private val json = Json { encodeDefaults = true; ignoreUnknownKeys = true }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private class Tab(val host: HostProfile, val pane: String?) {
        var native: NativeFiles? = null
        val jobs = mutableMapOf<Long, NativeFileTransfer>()
        val requests = mutableMapOf<Long, FileCopy>()
    }
    private val owners = mutableMapOf<FileTabKey, Tab>()
    private var nextJob = 0L
    override val tabs = MutableStateFlow<Map<FileTabKey, FileTabState>>(emptyMap())
    override val selected = MutableStateFlow<FileTabKey?>(null)

    private fun change(key: FileTabKey, f: (FileTabState) -> FileTabState) {
        tabs.value[key]?.let { tabs.value += key to f(it) }
    }
    override fun open(host: HostProfile, session: String?, pane: String?, name: String?) {
        val key = FileTabKey(host.id, session)
        if (key !in owners) {
            owners[key] = Tab(host, pane)
            val title = listOfNotNull("SFTP", host.alias, name ?: session).joinToString(" · ")
            tabs.value += key to FileTabState(title, loading = false)
            browse(key, null)
        }
        select(key)
    }
    override fun select(key: FileTabKey?) { selected.value = key }
    override fun toggle(key: FileTabKey, name: String) = change(key) { state ->
        state.copy(selected = if (name in state.selected) state.selected - name else state.selected + name)
    }
    private fun native(tab: Tab, key: FileTabKey): NativeFiles {
        val host = tab.host
        val cred = model.credentials.read(host.credentialRef) ?: error(t("host.credentials_hint"))
        val trusted = host.verifiedFingerprint(model.credentials, model.local.value)
        val secret = buildJsonObject {
            put("password", cred.password); put("privateKey", cred.privateKey); put("passphrase", cred.passphrase)
        }
        return NativeFiles(json.encodeToString(HostProfile.serializer(), host), secret.toString(), trusted, key.session, tab.pane)
    }
    override fun browse(key: FileTabKey, path: String?) {
        val tab = owners[key] ?: return
        if (tabs.value[key]?.loading == true) return
        change(key) { it.copy(loading = true, error = null, fingerprint = null) }
        scope.launch {
            try {
                val client = tab.native ?: withContext(Dispatchers.IO) { native(tab, key) }.also {
                    if (owners[key] !== tab) { it.close(); return@launch }
                    tab.native = it
                }
                val list = withContext(Dispatchers.IO) { json.decodeFromString(FileListing.serializer(), client.list(path)) }
                if (owners[key] === tab) change(key) { it.copy(listing = list, loading = false,
                    selected = if (list.path == it.listing.path) it.selected.intersect(list.entries.map { e -> e.name }.toSet()) else emptySet()) }
            } catch (e: CancellationException) { throw e }
            catch (e: Exception) {
                if (owners[key] === tab) change(key) { it.copy(loading = false, error = e.message,
                    fingerprint = if ((e as? CoreException.Failure)?.code == "HostKeyUnknown") tab.native?.fingerprint() else null) }
            }
        }
    }
    override fun trust(key: FileTabKey) {
        val tab = owners[key] ?: return
        val fingerprint = tabs.value[key]?.fingerprint ?: return
        try {
            val host = tab.host
            model.modify { old ->
                val known = old.trust[host.endpoint]
                check(known == null || known == fingerprint) { t("connection.trust_record_mismatch") }
                if (known == null) model.credentials.write(host.trustReference(), Credential(password = fingerprint))
                old.copy(trust = old.trust + (host.endpoint to fingerprint))
            }
            tab.native?.close(); tab.native = null
            browse(key, null)
        } catch (e: Exception) { change(key) { it.copy(error = e.message) } }
    }
    override fun copy(key: FileTabKey, request: FileCopy) {
        val tab = owners[key] ?: return
        val job = tab.native?.transfer(documents, request.upload, request.sources, request.destination) ?: return
        val id = ++nextJob
        tab.jobs[id] = job
        tab.requests[id] = request
        scope.launch {
            try {
                while (owners[key] === tab) {
                    val p = json.decodeFromString(FileProgress.serializer(), job.progress())
                    change(key) { it.copy(transfers = it.transfers + (id to p)) }
                    if (p.done) {
                        if (p.error == null) {
                            tab.requests.remove(id)
                            if (request.upload && tabs.value[key]?.listing?.path == request.destination) browse(key, request.destination)
                        }
                        break
                    }
                    delay(150)
                }
            } finally {
                tab.jobs.remove(id)?.close()
            }
        }
    }
    override fun cancel(key: FileTabKey, job: Long) { owners[key]?.jobs?.get(job)?.cancel() }
    override fun overwrite(key: FileTabKey, job: Long, expected: String) { owners[key]?.jobs?.get(job)?.overwrite(expected) }
    override fun retry(key: FileTabKey, job: Long) { owners[key]?.requests?.get(job)?.let { copy(key, it) } }
    override fun close(key: FileTabKey) {
        val tab = owners.remove(key) ?: return
        tab.jobs.values.forEach { it.cancel(); it.close() }; tab.jobs.clear()
        tab.native?.close()
        tabs.value -= key
        if (selected.value == key) selected.value = null
    }
    override fun shutdown() { owners.keys.toList().forEach(::close); scope.cancel() }
}
