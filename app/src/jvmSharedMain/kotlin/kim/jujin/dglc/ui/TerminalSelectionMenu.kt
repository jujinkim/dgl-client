package kim.jujin.dglc.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.layout.IntrinsicMeasurable
import androidx.compose.ui.layout.IntrinsicMeasureScope
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.layout.MeasurePolicy
import androidx.compose.ui.layout.MeasureResult
import androidx.compose.ui.layout.MeasureScope
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Constraints
import androidx.compose.foundation.layout.*
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.t

/**
 * A finished long-press selection in one pane. [target] is the bridge's input target when that
 * pane was the input pane, [url] a validated leading http(s) link, [bounds] the selection's
 * visible rows in terminal-view pixels, and [lineBreaks] whether its copied text has line breaks.
 */
data class TerminalSelection(val pane: String, val target: String?, val url: String?, val bounds: Rect,
    val lineBreaks: Boolean = false)

/**
 * Copy, paste and link actions for a terminal selection. It never takes focus, so the terminal
 * keeps the IME. A null [paste] hides paste for panes other than the input pane, and a null
 * [copyWithoutLineBreaks] hides that copy for a selection without line breaks.
 */
@Composable internal fun TerminalSelectionMenu(url: String?, paste: (() -> Unit)?, pasteEnabled: Boolean,
    copy: () -> Unit, copyWithoutLineBreaks: (() -> Unit)?, openLink: () -> Unit, copyLink: () -> Unit) {
    Surface(modifier = Modifier.semantics { paneTitle = t("terminal.selection_menu") }, shadowElevation = 8.dp,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
        Column(Modifier.widthIn(min = 160.dp, max = 320.dp).width(IntrinsicSize.Max).focusProperties { canFocus = false }) {
            if (url != null) {
                Text(url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp))
                HorizontalDivider()
            }
            if (copyWithoutLineBreaks == null) SelectionAction(t("terminal.copy"), copy)
            else CopyActions(copy, copyWithoutLineBreaks)
            if (paste != null) SelectionAction(t("common.paste"), paste, pasteEnabled)
            if (url != null) {
                SelectionAction(t("terminal.open_link"), openLink)
                SelectionAction(t("terminal.copy_link"), copyLink)
            }
        }
    }
}

@Composable private fun SelectionAction(label: String, onClick: () -> Unit, enabled: Boolean = true) {
    Row(Modifier.fillMaxWidth().height(48.dp).focusProperties { canFocus = false }
        .clickable(enabled = enabled, role = Role.Button, onClick = onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (enabled) 1f else .38f))
    }
}

/**
 * Copy with a compact line-break-free copy at the end of its row. When the menu is too narrow
 * for both, the second copy becomes its own row below.
 */
@Composable private fun CopyActions(copy: () -> Unit, copyWithoutLineBreaks: () -> Unit) {
    val label = t("terminal.copy_without_line_breaks")
    Layout(content = {
        SelectionAction(t("terminal.copy"), copy)
        Box(Modifier.height(48.dp).focusProperties { canFocus = false }
            .clickable(role = Role.Button, onClick = copyWithoutLineBreaks)
            .semantics { contentDescription = label }.padding(horizontal = 8.dp),
            contentAlignment = Alignment.Center) {
            Text(t("terminal.no_line_breaks"), style = MaterialTheme.typography.labelMedium, maxLines = 1,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(12.dp))
                    .padding(horizontal = 10.dp, vertical = 4.dp))
        }
        SelectionAction(label, copyWithoutLineBreaks)
    }, measurePolicy = object : MeasurePolicy {
        override fun MeasureScope.measure(measurables: List<Measurable>, constraints: Constraints): MeasureResult {
            val (row, compact, separate) = measurables
            val loose = constraints.copy(minWidth = 0, minHeight = 0)
            val button = compact.measure(loose)
            val inline = row.maxIntrinsicWidth(button.height) + button.width
            val width = if (constraints.hasBoundedWidth) constraints.maxWidth else inline
            return if (inline <= width) {
                val copyRow = row.measure(loose.copy(maxWidth = width - button.width))
                layout(width, maxOf(copyRow.height, button.height)) {
                    copyRow.place(0, 0); button.place(width - button.width, 0)
                }
            } else {
                val copyRow = row.measure(loose); val own = separate.measure(loose)
                layout(width, copyRow.height + own.height) { copyRow.place(0, 0); own.place(0, copyRow.height) }
            }
        }
        // The menu sizes itself to the inline arrangement; narrower constraints fall back to two rows.
        override fun IntrinsicMeasureScope.maxIntrinsicWidth(measurables: List<IntrinsicMeasurable>, height: Int) =
            measurables[0].maxIntrinsicWidth(height) + measurables[1].maxIntrinsicWidth(height)
        override fun IntrinsicMeasureScope.minIntrinsicWidth(measurables: List<IntrinsicMeasurable>, height: Int) =
            maxOf(measurables[0].minIntrinsicWidth(height), measurables[2].minIntrinsicWidth(height))
        override fun IntrinsicMeasureScope.maxIntrinsicHeight(measurables: List<IntrinsicMeasurable>, width: Int) =
            measurables[0].maxIntrinsicHeight(width) + measurables[2].maxIntrinsicHeight(width)
        override fun IntrinsicMeasureScope.minIntrinsicHeight(measurables: List<IntrinsicMeasurable>, width: Int) =
            measurables[0].minIntrinsicHeight(width)
    })
}
