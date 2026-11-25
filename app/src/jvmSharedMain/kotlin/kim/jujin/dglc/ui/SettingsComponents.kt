package kim.jujin.dglc.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/** Spacing shared by every settings page. The rest of the app keeps its compact sizes. */
internal object SettingsSpacing {
    val page = 16.dp
    val pageWide = 24.dp
    val groupGap = 20.dp
    val rowMinHeight = 56.dp
    val checkRowHeight = 48.dp
    val rowH = 16.dp
    val rowV = 12.dp
    val controlGap = 8.dp
    val navWidth = 232.dp
    val navRow = 52.dp
    val buttonPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp)
    val controlHeight = 40.dp
}

/** Settings-only shapes; the app theme keeps its square corners. */
internal object SettingsShapes {
    val group = RoundedCornerShape(12.dp)
    val control = RoundedCornerShape(8.dp)
}

internal enum class SettingsActionStyle { Outlined, Primary, Danger }

@Composable private fun disabledText(enabled: Boolean): Color =
    if (enabled) Color.Unspecified else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)

/** Page title (two-pane) and the always-visible page description. */
@Composable internal fun SettingsPageHeader(title: String?, description: String?) {
    if (title == null && description == null) return
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        title?.let { Text(it, Modifier.semantics { heading() }, style = MaterialTheme.typography.headlineSmall) }
        description?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}

/** Section heading used by groups and by subsections inside a page. */
@Composable internal fun SettingsSubheading(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier.semantics { heading() }, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
}

/** A titled card. The title and description sit outside the card so they read as a section heading. */
@Composable internal fun SettingsGroup(title: String?, description: String? = null, modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (title != null) SettingsSubheading(title, Modifier.padding(horizontal = 4.dp))
        if (description != null) Text(description, Modifier.padding(horizontal = 4.dp),
            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Surface(Modifier.fillMaxWidth(), shape = SettingsShapes.group, color = MaterialTheme.colorScheme.surfaceContainer,
            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
            Column(Modifier.fillMaxWidth(), content = content)
        }
    }
}

@Composable internal fun SettingsDivider() {
    HorizontalDivider(Modifier.padding(horizontal = SettingsSpacing.rowH), color = MaterialTheme.colorScheme.outlineVariant)
}

/**
 * A switch row. Only the label sits inside the toggleable row, so its node text is exactly the label;
 * the description is a sibling below it.
 */
@Composable internal fun SettingsToggleRow(label: String, checked: Boolean, description: String? = null, enabled: Boolean = true,
    onChange: (Boolean) -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight)
            .toggleable(checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = disabledText(enabled))
            Spacer(Modifier.width(12.dp))
            AccentSwitch(checked, null, enabled = enabled)
        }
        description?.let {
            Text(it, Modifier.fillMaxWidth().padding(start = SettingsSpacing.rowH, end = SettingsSpacing.rowH, bottom = SettingsSpacing.rowV),
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/** A checkbox row; the whole row toggles. */
@Composable internal fun SettingsCheckRow(label: String, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.checkRowHeight)
        .toggleable(checked, enabled = enabled, role = Role.Checkbox, onValueChange = onChange)
        .padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        AccentCheckbox(checked, null, enabled = enabled)
        Spacer(Modifier.width(8.dp))
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = disabledText(enabled))
    }
}

/** A single choice among chips. Each chip keeps its own label as its node text. */
@Composable internal fun SettingsChoiceRow(label: String?, options: List<Pair<String, String>>, selected: String, enabled: Boolean = true,
    description: String? = null, onSelect: (String) -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = SettingsSpacing.rowV),
        verticalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
        label?.let { Text(it, style = MaterialTheme.typography.bodyLarge, color = disabledText(enabled)) }
        SettingsChips {
            options.forEach { (value, text) -> SettingsChip(text, selected == value, enabled) { onSelect(value) } }
        }
        description?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}

@Composable internal fun SettingsChips(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap),
        verticalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap), content = content)
}

@Composable internal fun SettingsChip(label: String, selected: Boolean, enabled: Boolean = true, leadingIcon: (@Composable () -> Unit)? = null,
    onClick: () -> Unit) {
    FilterChip(selected, onClick, label = { Text(label) }, enabled = enabled, leadingIcon = leadingIcon,
        shape = SettingsShapes.control, modifier = Modifier.heightIn(min = SettingsSpacing.controlHeight))
}

/**
 * A row that opens a page or dialog. A value is shown but hidden from the node text and
 * reported as the state instead, so the row is still found by its label alone.
 */
@Composable internal fun SettingsNavRow(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, value: String? = null,
    enabled: Boolean = true) {
    Row(modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight)
        .semantics { if (value != null) stateDescription = value }
        .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
        .padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = disabledText(enabled))
        value?.let {
            Text(it, Modifier.widthIn(max = 200.dp).clearAndSetSemantics {}, style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        UiGlyph(UiIcons.chevronRight, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** A label with an optional description on the left and controls on the right. */
@Composable internal fun SettingsRow(label: String, description: String? = null, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight)
        .padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            description?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
        trailing()
    }
}

/** Buttons that wrap onto further lines inside a group. */
@Composable internal fun SettingsActions(content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = SettingsSpacing.rowV),
        horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap),
        verticalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap), content = content)
}

/** Roomier text button for settings; [BorderedButton]'s compact defaults stay unchanged elsewhere. */
@Composable internal fun SettingsActionButton(label: String, onClick: () -> Unit, enabled: Boolean = true,
    style: SettingsActionStyle = SettingsActionStyle.Outlined, modifier: Modifier = Modifier) {
    val sized = modifier.heightIn(min = SettingsSpacing.controlHeight)
    when (style) {
        SettingsActionStyle.Outlined -> BorderedButton(onClick, sized, enabled, shape = SettingsShapes.control,
            contentPadding = SettingsSpacing.buttonPadding) { Text(label) }
        SettingsActionStyle.Primary -> {
            val (fill, text) = LocalAccentFill.current
            Button(onClick, sized, enabled, shape = SettingsShapes.control, contentPadding = SettingsSpacing.buttonPadding,
                colors = ButtonDefaults.buttonColors(containerColor = fill, contentColor = text)) { Text(label) }
        }
        SettingsActionStyle.Danger -> {
            val danger = MaterialTheme.colorScheme.error
            OutlinedButton(onClick, sized, enabled, shape = SettingsShapes.control, contentPadding = SettingsSpacing.buttonPadding,
                colors = ButtonDefaults.outlinedButtonColors(contentColor = danger),
                border = BorderStroke(1.dp, if (enabled) danger else MaterialTheme.colorScheme.outlineVariant)) { Text(label) }
        }
    }
}

/** Muted explanatory text, inset to line up with row labels. */
@Composable internal fun SettingsNote(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.rowH, vertical = 6.dp),
        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
