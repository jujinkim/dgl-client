package kim.jujin.dglc.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*
import kotlin.math.roundToLong

// Agent status HUD: the status chip, the full panel (dialog on wide screens, bottom sheet on
// phones), the sidebar summary and the settings card body. Unknown values are never shown as 0.

/** One visible chip part; [field] names the [AgentStatusSettings] item it belongs to. */
internal data class AgentChipPart(val field: String, val text: String)

/** Parts dropped first when the chip is narrow; the context part is kept the longest. */
internal val AGENT_STATUS_CHIP_DROP_ORDER = listOf("cost", "cache", "effort", "usage", "model", "context")

/** Why a status item has no value. */
internal enum class AgentFieldState { Shown, Unknown, NotCollected, NotSupported, NoLimits }

internal fun agentPercentText(value: Double): String = value.roundToLong().toString()

/** The limit closest to exhaustion; limits with an unknown figure are skipped. */
internal fun agentTopLimit(limits: List<UsageLimit>): UsageLimit? = limits.filter { it.used() != null }.maxByOrNull { it.used()!! }

/**
 * The status the chip summarizes: the focused pane's agent (parent first), else the first observation
 * with a status, else any observation so its "not collected" state stays reachable.
 */
internal fun agentStatusFocus(snapshot: AgentSnapshot, pane: String?): AgentObservation? {
    val inPane = snapshot.observations.filter { pane != null && it.pane == pane }
        .sortedWith(compareByDescending<AgentObservation> { it.status != null }.thenByDescending { it.actor == "parent" })
    return inPane.firstOrNull { it.status != null }
        ?: snapshot.observations.filter { it.status != null }.sortedByDescending { it.actor == "parent" }.firstOrNull()
        ?: inPane.firstOrNull() ?: snapshot.observations.firstOrNull()
}

/** Panel order: the focused pane's agents first, then the rest in snapshot order. */
internal fun agentStatusOrder(snapshot: AgentSnapshot, pane: String?): List<AgentObservation> =
    snapshot.observations.sortedByDescending { pane != null && it.pane == pane }

/** Chip parts in display order, only enabled items with a known value. */
internal fun agentStatusChipParts(observation: AgentObservation, limits: List<UsageLimit>, settings: AgentStatusSettings): List<AgentChipPart> {
    val s = observation.status ?: return emptyList()
    return buildList {
        if (settings.model) (s.model?.name ?: s.model?.id)?.takeIf { it.isNotBlank() }?.let { add(AgentChipPart("model", it)) }
        if (settings.effort) (s.effort?.takeIf { it.isNotBlank() } ?: t("agent_status.thinking").takeIf { s.thinking == true })
            ?.let { add(AgentChipPart("effort", it)) }
        if (settings.context) s.contextPercent()?.let { add(AgentChipPart("context", "${t("agent_status.field.context")} ${agentPercentText(it)}%")) }
        if (settings.usage) agentTopLimit(limits)?.let { add(AgentChipPart("usage", "${it.title()} ${agentPercentText(it.used()!!)}%")) }
        if (settings.cache) s.cacheHitRatio()?.let { add(AgentChipPart("cache", tr("agent_status.cache.hit_ratio", agentPercentText(it * 100)))) }
        if (settings.cost) s.cost_usd?.let { add(AgentChipPart("cost", tr("agent_status.cost", agentCostText(it)))) }
    }
}

/** The full chip text, then shorter texts that drop parts in [AGENT_STATUS_CHIP_DROP_ORDER]; ends with "". */
internal fun agentStatusChipVariants(parts: List<AgentChipPart>): List<String> {
    var remaining = parts
    val variants = mutableListOf(remaining.joinToString(" · ") { it.text })
    for (field in AGENT_STATUS_CHIP_DROP_ORDER) {
        if (remaining.none { it.field == field }) continue
        remaining = remaining.filter { it.field != field }
        variants += remaining.joinToString(" · ") { it.text }
    }
    if (variants.last().isNotEmpty()) variants += ""
    return variants.distinct()
}

internal fun agentCostText(cost: Double): String = String.format(java.util.Locale.ROOT, "%.2f", cost)

