package kim.jujin.dglc

import kim.jujin.dglc.core.Credential

data class GeneratedKey(val credential: Credential, val publicKey: String)
interface KeyTools {
    fun ed25519(): GeneratedKey
    fun publicKey(credential: Credential): String
}
