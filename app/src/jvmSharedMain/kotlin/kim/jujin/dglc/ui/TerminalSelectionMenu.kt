package kim.jujin.dglc.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
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
 * pane was the input pane, [url] a validated leading http(s) link, and [bounds] the selection's
 * visible rows in terminal-view pixels.
 */
data class TerminalSelection(val pane: String, val target: String?, val url: String?, val bounds: Rect)

/**
 * Copy, paste and link actions for a terminal selection. It never takes focus, so the terminal
 * keeps the IME. A null [paste] hides paste for panes other than the input pane.
 */
@Composable internal fun TerminalSelectionMenu(url: String?, paste: (() -> Unit)?, pasteEnabled: Boolean,
    copy: () -> Unit, openLink: () -> Unit, copyLink: () -> Unit) {
    Surface(modifier = Modifier.semantics { paneTitle = t("터미널 선택 메뉴") }, shadowElevation = 8.dp,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
        Column(Modifier.widthIn(min = 160.dp, max = 320.dp).width(IntrinsicSize.Max).focusProperties { canFocus = false }) {
            if (url != null) {
                Text(url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp))
                HorizontalDivider()
            }
            SelectionAction(t("복사"), copy)
            if (paste != null) SelectionAction(t("붙여넣기"), paste, pasteEnabled)
            if (url != null) {
                SelectionAction(t("링크 열기"), openLink)
                SelectionAction(t("링크 복사"), copyLink)
            }
        }
    }
}

@Composable private fun SelectionAction(label: String, onClick: () -> Unit, enabled: Boolean = true) {
    Row(Modifier.fillMaxWidth().height(48.dp).focusProperties { canFocus = false }
        .clickable(enabled = enabled, role = Role.Button, onClick = onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1,
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (enabled) 1f else .38f))
    }
}
