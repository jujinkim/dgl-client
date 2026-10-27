package kim.jujin.dglc.ui

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.*
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.zIndex
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.ViewConfiguration
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.*
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.*
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import kim.jujin.dglc.ClientConnection
import kim.jujin.dglc.core.ConnectionStatus
import kim.jujin.dglc.core.Phase
import kim.jujin.dglc.core.TerminalShortcut

internal enum class CliCommand(val wire: String) {
    Claude("claude\r"), ClaudeBypass("claude --dangerously-skip-permissions\r"),
    Codex("codex\r"), CodexBypass("codex --yolo\r"),
    Agy("agy\r"), AgyBypass("agy --dangerously-skip-permissions\r"),
    Grok("grok\r"), GrokBypass("grok --always-approve\r")
}

/** A viewport snapshot keeps the same input destination and must not dismiss its menu. */
private val ConnectionStatus.cliLaunchPhase: Phase
    get() = if (phase == Phase.Restoring && resizing && acceptsInput) Phase.Ready else phase

/** One selection can send once, only to the connection/pane on which it opened. */
internal class CliLaunchRequest(private val client: ClientConnection) {
    private val phase = client.status.value.cliLaunchPhase
    private val session = client.session.value
    private val pane = client.pane.value
    private val target = client.captureInputTarget()
    private var consumed = false

    fun send(command: CliCommand, current: ClientConnection, allowed: Boolean): Boolean {
        if (consumed) return false
        consumed = true
        if (!allowed || current !== client || !client.status.value.acceptsInput ||
            client.status.value.cliLaunchPhase != phase || client.session.value != session || client.pane.value != pane ||
            client.captureInputTarget() != target) return false
        client.input(command.wire.encodeToByteArray())
        return true
    }
}

internal class TerminalFocus {
    var shortcut: (TerminalShortcut) -> Unit = {}
    var activate: () -> Unit = {}
    var request: () -> Unit = {}
    var keyboard: () -> Unit = {}
    var imeVisible by mutableStateOf(false)
    var preparePaste: suspend (String) -> ByteArray = { _ -> error(t("터미널이 아직 준비되지 않았습니다.")) }
    var inputAllowed: () -> Boolean = { true }
}
internal val LocalTerminalFocus = staticCompositionLocalOf { TerminalFocus() }

