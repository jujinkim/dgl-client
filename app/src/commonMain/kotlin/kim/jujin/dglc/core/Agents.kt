package kim.jujin.dglc.core

import kotlinx.serialization.Serializable

@Serializable data class AgentNotificationSettings(val questions: Boolean = true, val responses: Boolean = true, val errors: Boolean = true, val os: Boolean = false)
@Serializable data class AgentEvent(val id: String, val seq: Long, val server: String, val run: String, val actor: String,
    val session: String, val window: String, val pane: String, val agent: String, val kind: String, val event: String, val at: Long)
@Serializable data class AgentObservation(val session: String, val window: String, val pane: String, val run: String,
    val actor: String, val agent: String, val last: AgentEvent? = null, val supported: List<String> = emptyList(),
    /** Mirrors Rust `Observation.status`: the core already dropped an invalid status. */
    val status: AgentStatus? = null)

/** Which agent status parts are shown; mirrors Rust `AgentStatusSettings`. Not synced. */
@Serializable data class AgentStatusSettings(val statusBar: Boolean = true, val sidebar: Boolean = true, val account: Boolean = true,
    val usage: Boolean = true, val context: Boolean = true, val breakdown: Boolean = true, val cache: Boolean = true,
    val model: Boolean = true, val effort: Boolean = true, val cost: Boolean = false)

/** Mirrors Rust `agent::AgentStatus` with its wire names. `null` is unknown and never means zero. */
@Serializable data class AgentStatus(val at: Long? = null, val sources: List<String> = emptyList(), val model: ModelInfo? = null,
    val effort: String? = null, val thinking: Boolean? = null, val context: ContextUsage? = null, val cache: CacheUsage? = null,
    val cost_usd: Double? = null, val account: AccountInfo? = null, val limits: List<UsageLimit> = emptyList(), val limits_at: Long? = null,
    val alert: String? = null) {
    /** Mirrors Rust `AgentStatus::context_percent`: the CLI's own figure, else used / size. */
    fun contextPercent(): Double? = context?.let { c ->
        c.percent ?: if (c.used != null && c.size != null && c.size > 0) (c.used * 100.0 / c.size).coerceIn(0.0, 100.0) else null
    }
    /** Mirrors Rust `AgentStatus::cache_hit_ratio`: reported, else read / (input + read + write). */
    fun cacheHitRatio(): Double? = cache?.let { c ->
        c.hit_ratio ?: if (c.input != null && c.read != null) {
            val total = c.input + c.read + (c.write ?: 0)
            if (total > 0) c.read.toDouble() / total else null
        } else null
    }
    val accountKey: String? get() = account?.key
    fun isStale(nowMs: Long): Boolean = at == null || nowMs - at >= AGENT_STATUS_STALE_MS
    /** Mirrors Rust `AgentStatus::limits_sampled_at`: `limits_at`, else the newest sample of any key. */
    fun limitsSampledAt(): Long = limits_at ?: at ?: 0L
}
@Serializable data class ModelInfo(val id: String? = null, val name: String? = null)
@Serializable data class ContextUsage(val used: Long? = null, val size: Long? = null, val percent: Double? = null,
    val compact_percent: Double? = null, val breakdown: List<ContextPart> = emptyList(), val breakdown_total: Long? = null,
    val breakdown_window: Long? = null)
