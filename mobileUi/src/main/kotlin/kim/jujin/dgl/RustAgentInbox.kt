package kim.jujin.dgl

import kim.jujin.rovessh.AgentInbox
import kim.jujin.rovessh.core.*
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.SetSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.*
import uniffi.dgl_core.NativeAgentInbox

class RustAgentInbox : AgentInbox {
    private val native = NativeAgentInbox()
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
    override fun tick(sources: List<Pair<String, AgentSnapshot>>, visible: Set<String>, settings: AgentNotificationSettings): AgentInboxResult {
        val input = buildJsonArray { sources.forEach { (endpoint, snapshot) -> add(buildJsonArray { add(endpoint); add(json.encodeToJsonElement(AgentSnapshot.serializer(), snapshot)) }) } }
        return json.decodeFromString(AgentInboxResult.serializer(), native.tick(input.toString(), json.encodeToString(SetSerializer(String.serializer()), visible), json.encodeToString(AgentNotificationSettings.serializer(), settings)))
    }
    override fun markRead(id: String) = native.markRead(id)
    override fun close() = native.close()
}
