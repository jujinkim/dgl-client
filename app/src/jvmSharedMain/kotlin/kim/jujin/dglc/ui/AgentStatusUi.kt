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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*
import kotlin.math.roundToLong

// Agent status HUD: the status chip, the full panel (dialog on wide screens, bottom sheet on
// phones), the sidebar summary and the settings card body.

internal fun agentPercentText(value: Double): String = value.roundToLong().toString()

/** The limit closest to exhaustion; limits with an unknown figure are skipped. */
internal fun agentTopLimit(limits: List<UsageLimit>): UsageLimit? = limits.filter { it.used() != null }.maxByOrNull { it.used()!! }

/** The observer polls the whole tmux server. The status chip belongs to one tab's session. */
internal fun AgentSnapshot.inSession(session: String?) = observations.filter { it.session == session }

internal fun agentStatusFocus(snapshot: AgentSnapshot, session: String?, pane: String?, now: Long): AgentObservation? =
    snapshot.inSession(session).filter { it.pane == pane && snapshot.hasDisplayStatus(it, now) }
        .minWithOrNull(compareBy<AgentObservation> { it.actor != "parent" }
        .thenBy { it.status == null })

/** Panel order: the focused pane's agents first, then the rest in snapshot order. */
internal fun agentStatusOrder(observations: List<AgentObservation>, pane: String?): List<AgentObservation> =
    observations.sortedByDescending { it.pane == pane }

/** Known, enabled status values for the status button's accessibility description. */
internal fun agentStatusSummary(status: AgentStatus?, limits: List<UsageLimit>, settings: AgentStatusSettings): String {
    val s = status ?: return ""
    return buildList {
        if (settings.model) (s.model?.name ?: s.model?.id)?.takeIf { it.isNotBlank() }?.let { add(it) }
        if (settings.effort) (s.effort?.takeIf { it.isNotBlank() } ?: t("agent_status.thinking").takeIf { s.thinking == true })
            ?.let { add(it) }
        if (settings.context) s.contextPercent()?.let { add("${t("agent_status.field.context")} ${agentPercentText(it)}%") }
        if (settings.usage) agentTopLimit(limits)?.let { add("${it.title()} ${agentPercentText(it.used()!!)}%") }
        if (settings.cache) s.cacheHitRatio()?.let { add(tr("agent_status.cache.hit_ratio", agentPercentText(it * 100))) }
        if (settings.cost) s.cost_usd?.let { add(tr("agent_status.cost", agentCostText(it))) }
    }.joinToString(" · ")
}

internal fun agentCostText(cost: Double): String = String.format(java.util.Locale.ROOT, "%.2f", cost)

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

/** Compact summary of the selected tab's agent. */
@Composable internal fun AgentStatusChip(client: ClientConnection, settings: AgentStatusSettings, wide: Boolean = false, open: () -> Unit) {
    val snapshot by client.agents.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val now = System.currentTimeMillis()
    val focus = agentStatusFocus(snapshot, session, pane, now) ?: return
    val phase = agentPhaseLabel(snapshot.phaseOf(focus, now))
    val status = focus.status
    val stale = status?.isStale(now) == true
    val summary = agentStatusSummary(status, snapshot.limitsFor(focus), settings)
    val alert = status?.alert?.let(::agentAlertLabel)
    val description = listOfNotNull(t("agent_status.open_details"), agentName(focus.agent), phase, summary.takeIf { it.isNotEmpty() },
        alert, if (stale) t("agent_status.stale") else null).joinToString(" · ")
    val color = when {
        alert != null -> MaterialTheme.colorScheme.error
        stale -> MaterialTheme.colorScheme.onSurfaceVariant
        else -> MaterialTheme.colorScheme.onSurface
    }
    val parts = buildList {
        phase?.let { add(it) }
        if (wide && snapshot.current(now)) {
            val contextAt = status?.context_at ?: status?.at
            if (settings.context && contextAt != null && now - contextAt < AGENT_STATUS_STALE_MS)
                status?.contextPercent()?.let { add("ctx " + agentPercentText(it) + "%") }
            val sample = snapshot.limitSampleFor(focus)
            if (settings.usage && sample != null && now - sample.limitsSampledAt() < AGENT_STATUS_STALE_MS)
                agentTopLimit(sample.limits)?.used()?.let { add(tr("agent_status.remaining", agentPercentText((100 - it).coerceAtLeast(0.0)))) }
        }
    }
    val descriptionModifier = Modifier.semantics { contentDescription = description }
    if (parts.isEmpty()) {
        IconButton(open, Modifier.size(48.dp).then(descriptionModifier)) {
            Icon(LauncherIcons.agent(focus.agent), null, Modifier.size(16.dp), tint = color)
        }
    } else {
        TextButton(open, descriptionModifier, contentPadding = PaddingValues(horizontal = 4.dp)) {
            Icon(LauncherIcons.agent(focus.agent), null, Modifier.size(16.dp), tint = color)
            Text(parts.joinToString(" · "), Modifier.padding(start = 4.dp), maxLines = 1, style = MaterialTheme.typography.labelSmall)
        }
    }
}

