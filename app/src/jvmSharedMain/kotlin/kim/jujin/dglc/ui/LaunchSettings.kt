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
    Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        for (agent in LaunchAgent.entries) {
            val config = preferences.launchFor(agent)
            Text(agent.label, style = MaterialTheme.typography.titleMedium)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(t("launcher.bypass_arguments"), style = MaterialTheme.typography.labelMedium)
                    Text(config.bypass)
                }
                BorderedButton({ editor = LaunchEditor.Bypass(agent, config.bypass) }) { Text(t("common.edit")) }
            }
            config.options.forEachIndexed { index, option ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(option.name, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    BorderedButton({ editor = LaunchEditor.Option(agent, index, option) }) { Text(t("common.edit")) }
                    BorderedButton({ update(agent) { it.copy(options = it.options.filterIndexed { i, _ -> i != index }) } }) {
                        Text(t("common.delete"))
                    }
                }
            }
            BorderedButton({ editor = LaunchEditor.Option(agent, null, LaunchOption()) }) { Text(t("launcher.add_option")) }
            HorizontalDivider()
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
                    for (kind in OptionKind.entries) {
                        FilterChip(selected = value.kind == kind, onClick = { draft = option.copy(value = value.copy(kind = kind)) },
                            label = { Text(t(if (kind == OptionKind.Environment) "launcher.environment" else "launcher.argument")) })
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
