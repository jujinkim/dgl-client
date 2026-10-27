package kim.jujin.dglc.core

import kotlinx.serialization.Serializable

@Serializable data class AgentNotificationSettings(val questions: Boolean = true, val responses: Boolean = true, val errors: Boolean = true, val os: Boolean = false)
@Serializable data class AgentEvent(val id: String, val seq: Long, val server: String, val run: String, val actor: String,
    val session: String, val window: String, val pane: String, val agent: String, val kind: String, val event: String, val at: Long)
@Serializable data class AgentObservation(val session: String, val window: String, val pane: String, val run: String,
    val actor: String, val agent: String, val last: AgentEvent? = null, val supported: List<String> = emptyList())
@Serializable data class AgentSnapshot(val server: String = "", val epoch: String = "", val cursor: Long = 0, val gap: Boolean = false,
    val observations: List<AgentObservation> = emptyList(), val events: List<AgentEvent> = emptyList(), val valid: Boolean = false, val observed_at: Long = 0) {
    fun current(now: Long) = valid && !gap && now - observed_at < 10_000
}
@Serializable data class AgentNotice(val endpoint: String, val event: AgentEvent, val read: Boolean = false) {
    val id get() = "$endpoint:${event.id}"
    val key get() = "$endpoint:${event.server}:${event.pane}"
}
@Serializable data class AgentInboxResult(val recent: List<AgentNotice> = emptyList(), val deliver: List<AgentNotice> = emptyList())
fun agentPriority(kind: String) = when (kind) { "Question", "Approval" -> 0; "Error" -> 1; "Response" -> 2; "Activity" -> 3; else -> 4 }
fun agentName(agent: String) = when (agent) { "Agy" -> "AGY"; "Grok" -> "Grok Build"; else -> agent }
fun agentMessage(kind: String) = when (kind) {
    "Question" -> t("질문이 발생했습니다"); "Approval" -> t("승인 요청이 발생했습니다"); "Response" -> t("응답 종료 신호를 받았습니다")
    "Error" -> t("명시적 오류를 받았습니다"); "Activity" -> t("작업 사건을 받았습니다"); else -> t("실행 종료/중단 신호를 받았습니다")
}

/** A window (@ID) or pane (%ID) filter drives the button, unknown state, unread count, and details. */
data class PanelAgentState(val observations: List<AgentObservation>, val unread: List<AgentNotice>, val label: String, val kind: String?)
fun AgentSnapshot.panelState(pane: String?, endpoint: String, notices: List<AgentNotice>, now: Long): PanelAgentState {
    val observations = observations.filter { pane == null || (if (pane.startsWith("@")) it.window == pane else it.pane == pane) }
    val events = observations.mapNotNull { it.last }.filter { it.kind != "Ended" }
    val representative = events.minByOrNull { agentPriority(it.kind) }
    val unread = notices.filter { n -> !n.read && n.endpoint == endpoint && n.event.server == server &&
        (pane == null || (if (pane.startsWith("@")) pane == n.event.window else pane == n.event.pane)) && observations.any { it.pane == n.event.pane && it.run == n.event.run && it.last != null } }
    val priority = unread.minByOrNull { agentPriority(it.event.kind) }
    val label = if (current(now) && priority != null) tr("{0} · 읽지 않은 사건 {1}건", agentMessage(priority.event.kind), unread.size)
        else if (!current(now) || representative == null) t("상태 미확인")
        else agentMessage(representative.kind) + " ${events.count { agentPriority(it.kind) == agentPriority(representative.kind) }}"
    val kind = if (current(now)) priority?.event?.kind ?: representative?.kind else null
    return PanelAgentState(observations, unread, label, kind)
}
