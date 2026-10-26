package kim.jujin.dglc.storage

import kim.jujin.dglc.core.*
import kotlinx.serialization.json.*
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

abstract class ProtectedFileStore(private val directory: File) : CredentialStore {
    protected abstract fun protect(plain: ByteArray): ByteArray
    protected abstract fun unprotect(ciphertext: ByteArray): ByteArray
    private fun path(reference: String): File {
        require(reference.matches(Regex("[a-zA-Z0-9-]{1,80}")))
        return File(directory, "$reference.sealed")
    }
    @Synchronized override fun read(reference: String): Credential? {
        val file = path(reference)
        if (!file.exists()) return null
        check(file.length() <= 256 * 1024) { "Credential size limit exceeded" }
        val bytes = unprotect(file.readBytes())
        try {
            val value = Json.parseToJsonElement(bytes.decodeToString()).jsonObject
            return Credential(value.getValue("password").jsonPrimitive.content, value.getValue("privateKey").jsonPrimitive.content, value.getValue("passphrase").jsonPrimitive.content)
        } finally { bytes.fill(0) }
    }
    @Synchronized override fun write(reference: String, credential: Credential) {
        require(credential.privateKey.length <= 131072 && credential.password.length <= 4096 && credential.passphrase.length <= 4096)
        val file = path(reference)
        val bytes = buildJsonObject {
            put("password", credential.password); put("privateKey", credential.privateKey); put("passphrase", credential.passphrase)
        }.toString().encodeToByteArray()
        val sealed = try { protect(bytes) } finally { bytes.fill(0) }
        directory.mkdirs()
        val temporary = File(directory, file.name + ".tmp")
        temporary.writeBytes(sealed)
        try { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE) }
        catch (_: java.nio.file.AtomicMoveNotSupportedException) { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING) }
    }
    @Synchronized override fun delete(reference: String) {
        val file = path(reference)
        check(!file.exists() || file.delete()) { "Credential deletion failed" }
    }
}
