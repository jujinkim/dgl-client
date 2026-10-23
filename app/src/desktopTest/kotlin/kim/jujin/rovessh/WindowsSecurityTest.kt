package kim.jujin.rovessh

import kim.jujin.rovessh.core.Credential
import org.junit.Assume.assumeTrue
import java.nio.file.Files
import java.util.UUID
import kotlin.test.*

class WindowsSecurityTest {
    @Test fun currentUserDpapiRoundTripTamperAndPathValidation() {
        assumeTrue("Windows execution required", System.getProperty("os.name").startsWith("Windows"))
        val directory = Files.createTempDirectory("dgl-dpapi-test-").toFile()
        try {
            val store = WindowsCredentialStore(directory)
            val password = UUID.randomUUID().toString()
            store.write("test", Credential(password))
            assertTrue(password == store.read("test")?.password, "Protected credential round trip")
            val file = directory.resolve("test.sealed")
            assertFalse(file.readBytes().toString(Charsets.UTF_8).contains(password))
            val bytes = file.readBytes(); bytes[bytes.lastIndex] = (bytes.last().toInt() xor 1).toByte(); file.writeBytes(bytes)
            assertFails { store.read("test") }
            assertFailsWith<IllegalArgumentException> { store.read("../invalid") }
            store.delete("test"); assertNull(store.read("test"))
        } finally { directory.deleteRecursively() }
    }
}
