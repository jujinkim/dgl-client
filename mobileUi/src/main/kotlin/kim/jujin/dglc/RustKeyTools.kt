package kim.jujin.dglc

import kim.jujin.dglc.GeneratedKey
import kim.jujin.dglc.KeyTools
import kim.jujin.dglc.core.Credential

object RustKeyTools : KeyTools {
    override fun ed25519(): GeneratedKey {
        val key = uniffi.dgl_core.generateKey()
        return GeneratedKey(Credential(privateKey = key.privateKey), key.publicKey)
    }

    override fun publicKey(credential: Credential): String =
        uniffi.dgl_core.publicKey(credential.privateKey, credential.passphrase)
}
