package kim.jujin.rovessh.ui

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
import kim.jujin.rovessh.core.*

private val VolumeAction.label: String get() = when (this) {
    VolumeAction.Default -> "기본 음량 조절"
    VolumeAction.Shortcut -> "단축키"
    VolumeAction.Macro -> "매크로"
}

@Composable actual fun PlatformInputSettings(preferences: Preferences, update: (Preferences) -> Unit) {
    var editing by remember { mutableStateOf<Boolean?>(null) }
    Row(Modifier.fillMaxWidth().toggleable(preferences.volumeKeysEnabled, role = Role.Switch,
        onValueChange = { update(preferences.copy(volumeKeysEnabled = it)) })
        .semantics { contentDescription = "볼륨 키로 단축키 실행" }, verticalAlignment = Alignment.CenterVertically) {
        Text("볼륨 키로 단축키 실행", Modifier.weight(1f))
        Switch(preferences.volumeKeysEnabled, onCheckedChange = null)
    }
    Text("앱 전면의 입력 가능한 터미널에서만 실행합니다.", style = MaterialTheme.typography.bodySmall)
    listOf(true to preferences.volumeUp, false to preferences.volumeDown).forEach { (up, binding) ->
        TextButton({ editing = up }) {
            val summary = if (binding.action == VolumeAction.Shortcut) (binding.steps.singleOrNull() as? MacroStep.Key)?.key?.label
                else if (binding.action == VolumeAction.Macro) "${binding.steps.size}단계" else null
            Text("볼륨 ${if (up) "위" else "아래"}: ${binding.action.label}" + (summary?.let { " · $it" } ?: ""))
        }
    }
    editing?.let { up ->
        VolumeBindingEditor(if (up) "볼륨 위 설정" else "볼륨 아래 설정", if (up) preferences.volumeUp else preferences.volumeDown,
            save = { binding ->
                update(if (up) preferences.copy(volumeUp = binding) else preferences.copy(volumeDown = binding))
                editing = null
            }, dismiss = { editing = null })
    }
}

@Composable private fun KeyPicker(selected: TerminalKey, change: (TerminalKey) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        OutlinedButton({ expanded = true }) { Text("키: ${selected.label}") }
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
                VolumeAction.Default -> Text("이 키는 원래 음량 조절 기능을 사용합니다.")
                VolumeAction.Shortcut -> KeyPicker(shortcut) { shortcut = it; error = null }
                VolumeAction.Macro -> {
                    Text("위에서 아래 순서로 한 번 전송합니다. 텍스트 끝에 Enter를 자동으로 붙이지 않습니다. 실제 줄바꿈은 Enter로, \\n처럼 입력한 문자는 그대로 전송합니다.", style = MaterialTheme.typography.bodySmall)
                    steps.forEachIndexed { index, step ->
                        OutlinedCard {
                            Column(Modifier.padding(8.dp)) {
                                Text("${index + 1}단계")
                                when (step) {
                                    is MacroStep.Text -> OutlinedTextField(step.text, { value ->
                                        steps = steps.toMutableList().also { it[index] = MacroStep.Text(value) }; error = null
                                    }, label = { Text("텍스트 ${index + 1}") }, modifier = Modifier.fillMaxWidth(), maxLines = 4)
                                    is MacroStep.Key -> KeyPicker(step.key) { value ->
                                        steps = steps.toMutableList().also { it[index] = MacroStep.Key(value) }; error = null
                                    }
                                }
                                Row(Modifier.horizontalScroll(rememberScrollState())) {
                                    TextButton({ steps = steps.toMutableList().also { it.add(index - 1, it.removeAt(index)) } }, enabled = index > 0,
                                        modifier = Modifier.semantics { contentDescription = "${index + 1}단계 위로" }) { Text("위로") }
                                    TextButton({ steps = steps.toMutableList().also { it.add(index + 1, it.removeAt(index)) } }, enabled = index < steps.lastIndex,
                                        modifier = Modifier.semantics { contentDescription = "${index + 1}단계 아래로" }) { Text("아래로") }
                                    TextButton({ steps = steps.filterIndexed { i, _ -> i != index }; error = null },
                                        modifier = Modifier.semantics { contentDescription = "${index + 1}단계 삭제" }) { Text("삭제") }
                                }
                            }
                        }
                    }
                    Row {
                        TextButton({ steps = steps + MacroStep.Text(""); error = null }) { Text("+ 텍스트") }
                        TextButton({ steps = steps + MacroStep.Key(TerminalKey.Enter); error = null }) { Text("+ 키") }
                    }
                }
            }
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }, confirmButton = {
        TextButton({
            val binding = VolumeBinding(action, when (action) {
                VolumeAction.Default -> emptyList()
                VolumeAction.Shortcut -> listOf(MacroStep.Key(shortcut))
                VolumeAction.Macro -> steps.filterNot { it is MacroStep.Text && it.text.isEmpty() }
            })
            try {
                binding.bytes() // Validate the exact UTF-8 payload before committing any preference.
                save(binding)
            } catch (e: IllegalArgumentException) { error = e.message }
            catch (_: Exception) { error = "설정을 저장하지 못했습니다. 다시 시도하세요." }
        }) { Text("저장") }
    }, dismissButton = { TextButton(dismiss) { Text("취소") } })
}