@Composable private fun AgentStatusPanel(client: ClientConnection, settings: AgentStatusSettings, fromSidebar: Boolean, modifier: Modifier) {
    val snapshot by client.agents.collectAsState()
    val remote by client.remote.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val now = System.currentTimeMillis()
    val observations = agentStatusOrder(
        (if (fromSidebar) snapshot.observations else snapshot.inSession(session)).filter { snapshot.hasDisplayStatus(it, now) }, pane)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        observations.forEachIndexed { index, o ->
            if (index > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            AgentStatusDetails(o, agentPhaseLabel(snapshot.phaseOf(o, now)), snapshot.limitsFor(o), settings, now, remote.targetLabel(o.session, o.pane))
        }
    }
}

@Composable private fun StatusItem(title: String, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        content()
    }
}

@Composable private fun AgentStatusDetails(o: AgentObservation, phase: String?, limits: List<UsageLimit>, settings: AgentStatusSettings, now: Long, target: String) {
    val s = o.status
    val small = MaterialTheme.typography.bodySmall
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Icon(LauncherIcons.agent(o.agent), null, Modifier.size(18.dp))
            Text(agentName(o.agent) + if (o.actor != "parent") t("agent.subagent_suffix") else "", style = MaterialTheme.typography.titleSmall)
            Text(target, Modifier.weight(1f), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        phase?.let { Text(it, style = small) }
        s?.alert?.let(::agentAlertLabel)?.let { Text(it, color = MaterialTheme.colorScheme.error, style = small) }
        if (s == null) return@Column
        val ago = s.at?.let { tr("agent_status.observed_ago", formatDurationS((now - it) / 1000)) }
        val stale = s.isStale(now)
        if (ago != null || stale) {
            Text(listOfNotNull(t("agent_status.stale").takeIf { stale }, ago).joinToString(" · "),
                style = MaterialTheme.typography.labelSmall, color = if (stale) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
        }
        val account = s.account?.let { listOfNotNull(it.kindLabel(), it.plan?.takeIf(String::isNotBlank)).joinToString(" · ") }
        if (settings.account && !account.isNullOrEmpty()) StatusItem(t("agent_status.field.account")) { Text(account) }
        val shownLimits = limits.filter { it.used() != null || it.resets_at != null }
        if (settings.usage && shownLimits.isNotEmpty()) StatusItem(t("agent_status.field.usage")) {
            shownLimits.forEach { limit ->
                val used = limit.used()
                Row(Modifier.fillMaxWidth()) {
                    Text(limit.title(), Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    used?.let { Text(tr("agent_status.limit.used", agentPercentText(it)), style = small) }
                }
                // An unknown share is never drawn as an empty (0%) gauge.
                used?.let { Gauge(it / 100, color = gaugeColor(it)) }
                limit.resetsInS(now)?.let { Text(tr("agent_status.limit.resets_in", formatDurationS(it)), style = MaterialTheme.typography.labelSmall) }
            }
        }
        val context = s.context
        val percent = s.contextPercent()
        val tokens = context?.used?.let { used -> context.size?.let { tr("agent_status.context.tokens", formatTokens(used), formatTokens(it)) } ?: formatTokens(used) }
        val compact = context?.compact_percent
        if (settings.context && (percent != null || tokens != null || compact != null)) StatusItem(t("agent_status.field.context")) {
            if (percent != null || tokens != null) {
                Row(Modifier.fillMaxWidth()) {
                    tokens?.let { Text(it, Modifier.weight(1f), style = small) }
                    percent?.let { Text("${agentPercentText(it)}%", style = small) }
                }
                percent?.let { Gauge(it / 100, color = gaugeColor(it)) }
            }
            compact?.let { Text(tr("agent_status.context.compact_at", agentPercentText(it)), style = MaterialTheme.typography.labelSmall) }
        }
        val shares = if (settings.breakdown) context?.let(::agentBreakdownShares).orEmpty() else emptyList()
        if (shares.isNotEmpty()) StatusItem(t("agent_status.field.breakdown")) {
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
        val cache = s.cache
        if (settings.cache && cache != null) {
            val counts = listOf("agent_status.cache.input" to cache.input, "agent_status.cache.read" to cache.read,
                "agent_status.cache.write" to cache.write, "agent_status.cache.output" to cache.output)
                .mapNotNull { (key, value) -> value?.let { key to it } }
            val ratio = s.cacheHitRatio()
            if (counts.isNotEmpty() || ratio != null || cache.last_at != null) StatusItem(t("agent_status.field.cache")) {
                Text(t("agent_status.cache.server_side"), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                counts.forEach { (key, value) ->
                    Row(Modifier.fillMaxWidth()) { Text(t(key), Modifier.weight(1f), style = small); Text(formatTokens(value), style = small) }
                }
                ratio?.let { Text(tr("agent_status.cache.hit_ratio", agentPercentText(it * 100)), style = small) }
                cache.last_at?.let { Text(tr("agent_status.cache.last_request", formatDurationS((now - it) / 1000)), style = small) }
                val ttl = cache.ttl?.takeIf { it.isNotBlank() }
                val expiry = cache.estimatedExpiryMs()
                if (ttl != null && expiry != null) Text(if (expiry > now) tr("agent_status.cache.expires_in", ttl, formatDurationS((expiry - now) / 1000))
                    else tr("agent_status.cache.expired", ttl), style = small)
            }
        }
        val name = s.model?.name?.takeIf(String::isNotBlank)
        val id = s.model?.id?.takeIf(String::isNotBlank)
        val model = if (name != null && id != null && name != id) "$name ($id)" else name ?: id
        if (settings.model && model != null) StatusItem(t("agent_status.field.model")) {
            Text(model)
        }
        val effort = listOfNotNull(s.effort?.takeIf(String::isNotBlank), t("agent_status.thinking").takeIf { s.thinking == true })
        if (settings.effort && effort.isNotEmpty()) StatusItem(t("agent_status.field.effort")) {
            Text(effort.joinToString(" · "))
        }
        val cost = s.cost_usd
        if (settings.cost && cost != null) StatusItem(t("agent_status.field.cost")) {
            Text(tr("agent_status.cost", agentCostText(cost)))
        }
    }
}

/** The full panel for wide screens. */
@Composable internal fun AgentStatusDialog(client: ClientConnection, settings: AgentStatusSettings, fromSidebar: Boolean, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(t("agent_status.title")) }, text = {
        AgentStatusPanel(client, settings, fromSidebar, Modifier.verticalScroll(rememberScrollState()))
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}

/** The full panel for phones, in the same bottom sheet form as tmux navigation. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun AgentStatusSheet(client: ClientConnection, settings: AgentStatusSettings, fromSidebar: Boolean, dismiss: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = dismiss, sheetState = sheet) {
        // A weighted child needs a finite height before the sheet computes its anchors.
        Column(Modifier.fillMaxWidth().heightIn(max = 600.dp)) {
            Text(t("agent_status.title"), Modifier.padding(horizontal = 16.dp).semantics { heading() }, style = MaterialTheme.typography.titleMedium)
            AgentStatusPanel(client, settings, fromSidebar, Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(16.dp))
            BorderedButton(dismiss, modifier = Modifier.fillMaxWidth()) { Text(t("common.close")) }
        }
    }
}

/** Collapsible summary at the bottom of the wide sidebar: one line per agent with mini gauges. */
@Composable internal fun AgentStatusSidebar(client: ClientConnection?, settings: AgentStatusSettings, openDetails: () -> Unit) {
    var expanded by rememberSaveable { mutableStateOf(true) }
    if (client == null) return
    val snapshot by client.agents.collectAsState()
    val pane by client.pane.collectAsState()
    val now = System.currentTimeMillis()
    val observations = agentStatusOrder(snapshot.observations.filter { snapshot.hasDisplayStatus(it, now) }, pane)
    if (observations.isEmpty()) return
    Column(Modifier.fillMaxWidth()) {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(expanded, role = Role.Button) { expanded = it }
            .semantics { stateDescription = t(if (expanded) "common.collapse" else "common.expand") }.padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            UiGlyph(if (expanded) UiIcons.chevronDown else UiIcons.chevronRight)
            Text(t("agent_status.title"), Modifier.weight(1f).semantics { heading() }, style = MaterialTheme.typography.titleSmall)
        }
        if (expanded) Column(Modifier.fillMaxWidth().heightIn(max = 220.dp).verticalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, bottom = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            observations.forEach { o -> AgentStatusLine(o, agentPhaseLabel(snapshot.phaseOf(o, now)), snapshot.limitsFor(o), settings, o.status?.isStale(now) == true, openDetails) }
            TextButton(openDetails) { Text(t("agent_status.open_details")) }
        }
    }
}

@Composable private fun AgentStatusLine(o: AgentObservation, phase: String?, limits: List<UsageLimit>, settings: AgentStatusSettings, stale: Boolean, open: () -> Unit) {
    val s = o.status
    val small = MaterialTheme.typography.labelSmall
    Column(Modifier.fillMaxWidth().clickable(onClick = open).semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Icon(LauncherIcons.agent(o.agent), null, Modifier.size(14.dp))
            val model = if (settings.model) s?.model?.let { it.name ?: it.id } else null
            Text(listOfNotNull(agentName(o.agent) + if (o.actor != "parent") t("agent.subagent_suffix") else "", phase, model).joinToString(" · "),
                Modifier.weight(1f), style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (stale) Text(t("agent_status.stale"), style = small, color = MaterialTheme.colorScheme.onSurfaceVariant)
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
