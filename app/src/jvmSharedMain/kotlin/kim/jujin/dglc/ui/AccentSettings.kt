package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
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
    // Rendered inside a settings group titled with the accent color.
    SettingsChoiceRow(null, listOf("system" to t("common.system"), "default" to t("accent.dagali_default"), "custom" to t("accent.custom")),
        settings.mode) { mode -> update(settings.copy(mode = mode)) }
    SettingsDivider()
    // Retain the draft when changing mode. Presets always apply and save immediately.
    Box(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = SettingsSpacing.rowV)) {
        SettingsChips {
            accentPresets.forEach { (label, hex) ->
                SettingsChip(label, settings.mode == "custom" && settings.customColor == hex, leadingIcon = {
                    Box(Modifier.size(14.dp).background(Color(0xff000000 or parseAccentHex(hex)!!.toLong()), SettingsShapes.control))
                }) { draft = hex; update(settings.copy(mode = "custom", customColor = hex)) }
            }
        }
    }
    SettingsDivider()
    Row(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = SettingsSpacing.rowV),
        verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
        OutlinedTextField(draft, { draft = it }, singleLine = true, label = { Text("#RRGGBB") }, isError = color == null,
            supportingText = { if (color == null) Text(t("accent.invalid_hex")) }, shape = SettingsShapes.control,
            trailingIcon = { Box(Modifier.size(24.dp).border(1.dp, MaterialTheme.colorScheme.outline)
                .background(color?.let { Color(0xff000000 or it.toLong()) } ?: Color.Transparent)) },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { apply() }),
            modifier = Modifier.weight(1f).onPreviewKeyEvent {
                if (it.key == Key.Enter && it.type == KeyEventType.KeyUp) { apply(); true } else false
            })
        SettingsActionButton(t("accent.apply_color"), { apply() }, enabled = color != null, style = SettingsActionStyle.Primary,
            modifier = Modifier.padding(top = 8.dp))
    }
}