@Serializable data class ContextPart(val category: String = "", val name: String? = null, val tokens: Long? = null, val kind: String? = null) {
    /** Mirrors Rust `ContextPart::category_key`: unknown categories are `other`. */
    val categoryKey: String get() = if (category in AGENT_CONTEXT_CATEGORIES) category else "other"
    fun label(): String = when (categoryKey) {
        "system_prompt" -> t("agent_status.category.system_prompt"); "system_tools" -> t("agent_status.category.system_tools")
        "mcp_tools" -> t("agent_status.category.mcp_tools"); "custom_agents" -> t("agent_status.category.custom_agents")
        "memory_files" -> t("agent_status.category.memory_files"); "skills" -> t("agent_status.category.skills")
        "messages" -> t("agent_status.category.messages"); "autocompact_buffer" -> t("agent_status.category.autocompact_buffer")
        "free" -> t("agent_status.category.free"); "deferred" -> t("agent_status.category.deferred")
        else -> name?.takeIf { it.isNotBlank() } ?: t("agent_status.category.other")
    }
}
@Serializable data class CacheUsage(val input: Long? = null, val read: Long? = null, val write: Long? = null, val output: Long? = null,
    val ttl: String? = null, val last_at: Long? = null, val hit_ratio: Double? = null) {
    /** Mirrors Rust `CacheUsage::ttl_ms`: `5m`, `1h`, `30s`. */
    fun ttlMs(): Long? {
        val value = ttl?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        val unit = when (value.last()) { 's' -> 1_000L; 'm' -> 60_000L; 'h' -> 3_600_000L; else -> return null }
        val number = value.dropLast(1).takeIf { it.isNotEmpty() && it.all(Char::isDigit) }?.toLongOrNull() ?: return null
        return if (number > Long.MAX_VALUE / unit) null else number * unit
    }
    /** Estimated: last request time plus TTL; providers do not report expiry. */
    fun estimatedExpiryMs(): Long? { val last = last_at ?: return null; return ttlMs()?.let { last + it } }
}
@Serializable data class AccountInfo(val kind: String? = null, val plan: String? = null, val key: String? = null) {
    fun kindLabel(): String? = when (kind ?: return null) {
        "subscription" -> t("agent_status.account.subscription"); "api_key" -> t("agent_status.account.api_key")
        "cloud" -> t("agent_status.account.cloud"); "enterprise" -> t("agent_status.account.enterprise")
        else -> t("agent_status.account.other")
    }
}
@Serializable data class UsageLimit(val id: String = "", val label: String? = null, val used_percent: Double? = null,
    val remaining_fraction: Double? = null, val resets_at: Long? = null) {
    /** Mirrors Rust `UsageLimit::used`. */
    fun used(): Double? = used_percent ?: remaining_fraction?.let { 100.0 * (1.0 - it) }
    fun title(): String = when (id) {
        "five_hour" -> t("agent_status.limit.five_hour"); "seven_day" -> t("agent_status.limit.seven_day")
        "spend_limit" -> t("agent_status.limit.spend_limit"); else -> label?.takeIf { it.isNotBlank() } ?: id
    }
    /** Seconds until reset, computed locally; null when unknown. */
    fun resetsInS(nowMs: Long): Long? = resets_at?.let { maxOf(0L, it - nowMs / 1000) }
}
val AGENT_CONTEXT_CATEGORIES = listOf("system_prompt", "system_tools", "mcp_tools", "custom_agents", "memory_files", "skills",
    "messages", "autocompact_buffer", "free", "deferred", "other")
