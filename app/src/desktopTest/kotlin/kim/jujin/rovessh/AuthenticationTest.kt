package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.SshjTransport
import kim.jujin.rovessh.storage.KeyGeneration
import kotlinx.coroutines.runBlocking
import org.apache.sshd.server.SshServer
import org.apache.sshd.server.keyprovider.SimpleGeneratorHostKeyProvider
import java.util.UUID
import kotlin.test.*

class AuthenticationTest {
    @Test fun passwordSuccessFailureAndGeneratedEd25519Key() = runBlocking<Unit> {
        val password = UUID.randomUUID().toString()
        val server = SshServer.setUpDefaultServer().apply {
            host = "127.0.0.1"; port = 0
            keyPairProvider = SimpleGeneratorHostKeyProvider().apply { algorithm = "EC" }
            setPasswordAuthenticator { user, value, _ -> user == "synthetic" && value == password }
            // Test server has no command, filesystem, shell, or forwarding capabilities.
            setPublickeyAuthenticator { user, _, _ -> user == "synthetic" }
        }
        server.start()
        val ssh = SshjTransport()
        val host = HostProfile("password-test", "Synthetic", "127.0.0.1", server.port, "synthetic")
        try {
            val unknown = assertFailsWith<UnknownHostKey> { ssh.connect(host, Credential(password), null) }
            ssh.connect(host, Credential(password), unknown.fingerprint)
            ssh.close()
            assertEquals(Failure.Authentication, assertFailsWith<ClientFailure> { ssh.connect(host, Credential("invalid"), unknown.fingerprint) }.kind)
            val generated = KeyGeneration.ed25519()
            assertTrue(generated.publicKey.startsWith("ssh-ed25519 "))
            assertEquals(generated.publicKey, KeyGeneration.publicKey(generated.credential))
            assertFails { KeyGeneration.publicKey(Credential(privateKey = "invalid")) }
            ssh.connect(host.copy(auth = AuthKind.PrivateKey), generated.credential, unknown.fingerprint)
        } finally { ssh.close(); server.stop(true) }
    }
}
