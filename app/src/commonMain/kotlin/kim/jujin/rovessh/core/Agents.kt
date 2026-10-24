package kim.jujin.rovessh.core

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
fun agentMessage(kind: String) = when (kind) {
    "Question" -> "? 질문이 발생했습니다"; "Approval" -> "◇ 승인 요청이 발생했습니다"; "Response" -> "○ 응답 종료 신호를 받았습니다"
    "Error" -> "! 명시적 오류를 받았습니다"; "Activity" -> "↻ 작업 사건을 받았습니다"; else -> "— 실행 종료/중단 신호를 받았습니다"
}