private object AboveLauncher : PopupPositionProvider {
    override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize) =
        IntOffset((anchorBounds.right - popupContentSize.width).coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0)),
            (anchorBounds.top - popupContentSize.height).coerceAtLeast(0))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable internal fun TerminalShortcutBar(client: ClientConnection, shortcuts: List<TerminalShortcut>, allowed: () -> Boolean, terminalFocus: TerminalFocus,
    mobile: Boolean = false, openComposer: () -> Unit = {}, workspace: Boolean = false, mobileShortcutBarTwoRows: Boolean = true,
    inputGate: kim.jujin.dglc.AppModel? = null) {
    val shortcutInput = inputGate?.shortcutInput(client)
    val modifiers = shortcutInput?.modifiers?.collectAsState()?.value
    val status by client.status.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val menuOwner = remember { Any() }
    val blocks = inputGate?.inputBlocks?.collectAsState()?.value
    val menuAllowed = { allowed() && (blocks == null || inputGate?.inputBlocked(menuOwner) != true) }
    val available = menuAllowed()
    SideEffect { shortcutInput?.sync() }
    // A target/phase/dialog change discards the request before painting or handling another click.
    var request by remember(client, status.cliLaunchPhase, session, pane, available) { mutableStateOf<CliLaunchRequest?>(null) }
    if (inputGate != null) WorkspaceInputBlock(inputGate, menuOwner, request != null)
    val dismiss: () -> Unit = { request = null; inputGate?.blockInput(menuOwner, false) }
    val compact = mobile && !workspace
    val twoRows = compact && mobileShortcutBarTwoRows
    val buttonHeight = if (compact) 30.dp else 48.dp
    val columnHeight = buttonHeight * 2 + 4.dp
    val imeVisible = terminalFocus.imeVisible
    val viewConfiguration = LocalViewConfiguration.current
    val compactConfiguration = remember(viewConfiguration, compact) { object : ViewConfiguration by viewConfiguration {
        override val minimumTouchTargetSize = if (compact) DpSize(30.dp, 30.dp) else DpSize(40.dp, 40.dp)
    } }
    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides if (mobile) 0.dp else LocalMinimumInteractiveComponentSize.current,
        LocalViewConfiguration provides if (mobile) compactConfiguration else viewConfiguration) {
        Row(Modifier.fillMaxWidth().semantics { contentDescription = t("터미널 입력 도구") }
            .padding(horizontal = 4.dp, vertical = if (compact) 3.dp else if (mobile) 4.dp else 0.dp), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            val mobileTools: @Composable () -> Unit = {
                TerminalToolButton({ terminalFocus.keyboard() }, enabled = available,
                    contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.size(buttonHeight).semantics {
                        contentDescription = if (imeVisible) t("키보드 닫기") else t("키보드 열기")
                        stateDescription = if (imeVisible) t("표시됨") else t("숨김")
                    }) { KeyboardIcon() }
                TerminalToolButton(openComposer, enabled = available,
                    modifier = Modifier.size(buttonHeight).semantics { contentDescription = t("텍스트 작성 열기") },
                    contentPadding = PaddingValues(0.dp)) { UiGlyph(UiIcons.compose, Modifier.size(18.dp)) }
            }
            if (mobile) {
                // Keep fixed tools above partially clipped shortcut hit/accessibility bounds.
                if (twoRows) Column(Modifier.zIndex(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) { mobileTools() }
                else Row(Modifier.zIndex(1f), horizontalArrangement = Arrangement.spacedBy(4.dp)) { mobileTools() }
            }
            val shortcutButton: @Composable (TerminalShortcut, Modifier) -> Unit = { shortcut, modifier ->
                TerminalToolButton({
                    if (menuAllowed() && client.status.value.acceptsInput) {
                        if (shortcut.key == null) shortcutInput?.toggle(shortcut)
                        else terminalFocus.shortcut(shortcut)
                        terminalFocus.request()
                    }
                }, enabled = available && status.acceptsInput,
                    containerColor = if (modifiers?.active(shortcut) == true) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                    modifier = modifier.height(buttonHeight).widthIn(min = if (mobile) 46.dp else 58.dp).semantics {
                        if (shortcut.key == null) stateDescription = t(if (modifiers?.active(shortcut) == true) "켜짐" else "꺼짐")
                    },
                    contentPadding = PaddingValues(horizontal = if (mobile) 8.dp else 10.dp)) {
                    Text(shortcut.label, maxLines = 1, fontSize = if (mobile) 11.sp else 14.sp)
                }
            }
            Box(Modifier.weight(1f).clipToBounds().horizontalScroll(rememberScrollState())) {
                if (twoRows) {
                    Column(Modifier.height(columnHeight), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        val ordered = shortcuts.distinct()
                        repeat(2) { row ->
                            Row(Modifier.height(buttonHeight), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                ordered.filterIndexed { index, _ -> index % 2 == row }.forEach { shortcutButton(it, Modifier) }
                            }
                        }
                    }
                } else Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    shortcuts.distinct().forEach { shortcut -> shortcutButton(shortcut, Modifier) }
                }
            }
            val density = LocalDensity.current
            val safeTop = WindowInsets.safeDrawing.getTop(density)
            var launcherTop by remember { mutableIntStateOf(0) }
            // Anchor and height limit use the entire fixed column, including the empty upper cell.
            Box(Modifier.height(if (twoRows) columnHeight else buttonHeight)
                .onGloballyPositioned { launcherTop = it.boundsInWindow().top.toInt() }, contentAlignment = Alignment.BottomCenter) {
                TerminalToolButton(onClick = { request = if (request == null) CliLaunchRequest(client) else null },
                    contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.size(buttonHeight).semantics { contentDescription = t("CLI 실행 메뉴") }) { UiGlyph(UiIcons.more, Modifier.size(if (mobile) 18.dp else 20.dp)) }
                val opened = request
                if (opened != null) {
                    // Android posts the focus request so it runs after the popup window is removed.
                    DisposableEffect(opened, terminalFocus) { onDispose { terminalFocus.request() } }
                    Popup(popupPositionProvider = AboveLauncher, onDismissRequest = dismiss,
                        properties = imePreservingPopupProperties()) {
                        val menuFocus = remember { FocusRequester() }
                        LaunchedEffect(Unit) { menuFocus.requestFocus() }
                        Surface(modifier = Modifier.semantics { paneTitle = t("CLI 실행 메뉴") }, shadowElevation = 8.dp,
                            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
                            val heightLimit = if (mobile) Modifier.heightIn(max = with(density) {
                                (launcherTop - safeTop).coerceAtLeast(0).toDp()
                            }) else Modifier
                            Column(Modifier.width(IntrinsicSize.Max).then(heightLimit).verticalScroll(rememberScrollState()).onPreviewKeyEvent {
                                if (it.key == Key.Escape && it.type == KeyEventType.KeyDown) { dismiss(); true } else false
                            }.focusRequester(menuFocus).focusable()) {
                                val launch: (CliCommand) -> Unit = { command ->
                                    if (request === opened) {
                                        request = null
                                        inputGate?.blockInput(menuOwner, false)
                                        opened.send(command, client, menuAllowed())
                                    }
                                }
                                LauncherRow("Claude Code", LauncherIcons.claude, available && status.acceptsInput,
                                    { launch(CliCommand.Claude) }, { launch(CliCommand.ClaudeBypass) })
                                LauncherRow("Codex", LauncherIcons.openai, available && status.acceptsInput,
                                    { launch(CliCommand.Codex) }, { launch(CliCommand.CodexBypass) })
                                LauncherRow("AGY", LauncherIcons.agy, available && status.acceptsInput,
                                    { launch(CliCommand.Agy) }, { launch(CliCommand.AgyBypass) }, experimental = true)
                                LauncherRow("Grok Build", LauncherIcons.grok, available && status.acceptsInput,
                                    { launch(CliCommand.Grok) }, { launch(CliCommand.GrokBypass) }, experimental = true)
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Paint the whole button, including its border, with the same interaction color. */
@Composable private fun TerminalToolButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true,
    containerColor: Color = MaterialTheme.colorScheme.surfaceVariant, contentPadding: PaddingValues,
    content: @Composable RowScope.() -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val focused by interaction.collectIsFocusedAsState()
    val pressed by interaction.collectIsPressedAsState()
    val foreground = MaterialTheme.colorScheme.onSurface
    val alpha = if (!enabled) 0f else if (pressed || focused) .12f else if (hovered) .08f else 0f
    val fill = foreground.copy(alpha = alpha).compositeOver(containerColor)
    val shape = RoundedCornerShape(6.dp)
    CompositionLocalProvider(LocalContentColor provides foreground.copy(alpha = if (enabled) 1f else .38f)) {
        ProvideTextStyle(MaterialTheme.typography.labelLarge) {
            Row(modifier.clip(shape).background(fill).border(1.dp, fill, shape)
                .clickable(interactionSource = interaction, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
                .padding(contentPadding), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center, content = content)
        }
    }
}

@Composable private fun LauncherRow(service: String, icon: ImageVector, enabled: Boolean, normal: () -> Unit, bypass: () -> Unit, experimental: Boolean = false) {
    val support = if (experimental) t(" · 실험 지원") else ""
    Row(Modifier.height(48.dp)) {
        LauncherItem(service, tr("{0} 실행{1}", service, support), icon, enabled, Modifier.widthIn(min = 48.dp), normal)
        LauncherItem(null, tr("{0} Bypass 실행{1}", service, support), LauncherIcons.bolt, enabled, Modifier.width(48.dp), bypass)
    }
}

@Composable private fun LauncherItem(label: String?, description: String, icon: ImageVector, enabled: Boolean, modifier: Modifier, action: () -> Unit) {
    val color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (enabled) 1f else 0.38f)
    Row(modifier.fillMaxHeight().clickable(enabled = enabled, role = Role.Button, onClick = action)
        .semantics(mergeDescendants = true) { contentDescription = description }.padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally)) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp), tint = color)
        if (label != null) Text(label, color = color, style = MaterialTheme.typography.labelLarge, maxLines = 1)
    }
}