/** The opt-in collector each CLI can install, as the helper names it in `sources`; null when none exists. */
private fun richSource(agent: String): String? = when (agent) { "Claude" -> "mod"; "Agy" -> "statusline"; else -> null }

/** Why an agent has no status at all: only a CLI with an installable collector reads as "not collected". */
internal fun agentNoStatusKey(agent: String): String =
    if (richSource(agent) != null) "agent_status.not_collected" else "agent_status.unknown"

/**
 * Whether an item has a value and, if not, why. [field] is an [AgentStatusSettings] item name.
 * Mirrors desktop `agent_status_ui::missing`: Codex and Grok have nothing to install, so their
 * missing account and usage are "not provided" and other missing items are unknown.
 */
internal fun agentFieldState(agent: String, field: String, status: AgentStatus?, limits: List<UsageLimit>): AgentFieldState {
    if (status != null) {
        val shown = when (field) {
            "account" -> status.account?.kind != null || !status.account?.plan.isNullOrBlank()
            "usage" -> limits.isNotEmpty()
            "context" -> status.contextPercent() != null || status.context?.used != null
            "breakdown" -> status.context?.breakdown?.isNotEmpty() == true
            "cache" -> status.cache?.let { c -> listOf(c.input, c.read, c.write, c.output).any { it != null } } == true
            "model" -> status.model?.let { !it.name.isNullOrBlank() || !it.id.isNullOrBlank() } == true
            "effort" -> !status.effort.isNullOrBlank() || status.thinking != null
            "cost" -> status.cost_usd != null
            else -> false
        }
        if (shown) return AgentFieldState.Shown
    }
    val supported = when (field) {
        "breakdown" -> agent == "Claude"
        "account", "usage" -> agent == "Claude" || agent == "Agy"
        "cost" -> agent != "Codex"
        else -> true
    }
    if (!supported) return AgentFieldState.NotSupported
    val rich = richSource(agent) ?: return AgentFieldState.Unknown
    if (status == null) return AgentFieldState.NotCollected
    val needsRich = field in setOf("account", "usage", "context", "breakdown", "cache", "cost")
    if (needsRich && rich !in status.sources) return AgentFieldState.NotCollected
    // Mirrors desktop `missing()`: "no limit data" only when the account is known not to be a subscription.
    val kind = status.account?.kind
    return if (field == "usage" && kind != null && kind != "subscription") AgentFieldState.NoLimits else AgentFieldState.Unknown
}

/** Helper `status_collection` state of one CLI in an inspect reply; null when nothing can be installed. */
internal fun agentCollectionKey(supported: Boolean, installed: Boolean, replaced: Boolean): String? = when {
    installed -> "agent_status.collect.installed"
    replaced -> "agent_status.collect.replaced"
    supported -> "agent_status.not_collected"
    else -> null
}

/** Breakdown rows with their share of the context window, clamped so the shares never exceed 1. */
internal fun agentBreakdownShares(context: ContextUsage): List<Pair<ContextPart, Double>> {
    val rows = context.breakdown.filter { (it.tokens ?: 0) > 0 }
    val window = (context.breakdown_window ?: context.breakdown_total ?: rows.sumOf { it.tokens ?: 0 }).takeIf { it > 0 } ?: return emptyList()
    var left = 1.0
    return rows.map { row ->
        val share = ((row.tokens ?: 0).toDouble() / window).coerceIn(0.0, left)
        left -= share
        row to share
    }
}

private fun categoryColor(category: String, dark: Boolean, scheme: ColorScheme): Color = when (category) {
    "system_prompt" -> Color(if (dark) 0xff7aa7ff else 0xff3f73d6)
    "system_tools" -> Color(if (dark) 0xffa98cf0 else 0xff7a56c9)
    "mcp_tools" -> Color(if (dark) 0xff4fc3b3 else 0xff1f8a7c)
    "custom_agents" -> Color(if (dark) 0xffe8994f else 0xffc0661d)
    "memory_files" -> Color(if (dark) 0xffe07dbb else 0xffb4478c)
    "skills" -> Color(if (dark) 0xff9cc04a else 0xff5e8020)
    "messages" -> Color(if (dark) 0xffe8c04a else 0xffb08600)
    "deferred" -> Color(if (dark) 0xff9aa4b2 else 0xff6d7786)
    "autocompact_buffer" -> scheme.outline
    "free" -> scheme.outlineVariant
    else -> Color(if (dark) 0xffc98080 else 0xff9c4f4f)
}

