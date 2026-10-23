package kim.jujin.rovessh

import com.sun.jna.platform.win32.Crypt32Util
import kim.jujin.rovessh.storage.ProtectedFileStore
import java.io.File

class WindowsCredentialStore(directory: File) : ProtectedFileStore(directory) {
    init { check(System.getProperty("os.name").startsWith("Windows")) }
    // Crypt32Util's defaults use CRYPTPROTECT_UI_FORBIDDEN and CURRENT_USER scope.
    override fun protect(plain: ByteArray): ByteArray = Crypt32Util.cryptProtectData(plain)
    override fun unprotect(ciphertext: ByteArray): ByteArray = Crypt32Util.cryptUnprotectData(ciphertext)
}
