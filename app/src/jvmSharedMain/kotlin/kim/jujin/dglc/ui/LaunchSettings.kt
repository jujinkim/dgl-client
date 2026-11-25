package kim.jujin.dglc.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

@Composable internal fun LaunchSettings(preferences: Preferences, update: (LaunchAgent, (AgentLaunch) -> AgentLaunch) -> Unit) {
    var editor by remember { mutableStateOf<LaunchEditor?>(null) }
    // One group per agent. The settings detail column scrolls, so this adds no scroll of its own.
    for (agent in LaunchAgent.entries) {
        val config = preferences.launchFor(agent)
        SettingsGroup(agent.label) {
            SettingsRow(t("launcher.bypass_arguments"), config.bypass.takeIf { it.isNotEmpty() }) {
                SettingsActionButton(t("common.edit"), { editor = LaunchEditor.Bypass(agent, config.bypass) })
            }
            config.options.forEachIndexed { index, option ->
                SettingsDivider()
                Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight).padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
                    Text(option.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    SettingsActionButton(t("common.edit"), { editor = LaunchEditor.Option(agent, index, option) })
                    SettingsActionButton(t("common.delete"), { update(agent) { it.copy(options = it.options.filterIndexed { i, _ -> i != index }) } },
                        style = SettingsActionStyle.Danger)
                }
            }
            SettingsDivider()
            SettingsActions {
                SettingsActionButton(t("launcher.add_option"), { editor = LaunchEditor.Option(agent, null, LaunchOption()) })
            }
        }
    }
    editor?.let { initial ->
        var draft by remember(initial) { mutableStateOf(initial) }
        val option = draft as? LaunchEditor.Option
        AlertDialog(onDismissRequest = { editor = null }, title = { Text(initial.agent.label) }, text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (option == null) {
                    val bypass = draft as LaunchEditor.Bypass
                    LaunchField(t("launcher.bypass_arguments"), bypass.value) { draft = bypass.copy(value = it) }
                } else {
                    val value = option.value
                    LaunchField(t("common.name"), value.name) { draft = option.copy(value = value.copy(name = it)) }
                    LaunchField(t("launcher.description"), value.description) { draft = option.copy(value = value.copy(description = it)) }
                    SettingsChips {
                        for (kind in OptionKind.entries) {
                            SettingsChip(t(if (kind == OptionKind.Environment) "launcher.environment" else "launcher.argument"), value.kind == kind) {
                                draft = option.copy(value = value.copy(kind = kind))
                            }
                        }
                    }
                    LaunchField(t(if (value.kind == OptionKind.Environment) "launcher.env_value" else "launcher.argument_value"), value.value) {
                        draft = option.copy(value = value.copy(value = it))
                    }
                }
            }
        }, confirmButton = {
            BorderedButton(enabled = option?.value?.valid != false, onClick = {
                when (val edit = draft) {
                    is LaunchEditor.Bypass -> update(edit.agent) { it.copy(bypass = edit.value) }
                    is LaunchEditor.Option -> update(edit.agent) { config ->
                        config.copy(options = if (edit.index == null) config.options + edit.value else
                            config.options.mapIndexed { i, old -> if (i == edit.index) edit.value.copy(checked = old.checked) else old })
                    }
                }
                editor = null
            }) { Text(t("common.save")) }
        }, dismissButton = { BorderedButton({ editor = null }) { Text(t("common.cancel")) } })
    }
}

private sealed class LaunchEditor(val agent: LaunchAgent) {
    data class Bypass(val owner: LaunchAgent, val value: String) : LaunchEditor(owner)
    data class Option(val owner: LaunchAgent, val index: Int?, val value: LaunchOption) : LaunchEditor(owner)
}

@Composable private fun LaunchField(label: String, value: String, change: (String) -> Unit) {
    OutlinedTextField(value, { if (launchSingleLine(it)) change(it) }, label = { Text(label) },
        modifier = Modifier.fillMaxWidth(), singleLine = true)
}
