package kim.jujin.rovessh

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dgl.RustFileSync
import kim.jujin.dgl.RustKeyTools
import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.LocalRepository
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class FileSyncTest {
    @Test fun persistedDocumentTransportMergesProtectsSecretsAndRetriesFailedWrites() = runBlocking {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val uri = Uri.parse("content://kim.jujin.rovessh.test.sync/${UUID.randomUUID()}")
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
