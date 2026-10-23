package kim.jujin.rovessh.storage

import kim.jujin.rovessh.core.*
import kotlinx.serialization.json.Json
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import org.bouncycastle.crypto.generators.Ed25519KeyPairGenerator
import org.bouncycastle.crypto.params.Ed25519KeyGenerationParameters
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.util.OpenSSHPrivateKeyUtil
import org.bouncycastle.util.io.pem.PemObject
import org.bouncycastle.openssl.jcajce.JcaPEMWriter
import java.io.StringWriter
import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.security.SecureRandom
import java.util.Base64

class LocalRepository(private val directory: File) {
    private val file = File(directory, "settings.json")
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }
    @Synchronized fun read(): LocalState {
        if (!file.exists()) return LocalState()
        check(file.length() <= 1 shl 20) { "Settings file exceeds limit" }
        return json.decodeFromString<LocalState>(file.readText())
    }
    @Synchronized fun save(value: LocalState) {
        directory.mkdirs()
        val temporary = File(directory, "settings.tmp")
        temporary.writeText(json.encodeToString(LocalState.serializer(), value))
        try { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING) }
        catch (_: java.nio.file.AtomicMoveNotSupportedException) { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING) }
    }
}

object KeyGeneration {
    data class Generated(val credential: Credential, val publicKey: String)
    fun ed25519(): Generated {
        val generator = Ed25519KeyPairGenerator()
        generator.init(Ed25519KeyGenerationParameters(SecureRandom()))
        val pair = generator.generateKeyPair()
        val writer = StringWriter()
        JcaPEMWriter(writer).use { it.writeObject(PemObject("OPENSSH PRIVATE KEY", OpenSSHPrivateKeyUtil.encodePrivateKey(pair.private))) }
        val bytes = ByteArrayOutputStream()
        DataOutputStream(bytes).use { stream ->
            for (part in listOf("ssh-ed25519".toByteArray(), (pair.public as Ed25519PublicKeyParameters).encoded)) {
                stream.writeInt(part.size); stream.write(part)
            }
        }
        return Generated(Credential(privateKey = writer.toString()), "ssh-ed25519 ${Base64.getEncoder().encodeToString(bytes.toByteArray())} dgl-client")
    }
}

// Unsupported development platforms use explicitly ephemeral credentials, never plaintext fallback.
class MemoryCredentialStore : CredentialStore {
    private val values = mutableMapOf<String, Credential>()
    @Synchronized override fun read(reference: String) = values[reference]
    @Synchronized override fun write(reference: String, credential: Credential) { values[reference] = credential }
    @Synchronized override fun delete(reference: String) { values.remove(reference) }
}
