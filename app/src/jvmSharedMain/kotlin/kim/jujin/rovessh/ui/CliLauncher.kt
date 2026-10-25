package kim.jujin.rovessh.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.ViewConfiguration
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.*
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.*
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.core.TerminalKey

internal enum class CliCommand(val wire: String) {
    Claude("claude\r"), ClaudeBypass("claude --dangerously-skip-permissions\r"),
    Codex("codex\r"), CodexBypass("codex --yolo\r")
}

/** One selection can send once, only to the connection/pane on which it opened. */
internal class CliLaunchRequest(private val client: ClientConnection) {
    private val phase = client.status.value.phase
    private val session = client.session.value
    private val pane = client.pane.value
    private var consumed = false

    fun send(command: CliCommand, current: ClientConnection, allowed: Boolean): Boolean {
        if (consumed) return false
        consumed = true
        if (!allowed || current !== client || !client.status.value.acceptsInput ||
            client.status.value.phase != phase || client.session.value != session || client.pane.value != pane) return false
        client.input(command.wire.encodeToByteArray())
        return true
    }
}

internal class TerminalFocus {
    var request: () -> Unit = {}
    var keyboard: () -> Unit = {}
    var imeVisible by mutableStateOf(false)
    var preparePaste: suspend (String, Boolean) -> ByteArray = { _, _ -> error("터미널이 아직 준비되지 않았습니다.") }
    var inputAllowed: () -> Boolean = { true }
}
internal val LocalTerminalFocus = staticCompositionLocalOf { TerminalFocus() }

private object AboveLauncher : PopupPositionProvider {
    override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize) =
        IntOffset((anchorBounds.right - popupContentSize.width).coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0)),
            (anchorBounds.top - popupContentSize.height).coerceAtLeast(0))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable internal fun TerminalShortcutBar(client: ClientConnection, shortcuts: List<TerminalKey>, allowed: () -> Boolean, terminalFocus: TerminalFocus,
    mobile: Boolean = false, openComposer: () -> Unit = {}) {
    val status by client.status.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val available = allowed()
    // A target/phase/dialog change discards the request before painting or handling another click.
    var request by remember(client, status.phase, session, pane, available) { mutableStateOf<CliLaunchRequest?>(null) }
    val dismiss = { request = null; terminalFocus.request() }
    val buttonShape = RoundedCornerShape(6.dp)
    val buttonBorder = BorderStroke(1.dp, MaterialTheme.colorScheme.outline)
    val buttonColors = ButtonDefaults.outlinedButtonColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
    val buttonHeight = if (mobile) 40.dp else 48.dp
    val imeVisible = terminalFocus.imeVisible
    val viewConfiguration = LocalViewConfiguration.current
    val compactConfiguration = remember(viewConfiguration) { object : ViewConfiguration by viewConfiguration {
        override val minimumTouchTargetSize = DpSize(40.dp, 40.dp)
    } }
    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides if (mobile) 0.dp else LocalMinimumInteractiveComponentSize.current,
        LocalViewConfiguration provides if (mobile) compactConfiguration else viewConfiguration) {
        Row(Modifier.fillMaxWidth().semantics { contentDescription = "터미널 입력 도구" }
            .padding(horizontal = 4.dp, vertical = if (mobile) 4.dp else 0.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            if (mobile) {
                OutlinedButton({ terminalFocus.keyboard() }, enabled = available,
                    shape = buttonShape, border = buttonBorder, colors = buttonColors, contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.size(buttonHeight).semantics {
                        contentDescription = if (imeVisible) "키보드 닫기" else "키보드 열기"
                        stateDescription = if (imeVisible) "표시됨" else "숨김"
                    }) { KeyboardIcon() }
                OutlinedButton(openComposer, enabled = available, shape = buttonShape, border = buttonBorder,
                    colors = ButtonDefaults.outlinedButtonColors(containerColor = MaterialTheme.colorScheme.surface),
                    modifier = Modifier.height(buttonHeight).widthIn(min = 40.dp).semantics { contentDescription = "텍스트 입력 열기" },
                    contentPadding = PaddingValues(horizontal = 8.dp)) { Text("텍스트 입력", fontSize = 11.sp, maxLines = 1) }
            }
            Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                shortcuts.distinct().forEach { shortcut ->
                    OutlinedButton({
                        if (allowed() && client.status.value.acceptsInput) {
                            client.input(shortcut.bytes())
                            terminalFocus.request()
                        }
                    }, enabled = available && status.acceptsInput, shape = buttonShape, border = buttonBorder, colors = buttonColors,
                        modifier = Modifier.height(buttonHeight).widthIn(min = if (mobile) 46.dp else 58.dp),
                        contentPadding = PaddingValues(horizontal = if (mobile) 8.dp else 10.dp)) {
                        Text(shortcut.label, maxLines = 1, fontSize = if (mobile) 11.sp else 14.sp)
                    }
                }
            }
            Box {
                OutlinedButton(onClick = { request = if (request == null) CliLaunchRequest(client) else null },
                    shape = buttonShape, border = buttonBorder, colors = buttonColors, contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.size(buttonHeight).semantics { contentDescription = "CLI 실행 메뉴" }) { Text("…", fontSize = if (mobile) 18.sp else 22.sp) }
                val opened = request
                if (opened != null) Popup(popupPositionProvider = AboveLauncher, onDismissRequest = dismiss,
                    properties = PopupProperties(focusable = true)) {
                    val menuFocus = remember { FocusRequester() }
                    LaunchedEffect(Unit) { menuFocus.requestFocus() }
                    Surface(shadowElevation = 8.dp, border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
                        Column(Modifier.width(300.dp).onPreviewKeyEvent {
                            if (it.key == Key.Escape && it.type == KeyEventType.KeyDown) { dismiss(); true } else false
                        }.focusRequester(menuFocus).focusable()) {
                            val launch: (CliCommand) -> Unit = { command ->
                                if (request === opened) {
                                    request = null
                                    opened.send(command, client, allowed())
                                    terminalFocus.request()
                                }
                            }
                            LauncherRow("Claude Code", LauncherIcons.claude, available && status.acceptsInput,
                                { launch(CliCommand.Claude) }, { launch(CliCommand.ClaudeBypass) })
                            LauncherRow("Codex", LauncherIcons.openai, available && status.acceptsInput,
                                { launch(CliCommand.Codex) }, { launch(CliCommand.CodexBypass) })
                        }
                    }
                }
            }
        }
    }
}

@Composable private fun LauncherRow(service: String, icon: ImageVector, enabled: Boolean, normal: () -> Unit, bypass: () -> Unit) {
    Row(Modifier.fillMaxWidth().height(48.dp)) {
        LauncherItem(service, "$service 실행", icon, enabled, Modifier.weight(0.6f), normal)
        LauncherItem("Bypass", "$service Bypass 실행", LauncherIcons.bolt, enabled, Modifier.weight(0.4f), bypass)
    }
}

@Composable private fun LauncherItem(label: String, description: String, icon: ImageVector, enabled: Boolean, modifier: Modifier, action: () -> Unit) {
    val color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (enabled) 1f else 0.38f)
    Row(modifier.fillMaxHeight().clickable(enabled = enabled, role = Role.Button, onClick = action)
        .semantics(mergeDescendants = true) { contentDescription = description }.padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp), tint = color)
        Text(label, color = color, style = MaterialTheme.typography.labelLarge, maxLines = 1)
    }
}
