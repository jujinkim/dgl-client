package kim.jujin.dglc.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

private val VolumeAction.label: String get() = when (this) {
    VolumeAction.Default -> t("volume_key.default_action")
    VolumeAction.Shortcut -> t("volume_key.shortcut")
    VolumeAction.Macro -> t("volume_key.macro")
}

@Composable actual fun PlatformInputSettings(preferences: Preferences, update: (Preferences) -> Unit) {
    var editing by remember { mutableStateOf<Boolean?>(null) }
    Row(Modifier.fillMaxWidth().toggleable(preferences.volumeKeysEnabled, role = Role.Switch,
        onValueChange = { update(preferences.copy(volumeKeysEnabled = it)) })
        .semantics { contentDescription = t("volume_key.enable") }, verticalAlignment = Alignment.CenterVertically) {
        Text(t("volume_key.enable"), Modifier.weight(1f))
        AccentSwitch(preferences.volumeKeysEnabled, onCheckedChange = null)
    }
    Text(t("volume_key.foreground_note"), style = MaterialTheme.typography.bodySmall)
    listOf(true to preferences.volumeUp, false to preferences.volumeDown).forEach { (up, binding) ->
        BorderedButton({ editing = up }) {
            val summary = if (binding.action == VolumeAction.Shortcut) (binding.steps.singleOrNull() as? MacroStep.Key)?.key?.label
                else if (binding.action == VolumeAction.Macro) tr("volume_key.step", binding.steps.size) else null
            Text(tr("volume_key.summary", if (up) t("volume_key.up") else t("volume_key.down"), binding.action.label) + (summary?.let { " · $it" } ?: ""))
        }
    }
    editing?.let { up ->
        VolumeBindingEditor(if (up) t("volume_key.volume_up_settings") else t("volume_key.volume_down_settings"), if (up) preferences.volumeUp else preferences.volumeDown,
            save = { binding ->
                update(if (up) preferences.copy(volumeUp = binding) else preferences.copy(volumeDown = binding))
                editing = null
            }, dismiss = { editing = null })
    }
}

@Composable private fun KeyPicker(selected: TerminalKey, change: (TerminalKey) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        OutlinedButton({ expanded = true }) { Text(tr("volume_key.key_value", selected.label)) }
        DropdownMenu(expanded, { expanded = false }, Modifier.heightIn(max = 280.dp)) {
            TerminalKey.entries.forEach { key ->
                DropdownMenuItem(text = { Text(key.label) }, onClick = { change(key); expanded = false })
            }
        }
    }
}

@Composable private fun VolumeBindingEditor(title: String, original: VolumeBinding, save: (VolumeBinding) -> Unit, dismiss: () -> Unit) {
    var action by remember { mutableStateOf(original.action) }
    var shortcut by remember { mutableStateOf((original.steps.singleOrNull() as? MacroStep.Key)?.key ?: TerminalKey.Escape) }
    var steps by remember { mutableStateOf(original.steps.toList()) }
    var error by remember { mutableStateOf<String?>(null) }
    AlertDialog(onDismissRequest = dismiss, title = { Text(title) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                VolumeAction.entries.forEach { choice ->
                    FilterChip(action == choice, { action = choice; error = null }, label = { Text(choice.label) })
                }
            }
            when (action) {
                VolumeAction.Default -> Text(t("volume_key.default_description"))
                VolumeAction.Shortcut -> KeyPicker(shortcut) { shortcut = it; error = null }
                VolumeAction.Macro -> {
                    Text(t("volume_key.macro_note"), style = MaterialTheme.typography.bodySmall)
                    steps.forEachIndexed { index, step ->
                        OutlinedCard {
                            Column(Modifier.padding(8.dp)) {
                                Text(tr("volume_key.step", index + 1))
                                when (step) {
                                    is MacroStep.Text -> OutlinedTextField(step.text, { value ->
                                        steps = steps.toMutableList().also { it[index] = MacroStep.Text(value) }; error = null
                                    }, label = { Text(tr("volume_key.text_value", index + 1)) }, modifier = Modifier.fillMaxWidth(), maxLines = 4)
                                    is MacroStep.Key -> KeyPicker(step.key) { value ->
                                        steps = steps.toMutableList().also { it[index] = MacroStep.Key(value) }; error = null
                                    }
                                }
                                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    BorderedButton({ steps = steps.toMutableList().also { it.add(index - 1, it.removeAt(index)) } }, enabled = index > 0,
                                        modifier = Modifier.semantics { contentDescription = tr("volume_key.move_step_up", index + 1) }) { Text(t("volume_key.move_up")) }
                                    BorderedButton({ steps = steps.toMutableList().also { it.add(index + 1, it.removeAt(index)) } }, enabled = index < steps.lastIndex,
                                        modifier = Modifier.semantics { contentDescription = tr("volume_key.move_step_down", index + 1) }) { Text(t("volume_key.move_down")) }
                                    BorderedButton({ steps = steps.filterIndexed { i, _ -> i != index }; error = null },
                                        modifier = Modifier.semantics { contentDescription = tr("volume_key.delete_step", index + 1) }) { Text(t("common.delete")) }
                                }
                            }
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        BorderedButton({ steps = steps + MacroStep.Text(""); error = null }) { Text(t("volume_key.add_text")) }
                        BorderedButton({ steps = steps + MacroStep.Key(TerminalKey.Enter); error = null }) { Text(t("volume_key.add_key")) }
                    }
                }
            }
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }, confirmButton = {
        BorderedButton({
            val binding = VolumeBinding(action, when (action) {
                VolumeAction.Default -> emptyList()
                VolumeAction.Shortcut -> listOf(MacroStep.Key(shortcut))
                VolumeAction.Macro -> steps.filterNot { it is MacroStep.Text && it.text.isEmpty() }
            })
            try {
                binding.bytes() // Validate the exact UTF-8 payload before committing any preference.
                save(binding)
            } catch (e: IllegalArgumentException) { error = e.message }
            catch (_: Exception) { error = t("volume_key.save_failed") }
        }) { Text(t("common.save")) }
    }, dismissButton = { BorderedButton(dismiss) { Text(t("common.cancel")) } })
}
