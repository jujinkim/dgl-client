package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.*
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

@Composable expect fun PlatformAccentColor(dark: Boolean): Int?

val LocalAccentFill = staticCompositionLocalOf { Color(0xff2468c9) to Color.White }

@Composable fun AccentCheckbox(checked: Boolean, onCheckedChange: ((Boolean) -> Unit)?, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val (fill, text) = LocalAccentFill.current
    Checkbox(checked, onCheckedChange, modifier = modifier, enabled = enabled,
        colors = CheckboxDefaults.colors(checkedColor = fill, checkmarkColor = text))
}
@Composable fun AccentSwitch(checked: Boolean, onCheckedChange: ((Boolean) -> Unit)?, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val (fill, text) = LocalAccentFill.current
    Switch(checked, onCheckedChange, modifier = modifier, enabled = enabled,
        colors = SwitchDefaults.colors(checkedTrackColor = fill, checkedThumbColor = text, checkedBorderColor = fill))
}

@Composable fun AccentSettingsUi(settings: AccentSettings, update: (AccentSettings) -> Unit) {
    var draft by rememberSaveable { mutableStateOf(settings.customColor ?: "#2468C9") }
    var previousSaved by remember { mutableStateOf(settings.customColor) }
    LaunchedEffect(settings.customColor) {
        if (draft == (previousSaved ?: "#2468C9")) draft = settings.customColor ?: "#2468C9"
        previousSaved = settings.customColor
    }
    val color = parseAccentHex(draft)
    fun apply() {
        normalizeAccentHex(draft)?.let { value -> draft = value; update(settings.copy(mode = "custom", customColor = value)) }
    }
    Text(t("강조 색상"), style = MaterialTheme.typography.titleSmall)
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        listOf("system" to t("시스템"), "default" to t("DaGaLi 기본"), "custom" to t("사용자 지정")).forEach { (mode, label) ->
            FilterChip(settings.mode == mode, { update(settings.copy(mode = mode)) }, label = { Text(label) })
        }
    }
    // Retain the draft when changing mode. Presets always apply and save immediately.
    FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        accentPresets.forEach { (label, hex) ->
                FilterChip(settings.mode == "custom" && settings.customColor == hex,
                    { draft = hex; update(settings.copy(mode = "custom", customColor = hex)) },
                    label = { Text(label) }, leadingIcon = {
                        Box(Modifier.size(12.dp).background(Color(0xff000000 or parseAccentHex(hex)!!.toLong())))
                    })
        }
    }
    OutlinedTextField(draft, { draft = it }, singleLine = true, label = { Text("#RRGGBB") }, isError = color == null,
        supportingText = { if (color == null) Text(t("#RRGGBB 형식으로 입력하세요. 기존 색상을 유지합니다.")) },
        trailingIcon = { Box(Modifier.size(24.dp).border(1.dp, MaterialTheme.colorScheme.outline)
            .background(color?.let { Color(0xff000000 or it.toLong()) } ?: Color.Transparent)) },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { apply() }),
        modifier = Modifier.fillMaxWidth().onPreviewKeyEvent {
            if (it.key == Key.Enter && it.type == KeyEventType.KeyUp) { apply(); true } else false
        })
    TextButton({ apply() }, enabled = color != null) { Text(t("색상 적용")) }
}
