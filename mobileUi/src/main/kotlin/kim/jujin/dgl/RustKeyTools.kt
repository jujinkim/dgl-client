package kim.jujin.dgl

import kim.jujin.rovessh.GeneratedKey
import kim.jujin.rovessh.KeyTools
import kim.jujin.rovessh.core.Credential

object RustKeyTools : KeyTools {
    override fun ed25519(): GeneratedKey {
        val key = uniffi.dgl_core.generateKey()
        return GeneratedKey(Credential(privateKey = key.privateKey), key.publicKey)
    }

    override fun publicKey(credential: Credential): String =
        uniffi.dgl_core.publicKey(credential.privateKey, credential.passphrase)
}
