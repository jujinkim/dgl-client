package kim.jujin.dglc

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.RustFileSync
import kim.jujin.dglc.RustKeyTools
import kim.jujin.dglc.core.*
import kim.jujin.dglc.storage.LocalRepository
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class FileSyncTest {
    @Test fun oneShotFilesPreviewMergeWithoutPersistentPermissionOrReplacingSync() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val resolver = context.contentResolver
        val source = Uri.parse("content://${context.packageName}.test.sync/${UUID.randomUUID()}")
        val linked = Uri.parse("content://${context.packageName}.test.sync/${UUID.randomUUID()}")
        fun call(uri: Uri, method: String, data: String? = null, enabled: Boolean = false) = resolver.call(uri, method, uri.toString(), Bundle().apply { putString("data", data); putBoolean("enabled", enabled) })
        val root = File(context.noBackupFilesDir, "transfer-test-${UUID.randomUUID()}")
        val credentials = object : CredentialStore {
            override fun read(reference: String): Credential? = null
            override fun write(reference: String, credential: Credential) = Unit
            override fun delete(reference: String) = Unit
        }
        val app = AppModel(LocalRepository(root), credentials, "test", RustKeyTools) { _, _, _, _ -> error("No network") }
        val sync = RustFileSync(context, root, app)
        suspend fun idle() = withTimeout(10_000) { sync.status.first { !it.busy } }
        try {
            call(linked, "write", """{"format":"dgl-sync","version":2,"dataset":"one-shot-test","changes":{}}"""); call(linked, "grant")
            sync.connect(linked, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION, false); idle()
            assertTrue(sync.status.value.connected)
            val checkpoint = File(root, "cloud-sync.json").readText()
            app.update(LocalState(hosts = listOf(HostProfile("shared", "incoming", "example.test", username = "user", credentialRef = "private-reference")), preferences = Preferences(theme = "dark")))
            val grants = resolver.persistedUriPermissions.map { it.uri }.toSet()
            sync.exportFile(source); idle()
            assertEquals("내보내기 완료", sync.status.value.message)
            val exported = call(source, "read")!!.getString("data")!!
            assertFalse(exported.contains("private-reference"))
            assertEquals(grants, resolver.persistedUriPermissions.map { it.uri }.toSet())
            app.update(LocalState(hosts = listOf(HostProfile("local", "existing", "local.test", username = "user"))))
            val before = app.local.value
            sync.importFile(source); idle()
            assertEquals(1, sync.status.value.pendingImportCount)
            assertEquals(before, app.local.value)
            sync.sync(); assertFalse(sync.status.value.busy)
            sync.cancelImport(); assertNull(sync.status.value.pendingImportCount)
            assertEquals(before, app.local.value)
            val savedBefore = File(root, "settings.json").readText()
            File(root, "settings.tmp").mkdir()
            try {
                sync.importFile(source); idle(); sync.confirmImport(); idle()
                assertTrue(sync.status.value.message.contains("저장하지 못했습니다"))
                assertEquals(before, app.local.value)
                assertEquals(savedBefore, File(root, "settings.json").readText())
            } finally { File(root, "settings.tmp").deleteRecursively() }
            sync.importFile(source); idle(); sync.confirmImport(); idle()
            assertEquals(setOf("local", "shared"), app.local.value.hosts.map { it.id }.toSet())
            assertEquals("dark", app.local.value.preferences.theme)
            assertEquals(exported, call(source, "read")!!.getString("data"))
            assertEquals(checkpoint, File(root, "cloud-sync.json").readText())
            assertEquals(grants, resolver.persistedUriPermissions.map { it.uri }.toSet())
            val linkedBefore = call(linked, "read")!!.getString("data")
            sync.exportFile(linked); idle()
            assertTrue(sync.status.value.message.contains("실패"))
            assertEquals(linkedBefore, call(linked, "read")!!.getString("data"))
            val imported = app.local.value
            call(source, "write", "invalid"); sync.importFile(source); idle()
            assertNull(sync.status.value.pendingImportCount); assertEquals(imported, app.local.value)
            call(source, "fail", enabled = true); sync.exportFile(source); idle()
            assertTrue(sync.status.value.message.contains("실패"))
            sync.sync(); idle(); assertTrue(sync.status.value.connected)
        } finally {
            sync.cancelImport(); sync.disconnect(); idle(); sync.close(); app.close()
            call(source, "delete"); call(linked, "delete"); root.deleteRecursively()
        }
    }
    @Test fun persistedDocumentTransportMergesProtectsSecretsAndRetriesFailedWrites() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val uri = Uri.parse("content://${context.packageName}.test.sync/${UUID.randomUUID()}")
        val resolver = context.contentResolver
        fun call(method: String, data: String? = null, enabled: Boolean = false) = resolver.call(uri, method, uri.toString(), Bundle().apply { putString("data", data); putBoolean("enabled", enabled) })
        call("write", """{"format":"dgl-sync","version":1,"dataset":"test-dataset","changes":{}}""")
        call("grant")
        val root = File(context.noBackupFilesDir, "sync-test-${UUID.randomUUID()}")
        fun model(name: String): AppModel {
            val credentials = object : CredentialStore {
                override fun read(reference: String): Credential? = if (reference == "local-secret-reference") Credential(password = "secret-password") else null
                override fun write(reference: String, credential: Credential) = Unit
                override fun delete(reference: String) = Unit
            }
            return AppModel(LocalRepository(File(root, name)), credentials, "test", RustKeyTools) { _, _, _, _ -> error("No network connections in sync test") }
        }
        val a = model("a"); val b = model("b")
        val sa = RustFileSync(context, File(root, "a"), a)
        var sb = RustFileSync(context, File(root, "b"), b)
        suspend fun idle(sync: RustFileSync) = withTimeout(10_000) { sync.status.first { !it.busy } }
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        try {
            a.update(LocalState(hosts = listOf(HostProfile("test-host", "host", "example.test", username = "user", credentialRef = "local-secret-reference")),
                trust = mapOf("private-trust" to "private-fingerprint"), preferences = Preferences(theme = "dark")))
            sa.connect(uri, flags, false); assertTrue(idle(sa).message, sa.status.value.connected)
            val exported = call("read")!!.getString("data")!!
            for (secret in listOf("local-secret-reference", "secret-password", "private-trust", "private-fingerprint", "credentialRef", "volumeUp")) assertFalse(secret, exported.contains(secret))
            sb.connect(uri, flags, false); idle(sb)
            assertEquals("dark", b.local.value.preferences.theme)
            assertEquals(1, b.local.value.hosts.size)
            assertNotEquals("local-secret-reference", b.local.value.hosts.single().credentialRef)
            assertTrue(b.local.value.trust.isEmpty())
            b.modify { it.copy(hosts = it.hosts.map { h -> h.copy(favorite = true) }) }
            b.hostEditing = true; sb.sync(); assertFalse(sb.status.value.busy)
            sa.sync(); idle(sa); assertFalse(a.local.value.hosts.single().favorite)
            b.hostEditing = false; sb.sync(); idle(sb)
            sa.sync(); idle(sa); assertTrue(a.local.value.hosts.single().favorite)
            assertEquals("local-secret-reference", a.local.value.hosts.single().credentialRef)
            // Concurrent edits retain both values, including across adapter restart.
            a.modify { it.copy(hosts = it.hosts.map { h -> h.copy(alias = "from-a") }) }
            b.modify { it.copy(hosts = it.hosts.map { h -> h.copy(alias = "from-b") }) }
            sa.sync(); idle(sa); sb.sync(); idle(sb)
            assertEquals(1, sb.status.value.conflicts.size)
            sb.close(); sb = RustFileSync(context, File(root, "b"), b); sb.sync(); idle(sb)
            val conflict = sb.status.value.conflicts.single()
            val choice = conflict.choices.single { it.label.startsWith("from-b") }
            sb.sync(SyncResolution(conflict.key, choice.revision)); idle(sb); sa.sync(); idle(sa)
            assertEquals("from-b", a.local.value.hosts.single().alias)
            call("fail", enabled = true)
            b.modify { it.copy(preferences = it.preferences.copy(theme = "light")) }; sb.sync(); idle(sb)
            assertTrue(sb.status.value.message.contains("실패"))
            sb.close(); sb = RustFileSync(context, File(root, "b"), b)
            call("fail", enabled = false); sb.sync(); idle(sb); sa.sync(); idle(sa)
            assertEquals("light", a.local.value.preferences.theme)
            a.modify { it.copy(hosts = emptyList()) }; sa.sync(); idle(sa); sb.sync(); idle(sb)
            assertTrue(b.local.value.hosts.isEmpty())
            call("write", "invalid"); sb.sync(); idle(sb)
            assertEquals("invalid", call("read")!!.getString("data"))
            assertTrue(sb.status.value.message.contains("실패"))
            sb.disconnect(); idle(sb); assertFalse(sb.status.value.connected)
            assertEquals("invalid", call("read")!!.getString("data"))
        } finally {
            sa.close(); sb.close(); a.close(); b.close(); call("delete"); root.deleteRecursively()
        }
    }
}
