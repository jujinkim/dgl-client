package kim.jujin.dglc.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*
import kotlinx.serialization.json.*

@Composable internal fun AgentBadge(state: PanelAgentState, onDetails: (() -> Unit)? = null) {
    val modifier = if (onDetails == null) Modifier else Modifier.widthIn(min = 48.dp).height(48.dp)
        .clickable(role = Role.Button, onClick = onDetails)
        .semantics { contentDescription = tr("agent.window_status", state.label) }.padding(horizontal = 4.dp)
    Row(modifier, verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp, androidx.compose.ui.Alignment.CenterHorizontally)) {
        if (state.agent != null) Icon(LauncherIcons.agent(state.agent), null, Modifier.size(16.dp))
        if (state.kind != null || state.agent == null) UiGlyph(agentIcon(state.kind))
        if (onDetails == null) Text(state.label, style = MaterialTheme.typography.labelSmall, maxLines = 2,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
        else if (state.unread.isNotEmpty()) Text(state.unread.size.toString(), Modifier.clearAndSetSemantics {},
            style = MaterialTheme.typography.labelSmall, maxLines = 1)
    }
}

@Composable internal fun AgentDetails(client: ClientConnection, model: AppModel, pane: String? = null, dismiss: () -> Unit) {
    var showHooks by remember(client, pane) { mutableStateOf(false) }
    if (showHooks) {
        AgentHookSettings(client, dismiss)
        return
    }
    val snapshot by client.agents.collectAsState()
    val notices by model.agentNotifications.collectAsState()
    AlertDialog(onDismissRequest = dismiss, title = { Text(tr("agent.integration_named", client.host.alias)) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(t("agent.last_observed_note"))
            val now = System.currentTimeMillis()
            val panel = snapshot.panelState(pane, client.agentEndpoint, notices, now)
            AgentBadge(panel)
            val remote by client.remote.collectAsState()
            panel.observations.forEach { o ->
                Text("${agentName(o.agent)} · ${agentPhaseLabel(snapshot.phaseOf(o, now))} · ${remote.targetLabel(o.session, o.pane)}" + if (o.actor != "parent") t("agent.subagent_suffix") else "")
                Row { UiGlyph(agentIcon(o.last?.kind)); Text(o.last?.let { "${agentMessage(it.kind)} · ${it.event}\n${java.util.Date(it.at)}" } ?: t("agent.not_linked")) }
                Text(tr("agent.diagnostics_ids", o.session, o.window, o.pane), style = MaterialTheme.typography.labelSmall)
                Text(tr("agent.observed_event_types", o.supported.joinToString { agentMessage(it) }.ifEmpty { t("agent.unknown") }), style = MaterialTheme.typography.labelSmall)
                BorderedButton({
                    model.openAgentObservation(client, o)
                    dismiss()
                }) { Text(t("agent.go_pane")) }
            }
            if (pane == null) {
                Text(t("agent.basic_observation"))
                BorderedButton({
                    client.configureAgents("inspect")
                    showHooks = true
                }) { Text(t("agent.advanced_hooks")) }
            }
        }
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}

@Composable private fun AgentHookSettings(client: ClientConnection, dismiss: () -> Unit) {
    val setup by client.agentSetup.collectAsState()
    val preview = setup?.let { runCatching { Json.parseToJsonElement(it).jsonObject }.getOrNull() }
    val token = preview?.get("token")?.jsonPrimitive?.content
    val collections = preview?.get("configs")?.jsonArray.orEmpty().mapNotNull { runCatching { it.jsonObject["status_collection"]?.jsonObject }.getOrNull() }
    fun flag(o: JsonObject, name: String) = o[name]?.jsonPrimitive?.booleanOrNull == true
    val collectable = collections.any { flag(it, "supported") } || preview?.get("configs")?.jsonArray.orEmpty().any { flag(it.jsonObject, "supported") }
    // Install without the flag removes detailed collection, so the box starts as it is installed now.
    var collect by remember(token) { mutableStateOf(preview?.let { flag(it, "account_limits") } == true || collections.any { flag(it, "installed") }) }
    AlertDialog(onDismissRequest = dismiss, title = { Text(tr("agent.hooks_named", client.host.alias)) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(t("agent.optional_setup_existing_kept"))
            BorderedButton({ client.configureAgents("inspect") }) { Text(t("agent.inspect_before_install")) }
            if (preview == null) Text(t("agent.inspecting"))
            preview?.get("error")?.jsonPrimitive?.content?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            if (token != null) {
                Text("Python ${preview["python"]?.jsonPrimitive?.content} · ${preview["tmux"]?.jsonPrimitive?.content}")
                Text(tr("agent.helper_observation_storage", preview["helper"]?.jsonPrimitive?.content, preview["database"]?.jsonPrimitive?.content))
                preview["configs"]?.jsonArray?.forEach { item ->
                    val c = item.jsonObject
                    Text("${c["agent"]?.jsonPrimitive?.content}: ${c["version"]?.jsonPrimitive?.content}\n${c["path"]?.jsonPrimitive?.content}")
                    Text(if (c["installed"]?.jsonPrimitive?.boolean == true) t("agent.installed_check_activation_detail") else t("agent.not_installed"))
                    Text(if (c["supported"]?.jsonPrimitive?.boolean == true) tr("agent.added_hooks", c["events"]?.jsonArray?.joinToString { it.jsonPrimitive.content }) else t("agent.unverified_cli_version"))
                    if (c["experimental"]?.jsonPrimitive?.boolean == true) Text(t("agent.experimental_unverified"))
                    c["error"]?.jsonPrimitive?.content?.takeIf { it.isNotEmpty() }?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    Text(tr("agent.available_event_types", c["capabilities"]?.jsonArray?.joinToString { it.jsonPrimitive.content }))
                    c["limitations"]?.jsonPrimitive?.content?.takeIf { it.isNotEmpty() }?.let { Text(it) }
                    c["status_collection"]?.let { runCatching { it.jsonObject }.getOrNull() }?.let { sc ->
                        agentCollectionKey(flag(sc, "supported"), flag(sc, "installed"), flag(sc, "replaced"))?.let {
                            Text(t(it), color = if (flag(sc, "replaced")) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                        }
                    }
                }
                Text(t("agent.codex_trust_hooks"))
                if (collectable) {
                    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(collect, role = Role.Checkbox) { collect = it },
                        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                        AccentCheckbox(collect, null)
                        Text(t("agent_status.collect"), Modifier.padding(start = 4.dp))
                    }
                    Text(t("agent_status.collect_description"), style = MaterialTheme.typography.bodySmall)
                    Text(t("agent_status.collect.restart"), style = MaterialTheme.typography.bodySmall)
                }
                BorderedButton({ client.configureAgents("install", token, collectable && collect) }) { Text(t("agent.install_integration")) }
                BorderedButton({ client.configureAgents("remove", token) }) { Text(t("agent.remove_dagali_hooks_only")) }
            }
        }
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}

@Composable internal fun AgentNotifications(model: AppModel, dismiss: () -> Unit) {
    val notices by model.agentNotifications.collectAsState()
    AlertDialog(onDismissRequest = dismiss, title = { Text(tr("agent.recent_notifications_unread", notices.count { !it.read })) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(t("agent.recent_limit_note"))
            if (notices.isEmpty()) Text(t("agent.no_notifications_received"))
            notices.forEach { n ->
                Row {
                    if (!n.read) Icon(UiIcons.unread, t("common.unread"), Modifier.size(20.dp))
                    Text(tr("agent.notice_with_diagnostics", agentName(n.event.agent), n.endpoint, n.event.session, n.event.window, n.event.pane) + if (n.event.actor != "parent") t("agent.subagent_suffix") else "")
                }
                Row { UiGlyph(agentIcon(n.event.kind)); Text(agentMessage(n.event.kind)) }
                Text("${n.event.event} · ${java.util.Date(n.event.at)}", style = MaterialTheme.typography.labelSmall)
                BorderedButton({ model.openAgentNotice(n.id); dismiss() }) { Text(t("agent.go_pane_mark_read")) }
            }
        }
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}

@Composable internal fun AgentNotificationSettingsUi(model: AppModel) {
    val local by model.local.collectAsState()
    val settings = local.preferences.agentNotifications
    Text(t("agent.notifications"), style = MaterialTheme.typography.titleSmall)
    listOf(t("agent.questions_approval_requests") to settings.questions, t("agent.response_end_signals") to settings.responses, t("agent.explicit_errors") to settings.errors, t("agent.os_notifications") to settings.os).forEachIndexed { i, (label, enabled) ->
        Row { AccentCheckbox(enabled, { value ->
            val next = when (i) { 0 -> settings.copy(questions = value); 1 -> settings.copy(responses = value); 2 -> settings.copy(errors = value); else -> settings.copy(os = value) }
            model.modify { it.copy(preferences = it.preferences.copy(agentNotifications = next)) }
        }); Text(label, Modifier.padding(top = 12.dp)) }
    }
    Text(t("agent.in_app_notifications_note_running"), style = MaterialTheme.typography.labelSmall)
}