/** Mirrors Rust `STATUS_STALE_MS`. */
const val AGENT_STATUS_STALE_MS = 60L * 60 * 1000
fun agentAlertLabel(alert: String): String? = when (alert) {
    "rate_limit" -> t("agent_status.alert.rate_limit"); "billing_error" -> t("agent_status.alert.billing_error")
    "authentication_failed" -> t("agent_status.alert.authentication_failed"); else -> null
}
/** Mirrors Rust `agent::format_tokens`: `950`, `1.2k`, `84k`, `1.2M`, `12M`. */
fun formatTokens(n: Long): String {
    fun tenths(value: Long, unit: String) = if (value % 10 == 0L) "${value / 10}$unit" else "${value / 10}.${value % 10}$unit"
    if (n < 1_000) return maxOf(0L, n).toString()
    val k10 = (n + 50) / 100
    if (k10 < 100) return tenths(k10, "k")
    val k = (n + 500) / 1_000
    if (k < 1_000) return "${k}k"
    val m10 = (n + 50_000) / 100_000
    if (m10 < 100) return tenths(m10, "M")
    return "${(n + 500_000) / 1_000_000}M"
}
/** Mirrors Rust `agent::format_duration_s`: `45s`, `3m`, `2h 5m`, `4d`, `4d 3h`. */
fun formatDurationS(secs: Long): String {
    val s = maxOf(0L, secs)
    return when {
        s < 60 -> "${s}s"
        s < 3_600 -> "${s / 60}m"
        s < 86_400 -> { val h = s / 3_600; val m = s % 3_600 / 60; if (m == 0L) "${h}h" else "${h}h ${m}m" }
        else -> { val d = s / 86_400; val h = s % 86_400 / 3_600; if (h == 0L) "${d}d" else "${d}d ${h}h" }
    }
}
@Serializable data class AgentSnapshot(val server: String = "", val epoch: String = "", val cursor: Long = 0, val gap: Boolean = false,
    val observations: List<AgentObservation> = emptyList(), val events: List<AgentEvent> = emptyList(), val valid: Boolean = false, val observed_at: Long = 0) {
    fun current(now: Long) = valid && !gap && now - observed_at < 10_000
    /** Mirrors Rust `Snapshot::agent_in_pane`: a CLI process in a fresh poll; unknown state is null. */
    fun agentInPane(pane: String, now: Long): String? =
        if (!valid || now - observed_at >= 10_000) null else observations.firstOrNull { it.pane == pane }?.agent
    /** Mirrors Rust `Snapshot::statuses`: observations with a status; `@` window, `$` session or `%` pane filter. */
    fun statuses(filter: String? = null): List<AgentObservation> = observations.filter { o ->
        o.status != null && when {
            filter == null -> true
            filter.startsWith("@") -> o.window == filter
            filter.startsWith("$") -> o.session == filter
            else -> o.pane == filter
        }
    }
    /** Mirrors Rust `Snapshot::freshest_limits`: the newest limits sample of one account. */
    fun freshestLimits(agent: String, key: String): List<UsageLimit> = observations
        .filter { it.agent == agent }.mapNotNull { it.status }
        .filter { it.accountKey == key && it.limits.isNotEmpty() }
        .maxByOrNull { it.limitsSampledAt() }?.limits ?: emptyList()
    /** Mirrors Rust `Snapshot::limits_for`: the account's newest sample, else the observation's own. */
    fun limitsFor(observation: AgentObservation): List<UsageLimit> {
        val status = observation.status ?: return emptyList()
        val key = status.accountKey ?: return status.limits
        return freshestLimits(observation.agent, key).ifEmpty { status.limits }
    }
    /** Mirrors Rust `dgl_core::scroll::cli_in_pane` for every pane: observed, or tmux's current command. */
    fun cliPanes(panes: List<RemotePane>, now: Long): Set<String> =
        (if (!valid || now - observed_at >= 10_000) emptySet() else observations.map { it.pane }.toSet()) +
            panes.filter { agentForCommand(it.command) != null }.map { it.id }
}
/** Mirrors Rust `Agent::from_command`: a supported CLI's executable name as `ps` or tmux reports it. */
fun agentForCommand(command: String): String? = when (command.substringAfterLast('/')) {
    "codex" -> "Codex"; "claude" -> "Claude"; "agy" -> "Agy"; "grok" -> "Grok"; else -> null
}
@Serializable data class AgentNotice(val endpoint: String, val event: AgentEvent, val read: Boolean = false) {
    val id get() = "$endpoint:${event.id}"
    val key get() = "$endpoint:${event.server}:${event.pane}"
}
@Serializable data class AgentInboxResult(val recent: List<AgentNotice> = emptyList(), val deliver: List<AgentNotice> = emptyList())
fun agentPriority(kind: String) = when (kind) { "Question", "Approval" -> 0; "Error" -> 1; "Response" -> 2; "Activity" -> 3; else -> 4 }
fun agentName(agent: String) = when (agent) { "Agy" -> "AGY"; "Grok" -> "Grok Build"; else -> agent }
fun agentMessage(kind: String) = when (kind) {
    "Question" -> t("agent.question_reported"); "Approval" -> t("agent.approval_request_reported"); "Response" -> t("agent.response_end_signal_received")
    "Error" -> t("agent.explicit_error_reported"); "Activity" -> t("agent.activity_event_received"); else -> t("agent.run_ended_interrupted")
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
    val label = if (current(now) && priority != null) tr("agent.unread_events", agentMessage(priority.event.kind), unread.size)
        else if (!current(now) || representative == null) t("agent.status_unknown")
        else agentMessage(representative.kind) + " ${events.count { agentPriority(it.kind) == agentPriority(representative.kind) }}"
    val kind = if (current(now)) priority?.event?.kind ?: representative?.kind else null
    return PanelAgentState(observations, unread, label, kind)
}
