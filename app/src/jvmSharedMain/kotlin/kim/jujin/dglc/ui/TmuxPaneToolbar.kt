package kim.jujin.dglc.ui

import kim.jujin.dglc.core.t
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.ClientConnection
import kim.jujin.dglc.core.Phase
import kim.jujin.dglc.core.TmuxCommand

@Composable internal fun TmuxPaneToolbar(client: ClientConnection, returnToTerminal: () -> Unit, closePane: (String) -> Unit) {
    val status by client.status.collectAsState()
    val pane by client.pane.collectAsState()
    val enabled = status.phase == Phase.Ready && pane != null
    fun manage(command: (String) -> TmuxCommand) {
        if (client.status.value.phase == Phase.Ready) {
            client.pane.value?.let {
                client.manage(command(it))
                returnToTerminal()
            }
        }
    }
    Surface(color = MaterialTheme.colorScheme.surfaceVariant) {
        Column(Modifier.fillMaxWidth().semantics { contentDescription = t("pane.tools") }) {
            HorizontalDivider()
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    PaneToolCategory(t("pane.split")) {
                        PaneToolButton(UiIcons.splitHorizontal, t("pane.split_horizontally"), enabled) { manage { TmuxCommand.split(it, true) } }
                        PaneToolButton(UiIcons.splitVertical, t("pane.split_vertically"), enabled) { manage { TmuxCommand.split(it, false) } }
                    }
                    VerticalDivider(Modifier.height(56.dp))
                    PaneToolCategory(t("pane.navigate")) {
                        listOf(
                            Triple(UiIcons.arrowLeft, 'L', t("pane.focus_pane_left")),
                            Triple(UiIcons.arrowRight, 'R', t("pane.focus_pane_right")),
                            Triple(UiIcons.arrowUp, 'U', t("pane.focus_pane_above")),
                            Triple(UiIcons.arrowDown, 'D', t("pane.focus_pane_below")),
                        ).forEach { (icon, direction, description) ->
                            PaneToolButton(icon, description, enabled) { manage { TmuxCommand.focusPane(it, direction) } }
                        }
                    }
                    VerticalDivider(Modifier.height(56.dp))
                    PaneToolCategory(t("pane.resize")) {
                        listOf(
                            Triple(UiIcons.arrowLeft, 'L', t("pane.resize_left")),
                            Triple(UiIcons.arrowRight, 'R', t("pane.resize_right")),
                            Triple(UiIcons.arrowUp, 'U', t("pane.resize_up")),
                            Triple(UiIcons.arrowDown, 'D', t("pane.resize_down")),
                        ).forEach { (icon, direction, description) ->
                            PaneToolButton(icon, description, enabled) { manage { TmuxCommand.resizePane(it, direction) } }
                        }
                    }
                }
                VerticalDivider(Modifier.height(56.dp))
                PaneToolCategory(t("common.close")) {
                    PaneToolButton(UiIcons.close, t("pane.close_current"), enabled) {
                        if (client.status.value.phase == Phase.Ready) client.pane.value?.let(closePane)
                    }
                }
            }
            HorizontalDivider()
        }
    }
}

@Composable private fun PaneToolCategory(title: String, content: @Composable RowScope.() -> Unit) {
    Column(Modifier.padding(horizontal = 4.dp, vertical = 2.dp)) {
        Text(title, Modifier.padding(start = 8.dp), style = MaterialTheme.typography.labelSmall)
        Row(verticalAlignment = Alignment.CenterVertically, content = content)
    }
}

@Composable private fun PaneToolButton(icon: androidx.compose.ui.graphics.vector.ImageVector, description: String, enabled: Boolean, action: () -> Unit) {
    IconButton(action, enabled = enabled, modifier = Modifier.size(48.dp).semantics { contentDescription = description }) {
        UiGlyph(icon)
    }
}
