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

object KeyGeneration {
    data class Generated(val credential: Credential, val publicKey: String)
    fun publicKey(credential: Credential): String {
        kim.jujin.rovessh.ssh.SshCrypto.initialize()
        require(credential.privateKey.length in 32..131072)
        val ssh = net.schmizz.sshj.SSHClient()
        try {
            val provider = ssh.loadKeys(credential.privateKey, null,
                net.schmizz.sshj.userauth.password.PasswordUtils.createOneOff(credential.passphrase.toCharArray()))
            provider.private // Validate the passphrase before exporting its public half.
            val blob = net.schmizz.sshj.common.Buffer.PlainBuffer().putPublicKey(provider.public).compactData
            val type = java.io.DataInputStream(blob.inputStream()).use { input ->
                val count = input.readInt()
                require(count in 1..64)
                String(ByteArray(count).also(input::readFully), Charsets.US_ASCII)
            }
            return "$type ${Base64.getEncoder().encodeToString(blob)} dgl-client"
        } finally { ssh.close() }
    }
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
