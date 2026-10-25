package kim.jujin.rovessh.ui

import kim.jujin.rovessh.core.t
import kim.jujin.rovessh.core.tr
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
import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.core.Phase
import kim.jujin.rovessh.core.TmuxCommand

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
        Column(Modifier.fillMaxWidth().semantics { contentDescription = t("tmux 패널 도구") }) {
            HorizontalDivider()
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    PaneToolCategory(t("패널 분할")) {
                        PaneToolButton("↔", t("tmux 패널 분할 ↔"), enabled) { manage { TmuxCommand.split(it, true) } }
                        PaneToolButton("↕", t("tmux 패널 분할 ↕"), enabled) { manage { TmuxCommand.split(it, false) } }
                    }
                    VerticalDivider(Modifier.height(56.dp))
                    PaneToolCategory(t("이동")) {
                        listOf("←" to 'L', "→" to 'R', "↑" to 'U', "↓" to 'D').forEach { (arrow, direction) ->
                            PaneToolButton(arrow, tr("포커스 {0}", arrow), enabled) { manage { TmuxCommand.focusPane(it, direction) } }
                        }
                    }
                    VerticalDivider(Modifier.height(56.dp))
                    PaneToolCategory(t("크기 조절")) {
                        listOf("←" to 'L', "→" to 'R', "↑" to 'U', "↓" to 'D').forEach { (arrow, direction) ->
                            PaneToolButton(arrow, tr("경계{0}", arrow), enabled) { manage { TmuxCommand.resizePane(it, direction) } }
                        }
                    }
                }
                VerticalDivider(Modifier.height(56.dp))
                PaneToolCategory(t("닫기")) {
                    PaneToolButton("×", t("현재 tmux 패널 닫기"), enabled) {
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

@Composable private fun PaneToolButton(label: String, description: String, enabled: Boolean, action: () -> Unit) {
    IconButton(action, enabled = enabled, modifier = Modifier.size(48.dp).semantics { contentDescription = description }) {
        Text(label)
    }
}
