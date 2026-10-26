package kim.jujin.dglc

import kim.jujin.dglc.core.*

interface AgentInbox {
    fun tick(sources: List<Pair<String, AgentSnapshot>>, visible: Set<String>, settings: AgentNotificationSettings): AgentInboxResult
    fun markRead(id: String)
    fun close() {}
}
object NoAgentInbox : AgentInbox {
    override fun tick(sources: List<Pair<String, AgentSnapshot>>, visible: Set<String>, settings: AgentNotificationSettings) = AgentInboxResult()
    override fun markRead(id: String) {}
}
val ClientConnection.agentEndpoint: String get() = "${host.username}@${host.endpoint}"