/** A known share only: callers leave an unknown one out instead of drawing 0%. */
@Composable private fun Gauge(fraction: Double, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.primary) {
    Box(modifier.fillMaxWidth().height(6.dp).background(MaterialTheme.colorScheme.surfaceVariant)) {
        val value = fraction.takeIf { it.isFinite() }?.coerceIn(0.0, 1.0)?.toFloat() ?: 0f
        if (value > 0f) Box(Modifier.fillMaxWidth(value).fillMaxHeight().background(color))
    }
}

@Composable private fun gaugeColor(percent: Double): Color =
    if (percent >= 90.0) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary

/** Compact summary of the focused agent; hidden when the connection observes no agent. */
@Composable internal fun AgentStatusChip(client: ClientConnection, settings: AgentStatusSettings, open: () -> Unit, modifier: Modifier = Modifier) {
    val snapshot by client.agents.collectAsState()
    val pane by client.pane.collectAsState()
    val now = System.currentTimeMillis()
    val focus = agentStatusFocus(snapshot, pane) ?: return
    val status = focus.status
    val stale = !snapshot.current(now) || status?.isStale(now) == true
    val parts = agentStatusChipParts(focus, snapshot.limitsFor(focus), settings)
    val variants = if (parts.isEmpty()) listOf(agentName(focus.agent), "") else agentStatusChipVariants(parts)
    val alert = status?.alert?.let(::agentAlertLabel)
    val description = listOfNotNull(t("agent_status.open_details"), agentName(focus.agent), variants.first().takeIf { it.isNotEmpty() },
        alert, if (stale) t("agent_status.stale") else null, if (status == null) t(agentNoStatusKey(focus.agent)) else null).joinToString(" · ")
    val color = if (stale) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface
    val style = MaterialTheme.typography.labelMedium
    val measurer = rememberTextMeasurer()
    BorderedButton(open, modifier.semantics { contentDescription = description }, contentPadding = PaddingValues(horizontal = 8.dp)) {
        Icon(LauncherIcons.agent(focus.agent), null, Modifier.size(16.dp), tint = color)
        if (alert != null) UiGlyph(UiIcons.error, Modifier.padding(start = 4.dp).size(16.dp), tint = MaterialTheme.colorScheme.error)
        BoxWithConstraints(Modifier.weight(1f, fill = false).clearAndSetSemantics {}) {
            // The gap before the text is part of the measured width.
            val gap = with(androidx.compose.ui.platform.LocalDensity.current) { 6.dp.roundToPx() }
            val text = if (!constraints.hasBoundedWidth) variants.first()
                else variants.firstOrNull { it.isEmpty() || measurer.measure(it, style, maxLines = 1).size.width + gap <= constraints.maxWidth } ?: ""
            if (text.isNotEmpty()) Text(text, Modifier.padding(start = 6.dp), color = color, style = style, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** Every observed agent of [client] with all enabled items; the focused pane comes first. */
@Composable internal fun AgentStatusPanel(client: ClientConnection, settings: AgentStatusSettings, modifier: Modifier = Modifier) {
    val snapshot by client.agents.collectAsState()
    val remote by client.remote.collectAsState()
    val pane by client.pane.collectAsState()
    val now = System.currentTimeMillis()
    val observations = agentStatusOrder(snapshot, pane)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (observations.isEmpty()) Text(t("agent_status.none"))
        else if (!snapshot.current(now)) Text(t("agent_status.stale"), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        observations.forEachIndexed { index, o ->
            if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            AgentStatusDetails(o, snapshot.limitsFor(o), settings, now, remote.targetLabel(o.session, o.pane))
        }
    }
}

@Composable private fun StatusItem(title: String, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        content()
    }
}

@Composable private fun FallbackText(state: AgentFieldState, field: String) {
    Text(when (state) {
        AgentFieldState.NotCollected -> t("agent_status.not_collected")
        AgentFieldState.NotSupported -> if (field == "breakdown") t("agent_status.no_breakdown") else t("agent_status.not_supported")
        AgentFieldState.NoLimits -> t("agent_status.limit.none")
        else -> t("agent_status.unknown")
    }, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable private fun AgentStatusDetails(o: AgentObservation, limits: List<UsageLimit>, settings: AgentStatusSettings, now: Long, target: String) {
    val s = o.status
    val small = MaterialTheme.typography.bodySmall
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(LauncherIcons.agent(o.agent), null, Modifier.size(18.dp))
            Text(agentName(o.agent) + if (o.actor != "parent") t("agent.subagent_suffix") else "", style = MaterialTheme.typography.titleSmall)
            Text(target, Modifier.weight(1f), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        s?.alert?.let(::agentAlertLabel)?.let { Text(it, color = MaterialTheme.colorScheme.error, style = small) }
        if (s == null) {
            Text(t(agentNoStatusKey(o.agent)), style = small)
            return@Column
        }
        val ago = s.at?.let { tr("agent_status.observed_ago", formatDurationS((now - it) / 1000)) }
        Text(listOfNotNull(if (s.isStale(now)) t("agent_status.stale") else null, ago).joinToString(" · ").ifEmpty { t("agent_status.unknown") },
            style = MaterialTheme.typography.labelSmall, color = if (s.isStale(now)) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
        fun state(field: String) = agentFieldState(o.agent, field, s, limits)
        if (settings.account) StatusItem(t("agent_status.field.account")) {
            val account = s.account
            if (state("account") == AgentFieldState.Shown && account != null) Text(listOfNotNull(account.kindLabel(), account.plan?.takeIf { it.isNotBlank() }).joinToString(" · "))
            else FallbackText(state("account"), "account")
        }
        if (settings.usage) StatusItem(t("agent_status.field.usage")) {
            if (state("usage") != AgentFieldState.Shown) FallbackText(state("usage"), "usage")
            limits.forEach { limit ->
                val used = limit.used()
                Row(Modifier.fillMaxWidth()) {
                    Text(limit.title(), Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(used?.let { tr("agent_status.limit.used", agentPercentText(it)) } ?: t("agent_status.unknown"), style = small)
                }
                // An unknown share is never drawn as an empty (0%) gauge.
                used?.let { Gauge(it / 100, color = gaugeColor(it)) }
                limit.resetsInS(now)?.let { Text(tr("agent_status.limit.resets_in", formatDurationS(it)), style = MaterialTheme.typography.labelSmall) }
            }
        }
        val context = s.context
        if (settings.context) StatusItem(t("agent_status.field.context")) {
            val percent = s.contextPercent()
            if (state("context") != AgentFieldState.Shown) FallbackText(state("context"), "context")
            else {
                Row(Modifier.fillMaxWidth()) {
                    Text(context?.used?.let { used -> context.size?.let { tr("agent_status.context.tokens", formatTokens(used), formatTokens(it)) } ?: formatTokens(used) }
                        ?: t("agent_status.unknown"), Modifier.weight(1f), style = small)
                    percent?.let { Text("${agentPercentText(it)}%", style = small) }
                }
                percent?.let { Gauge(it / 100, color = gaugeColor(it)) }
                context?.compact_percent?.let { Text(tr("agent_status.context.compact_at", agentPercentText(it)), style = MaterialTheme.typography.labelSmall) }
            }
        }
        if (settings.breakdown) StatusItem(t("agent_status.field.breakdown")) {
            val shares = context?.let(::agentBreakdownShares).orEmpty()
            if (shares.isEmpty()) FallbackText(state("breakdown").let { if (it == AgentFieldState.Shown) AgentFieldState.Unknown else it }, "breakdown")
            else {
                val scheme = MaterialTheme.colorScheme
                val dark = scheme.background.luminance() < 0.5f
                Row(Modifier.fillMaxWidth().height(10.dp).background(scheme.surfaceVariant)) {
                    shares.forEach { (part, share) -> if (share > 0) Box(Modifier.weight(share.toFloat()).fillMaxHeight().background(categoryColor(part.categoryKey, dark, scheme))) }
                    val rest = 1.0 - shares.sumOf { it.second }
                    if (rest > 0.0001) Spacer(Modifier.weight(rest.toFloat()))
                }
                shares.forEach { (part, share) ->
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        Box(Modifier.size(10.dp).background(categoryColor(part.categoryKey, dark, scheme)))
                        Text(part.label(), Modifier.weight(1f), style = small, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text("${formatTokens(part.tokens ?: 0)} · ${agentPercentText(share * 100)}%", style = small)
                    }
                }
            }
        }
        if (settings.cache) StatusItem(t("agent_status.field.cache")) {
            val cache = s.cache
            Text(t("agent_status.cache.server_side"), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (state("cache") != AgentFieldState.Shown || cache == null) FallbackText(state("cache"), "cache")
            else {
                listOf("agent_status.cache.input" to cache.input, "agent_status.cache.read" to cache.read, "agent_status.cache.write" to cache.write,
                    "agent_status.cache.output" to cache.output).forEach { (key, value) ->
                    if (value != null) Row(Modifier.fillMaxWidth()) { Text(t(key), Modifier.weight(1f), style = small); Text(formatTokens(value), style = small) }
                }
                s.cacheHitRatio()?.let { Text(tr("agent_status.cache.hit_ratio", agentPercentText(it * 100)), style = small) }
                cache.last_at?.let { Text(tr("agent_status.cache.last_request", formatDurationS((now - it) / 1000)), style = small) }
                val ttl = cache.ttl?.takeIf { it.isNotBlank() }
                val expiry = cache.estimatedExpiryMs()
                if (ttl != null && expiry != null) Text(if (expiry > now) tr("agent_status.cache.expires_in", ttl, formatDurationS((expiry - now) / 1000))
                    else tr("agent_status.cache.expired", ttl), style = small)
            }
        }
        if (settings.model) StatusItem(t("agent_status.field.model")) {
            val name = s.model?.name?.takeIf { it.isNotBlank() }
            val id = s.model?.id?.takeIf { it.isNotBlank() }
            if (name == null && id == null) FallbackText(state("model"), "model")
            else Text(if (name != null && id != null && name != id) "$name ($id)" else name ?: id!!)
        }
        if (settings.effort) StatusItem(t("agent_status.field.effort")) {
            if (state("effort") != AgentFieldState.Shown) FallbackText(state("effort"), "effort")
            else Text(listOfNotNull(s.effort?.takeIf { it.isNotBlank() }, if (s.thinking == true) t("agent_status.thinking") else null)
                .joinToString(" · ").ifEmpty { t("agent_status.unknown") })
        }
        if (settings.cost) StatusItem(t("agent_status.field.cost")) {
            s.cost_usd?.let { Text(tr("agent_status.cost", agentCostText(it))) } ?: FallbackText(state("cost"), "cost")
        }
    }
}

/** The full panel for wide screens. */
@Composable internal fun AgentStatusDialog(client: ClientConnection, settings: AgentStatusSettings, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(t("agent_status.title")) }, text = {
        AgentStatusPanel(client, settings, Modifier.verticalScroll(rememberScrollState()))
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}

/** The full panel for phones, in the same bottom sheet form as tmux navigation. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun AgentStatusSheet(client: ClientConnection, settings: AgentStatusSettings, dismiss: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = dismiss, sheetState = sheet) {
        // A weighted child needs a finite height before the sheet computes its anchors.
        Column(Modifier.fillMaxWidth().heightIn(max = 600.dp)) {
            Text(t("agent_status.title"), Modifier.padding(horizontal = 16.dp).semantics { heading() }, style = MaterialTheme.typography.titleMedium)
            AgentStatusPanel(client, settings, Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(16.dp))
            BorderedButton(dismiss, modifier = Modifier.fillMaxWidth()) { Text(t("common.close")) }
        }
    }
}

/** Collapsible summary at the bottom of the wide sidebar: one line per agent with mini gauges. */
@Composable internal fun AgentStatusSidebar(client: ClientConnection?, settings: AgentStatusSettings, openDetails: () -> Unit) {
    var expanded by rememberSaveable { mutableStateOf(true) }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(expanded, role = Role.Button) { expanded = it }
            .semantics { stateDescription = t(if (expanded) "common.collapse" else "common.expand") }.padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            UiGlyph(if (expanded) UiIcons.chevronDown else UiIcons.chevronRight)
            Text(t("agent_status.title"), Modifier.weight(1f).semantics { heading() }, style = MaterialTheme.typography.titleSmall)
        }
        if (expanded) Column(Modifier.fillMaxWidth().heightIn(max = 220.dp).verticalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, bottom = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val snapshot = client?.agents?.collectAsState()?.value
            val pane = client?.pane?.collectAsState()?.value
            val now = System.currentTimeMillis()
            val observations = snapshot?.let { agentStatusOrder(it, pane) }.orEmpty()
            if (observations.isEmpty()) Text(t("agent_status.none"), style = MaterialTheme.typography.bodySmall)
            observations.forEach { o -> AgentStatusLine(o, snapshot!!.limitsFor(o), settings, !snapshot.current(now) || o.status?.isStale(now) == true, openDetails) }
            if (observations.isNotEmpty()) TextButton(openDetails) { Text(t("agent_status.open_details")) }
        }
    }
}

@Composable private fun AgentStatusLine(o: AgentObservation, limits: List<UsageLimit>, settings: AgentStatusSettings, stale: Boolean, open: () -> Unit) {
    val s = o.status
    val small = MaterialTheme.typography.labelSmall
    Column(Modifier.fillMaxWidth().clickable(onClick = open).semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Icon(LauncherIcons.agent(o.agent), null, Modifier.size(14.dp))
            val model = if (settings.model) s?.model?.let { it.name ?: it.id } else null
            Text(listOfNotNull(agentName(o.agent) + if (o.actor != "parent") t("agent.subagent_suffix") else "", model).joinToString(" · "),
                Modifier.weight(1f), style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        when {
            s == null -> Text(t(agentNoStatusKey(o.agent)), style = small, color = MaterialTheme.colorScheme.onSurfaceVariant)
            stale -> Text(t("agent_status.stale"), style = small, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        s?.alert?.let(::agentAlertLabel)?.let { Text(it, style = small, color = MaterialTheme.colorScheme.error) }
        val context = if (settings.context) s?.contextPercent() else null
        val top = if (settings.usage) agentTopLimit(limits) else null
        if (context != null || top != null) Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            context?.let { MiniGauge("${t("agent_status.field.context")} ${agentPercentText(it)}%", it, Modifier.weight(1f)) }
            top?.let { limit -> limit.used()?.let { MiniGauge("${limit.title()} ${agentPercentText(it)}%", it, Modifier.weight(1f)) } }
        }
    }
}

@Composable private fun MiniGauge(label: String, percent: Double, modifier: Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(label, style = MaterialTheme.typography.labelSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Gauge(percent / 100, Modifier.height(4.dp), color = gaugeColor(percent))
    }
}

/** Settings card body: where the HUD shows and which items it lists. Device-local. */
@Composable internal fun AgentStatusSettingsUi(model: AppModel) {
    val local by model.local.collectAsState()
    val settings = local.preferences.agentStatus
    fun update(next: AgentStatusSettings) = model.modify { it.copy(preferences = it.preferences.copy(agentStatus = next)) }
    @Composable fun option(label: String, checked: Boolean, change: (Boolean) -> AgentStatusSettings) {
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(checked, role = Role.Checkbox) { update(change(it)) },
            verticalAlignment = Alignment.CenterVertically) {
            AccentCheckbox(checked, null)
            Text(label, Modifier.padding(start = 4.dp))
        }
    }
    option(t("agent_status.show_status_bar"), settings.statusBar) { settings.copy(statusBar = it) }
    option(t("agent_status.show_sidebar"), settings.sidebar) { settings.copy(sidebar = it) }
    Text(t("agent_status.fields"), style = MaterialTheme.typography.titleSmall)
    option(t("agent_status.field.account"), settings.account) { settings.copy(account = it) }
    option(t("agent_status.field.usage"), settings.usage) { settings.copy(usage = it) }
    option(t("agent_status.field.context"), settings.context) { settings.copy(context = it) }
    option(t("agent_status.field.breakdown"), settings.breakdown) { settings.copy(breakdown = it) }
    option(t("agent_status.field.cache"), settings.cache) { settings.copy(cache = it) }
    option(t("agent_status.field.model"), settings.model) { settings.copy(model = it) }
    option(t("agent_status.field.effort"), settings.effort) { settings.copy(effort = it) }
    option(t("agent_status.field.cost"), settings.cost) { settings.copy(cost = it) }
}
