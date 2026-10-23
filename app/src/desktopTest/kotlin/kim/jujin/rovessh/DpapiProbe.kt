package kim.jujin.rovessh

import com.sun.jna.platform.win32.Win32Exception
import kim.jujin.rovessh.core.Credential
import java.io.File
import java.security.MessageDigest
import java.util.UUID

/** Test-only process entry point. No production API or credential format changes. */
object DpapiProbe {
    @JvmStatic fun main(args: Array<String>) {
        check(args.size == 3)
        val (mode, path, reference) = args
        val directory = File(path)
        val store = WindowsCredentialStore(directory)
        when (mode) {
            "write" -> {
                val password = UUID.randomUUID().toString()
                store.write(reference, Credential(password = password))
                check(store.read(reference)?.password == password)
                println("OWN_ROUNDTRIP_OK")
            }
            "deny" -> {
                // A successful raw read and matching hash distinguish DPAPI isolation from ACL denial.
                val bytes = directory.resolve("$reference.sealed").readBytes()
                val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
                    .joinToString("") { "%02x".format(it) }
                try {
                    store.read(reference)
                    error("Foreign user decrypted a credential")
                } catch (failure: Win32Exception) {
                    // ERROR_INVALID_DATA is CryptUnprotectData's wrong-user rejection.
                    check(failure.errorCode == 13) { "Unexpected Windows failure code: ${failure.errorCode}" }
                    println("FOREIGN_READ_SHA256=$digest")
                    println("DPAPI_DENIED=13")
                }
            }
            else -> error("Unknown probe mode")
        }
    }
}
