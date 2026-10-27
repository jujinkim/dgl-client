package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*
import java.util.UUID
import kotlinx.coroutines.flow.first

@Composable expect fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier)
@Composable expect fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit)
@Composable expect fun PlatformInputRegistration(client: ClientConnection, preferences: Preferences, allowed: () -> Boolean)
@Composable expect fun PlatformConnectionSettings(model: AppModel)
@Composable expect fun PlatformInputSettings(preferences: Preferences, update: (Preferences) -> Unit)
expect fun copyPublicKey(value: String)
internal val LocalAppModel = staticCompositionLocalOf<AppModel?> { null }
// Height removed by IME only; system bars/cutouts already belong to the logical viewport.
internal val LocalTerminalOcclusion = staticCompositionLocalOf { 0.dp }
internal val LocalTerminalImeVisible = staticCompositionLocalOf { false }

@Composable internal fun MobileAppIcon() {
    Image(AppIcon, contentDescription = "DaGaLi", modifier = Modifier.size(24.dp)
        .clip(androidx.compose.foundation.shape.RoundedCornerShape(4.dp)))
}

@Composable fun App(model: AppModel, androidInput: Boolean = false) {
    val local by model.local.collectAsState()
    val connections by model.connections.collectAsState()
    val selected by model.selected.collectAsState()
    var lastExplorerClient by remember(model) { mutableStateOf<ClientConnection?>(null) }
    SideEffect { if (selected != null) lastExplorerClient = selected }
    val navigationClient = selected ?: lastExplorerClient?.takeIf { it in connections }
    val inputBlocks by model.inputBlocks.collectAsState()
    val appInputOwner = remember { Any() }
    LaunchedEffect(connections, selected) { if (androidInput) model.reconcileWorkspace() }
    SideEffect { model.workspaceEnabled = androidInput }
    val dark = when (local.preferences.theme) { "dark" -> true; "light" -> false; else -> isSystemInDarkTheme() }
    var edit by remember { mutableStateOf<HostProfile?>(null) }
    var adding by remember { mutableStateOf(false) }
    var settings by remember { mutableStateOf(false) }
    var agentDetails by remember { mutableStateOf<Pair<ClientConnection, String?>?>(null) }
    var notifications by remember { mutableStateOf(false) }
    val mobileTabState = rememberLazyListState()
    val captureFocus = rememberAppFocusCapture()
    var restoreNotificationFocus by remember { mutableStateOf<() -> Unit>({}) }
    var availableBounds by remember { mutableStateOf(Rect.Zero) }
    var tmuxNavigationClient by remember { mutableStateOf<ClientConnection?>(null) }
    val agentNav by model.agentNavigation.collectAsState()
    val agentError by model.agentNavigationError.collectAsState()
    val agentNotices by model.agentNotifications.collectAsState()
    var explorer by remember { mutableStateOf(false) }
    var deletion by remember { mutableStateOf<HostProfile?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    SideEffect { model.hostEditing = adding || edit != null || deletion != null }
    LaunchedEffect(agentNav) {
        if (agentNav > 0) {
            explorer = false; notifications = false; agentDetails = null; tmuxNavigationClient = null
            settings = false; adding = false; edit = null; deletion = null; error = null
        }
    }
    DisposableEffect(model) { onDispose { model.visibleConnection = null; model.visibleConnections = emptySet(); model.hostEditing = false; model.blockInput(appInputOwner, false) } }
    val appModal = notifications || agentError != null || agentDetails != null || settings || adding || edit != null || deletion != null || error != null
    SideEffect { model.blockInput(appInputOwner, appModal) }
    val colors = if (dark) darkColorScheme(
        primary = Color(0xff75aaff), secondary = Color(0xff75aaff),
        primaryContainer = Color(0xff2b4264), secondaryContainer = Color(0xff2b4264),
        background = Color(0xff1e1f22), surface = Color(0xff1e1f22), surfaceTint = Color.Transparent,
        onBackground = Color(0xffdadce0), onSurface = Color(0xffdadce0),
        surfaceVariant = Color(0xff25262a), onSurfaceVariant = Color(0xff9ca1aa), surfaceContainer = Color(0xff25262a), surfaceContainerLow = Color(0xff25262a), surfaceContainerHigh = Color(0xff25262a), surfaceContainerHighest = Color(0xff25262a), surfaceContainerLowest = Color(0xff1e1f22),
        outlineVariant = Color(0xff3b3d42),
    ) else lightColorScheme(
        primary = Color(0xff2468c9), secondary = Color(0xff2468c9),
        primaryContainer = Color(0xffdbe9ff), secondaryContainer = Color(0xffdbe9ff),
        background = Color(0xfff7f7f8), surface = Color(0xfff7f7f8), surfaceTint = Color.Transparent,
        onBackground = Color(0xff24262b), onSurface = Color(0xff24262b),
        surfaceVariant = Color(0xffeeeef0), onSurfaceVariant = Color(0xff626670), surfaceContainer = Color(0xffeeeef0), surfaceContainerLow = Color(0xffeeeef0), surfaceContainerHigh = Color(0xffeeeef0), surfaceContainerHighest = Color(0xffeeeef0), surfaceContainerLowest = Color.White,
        outlineVariant = Color(0xffd7d9df),
    )
    val systemAccent = PlatformAccentColor(dark)
    val raw = local.preferences.accent.resolve(dark, systemAccent)
    val bg = if (dark) 0x1e1f22 else 0xf7f7f8
    val selection = if (raw == (if (dark) 0x75aaff else 0x2468c9)) {
        if (dark) 0x2b4264 else 0xdbe9ff
    } else blendAccent(raw, bg, 0.22)
    fun rgb(value: Int) = Color(0xff000000 or value.toLong())
    val readable = readableAccent(raw, listOf(bg, if (dark) 0x25262a else 0xeeeef0, selection))
    // Text and focus use an adjusted color; filled controls retain the raw color.
    val roles = colors.copy(primary = rgb(readable), onPrimary = rgb(onAccentColor(readable)),
        secondary = rgb(readable), onSecondary = rgb(onAccentColor(readable)),
        tertiary = rgb(readable), onTertiary = rgb(onAccentColor(readable)),
        primaryContainer = rgb(selection), onPrimaryContainer = rgb(readable),
        secondaryContainer = rgb(selection), onSecondaryContainer = rgb(readable),
        tertiaryContainer = rgb(selection), onTertiaryContainer = rgb(readable))
    MaterialTheme(colorScheme = roles, shapes = Shapes(
        extraSmall = androidx.compose.foundation.shape.RoundedCornerShape(0.dp), small = androidx.compose.foundation.shape.RoundedCornerShape(0.dp),
        medium = androidx.compose.foundation.shape.RoundedCornerShape(0.dp), large = androidx.compose.foundation.shape.RoundedCornerShape(0.dp),
        extraLarge = androidx.compose.foundation.shape.RoundedCornerShape(0.dp))) {
        CompositionLocalProvider(LocalAppModel provides model, LocalAccentFill provides (rgb(raw) to rgb(onAccentColor(raw))),
            LocalTextSelectionColors provides TextSelectionColors(rgb(readable), rgb(selection))) {
        Surface(Modifier.fillMaxSize()) {
          val safeInsets = WindowInsets.systemBars.union(WindowInsets.displayCutout)
          val density = LocalDensity.current
          val imeOcclusion = if (androidInput) with(density) {
              (WindowInsets.ime.getBottom(this) - safeInsets.getBottom(this)).coerceAtLeast(0).toDp()
          } else 0.dp
          CompositionLocalProvider(LocalTerminalOcclusion provides imeOcclusion,
              LocalTerminalImeVisible provides (imeOcclusion > 0.dp)) {
          BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(safeInsets).padding(bottom = imeOcclusion)
              .onGloballyPositioned { availableBounds = it.boundsInWindow() }) {
            val workspaceMode = androidInput && maxWidth >= 840.dp
            val mobileLayout = androidInput && !workspaceMode
            LaunchedEffect(workspaceMode) { if (workspaceMode) { explorer = false; notifications = false } }
            if (!workspaceMode) {
                val visible = if (!explorer && inputBlocks.isEmpty()) setOfNotNull(selected) else emptySet()
                SideEffect { model.visibleConnections = visible }
            }
            Column(Modifier.fillMaxSize()) {
                if (mobileLayout) {
                    MobileHeader(selected?.host?.alias, explorer || selected == null, { explorer = !explorer },
                        { selected?.let { explorer = false; tmuxNavigationClient = it } }, menu = {
                            key(selected) {
                                val client = selected
                                val actions = buildList<Pair<String, () -> Unit>> {
                                    if (client != null) {
                                        add(t(if (local.preferences.mobilePaneToolbarVisible) "패널 도구 접기" else "패널 도구 펼치기") to {
                                            model.modify { it.copy(preferences = it.preferences.copy(mobilePaneToolbarVisible = !it.preferences.mobilePaneToolbarVisible)) }
                                        })
                                        addAll(hostConnectionActions(client, model))
                                    }
                                    add(t("설정") to { settings = true })
                                }
                                MobileHeaderMenu(client?.let { tr("현재 호스트 {0} 연결 메뉴", it.host.alias) } ?: t("기타 메뉴"), actions)
                            }
                        }, notification = {
                            MobileNotificationButton(agentNotices.count { !it.read }, {
                                restoreNotificationFocus = captureFocus()
                                model.blockInput(appInputOwner, true)
                                notifications = true
                            }) { anchor ->
                                if (notifications) MobileAgentNotifications(model, availableBounds, anchor, restoreNotificationFocus) { notifications = false }
                            }
                        })
                } else if (!workspaceMode) Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("DaGaLi", style = MaterialTheme.typography.titleLarge)
                    Spacer(Modifier.width(8.dp))
                    Text("SSH · tmux", style = MaterialTheme.typography.labelMedium, modifier = Modifier.weight(1f))
                    selected?.let { HostConnectionMenu(it, model) }
                    TextButton({ explorer = !explorer }, contentPadding = PaddingValues(horizontal = 4.dp), modifier = Modifier.widthIn(min = 48.dp)) { Text(if (explorer) t("터미널") else t("호스트")) }
                    TextButton({ notifications = true }, contentPadding = PaddingValues(horizontal = 4.dp), modifier = Modifier.widthIn(min = 48.dp)) { Text(tr("알림 {0}", agentNotices.count { !it.read })) }
                    TextButton({ settings = true }, contentPadding = PaddingValues(horizontal = 4.dp), modifier = Modifier.widthIn(min = 48.dp)) { Text(t("설정")) }
                }
                if (!androidInput && connections.isNotEmpty()) Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
                    connections.forEach { connection ->
                        FilterChip(selected == connection, { model.selectConnection(connection); explorer = false }, label = { Text(connection.host.alias) })
                        TextButton({ model.removeConnection(connection) }) { Text("×") }
                    }
                }
                val maintenanceError by model.backgroundConnectionError.collectAsState()
                maintenanceError?.let { Text(it, Modifier.fillMaxWidth().padding(horizontal = 12.dp), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                BoxWithConstraints(Modifier.weight(1f)) {
                    val wide = workspaceMode || (!androidInput && maxWidth >= 840.dp)
                    Row(Modifier.fillMaxSize()) {
                        if (wide || selected == null || explorer) {
                            Column(if (wide) Modifier.width(272.dp).fillMaxHeight() else Modifier.fillMaxSize()) {
                                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                                    if (workspaceMode) {
                                        MobileAppIcon()
                                        Spacer(Modifier.width(8.dp))
                                    }
                                    Text(t("연결"), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                                    TextButton({ model.hostEditing = true; adding = true }) { Text(t("+ 호스트")) }
                                }
                                LazyColumn(Modifier.weight(if (workspaceMode && navigationClient != null) .45f else 1f), contentPadding = PaddingValues(8.dp), verticalArrangement = Arrangement.spacedBy(0.dp)) {
                                    if (local.hosts.isEmpty()) item { Text(t("SSH 호스트를 추가하여 시작하세요.\n원격 서버에는 SSH와 tmux만 필요합니다."), Modifier.padding(12.dp)) }
                                    items(local.hosts.sortedWith(compareByDescending<HostProfile> { it.favorite }.thenByDescending { it.lastConnected }), key = { it.id }) { host ->
                                        Column(Modifier.fillMaxWidth()) {
                                            Column(Modifier.padding(12.dp)) {
                                                Text((if (host.favorite) "★ " else "") + host.alias, style = MaterialTheme.typography.titleMedium)
                                                Text("${host.username}@${host.hostname}:${host.port}", style = MaterialTheme.typography.bodySmall)
                                                if (host.lastConnected != 0L) Text(t("최근 연결: ") + java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT).format(java.util.Date(host.lastConnected)), style = MaterialTheme.typography.labelSmall)
                                                Row(verticalAlignment = Alignment.CenterVertically) {
                                                    TextButton({
                                                        try {
                                                            if (model.credentials.read(host.credentialRef) == null) {
                                                                model.hostEditing = true
                                                                edit = host
                                                                error = t("이 기기에서 사용할 비밀번호 또는 개인키를 등록하세요.")
                                                            } else { model.open(host); explorer = false }
                                                        } catch (_: Exception) { error = t("보호 저장소를 열지 못했습니다.") }
                                                    }) { Text(t("연결")) }
                                                    Spacer(Modifier.weight(1f))
                                                    TreeMenu(tr("{0} 메뉴", host.alias), listOf(t("편집") to { model.hostEditing = true; edit = host }, t("에이전트 상태 연동") to { connections.lastOrNull { it.host.id == host.id }?.let { agentDetails = it to null } ?: run { error = t("호스트에 먼저 연결하세요.") } }, t("삭제…") to { model.hostEditing = true; deletion = host }))
                                                }
                                            }
                                            HorizontalDivider()
                                        }
                                    }
                                }
                                if (workspaceMode && navigationClient != null) WorkspaceNavigation(navigationClient, local.preferences, model, Modifier.weight(.55f)) { client, pane -> agentDetails = client to pane }
                                if (!workspaceMode) Text(model.storageDescription, Modifier.padding(12.dp), style = MaterialTheme.typography.labelSmall)
                            }
                        }
                        if (wide || (selected != null && !explorer)) {
                            Box(Modifier.weight(1f).fillMaxHeight()) {
                                if (workspaceMode) WorkspaceContent(model, local.preferences, { client, pane -> agentDetails = client to pane })
                                else if (selected != null) key(selected) {
                                    val client = selected!!
                                    ConnectionScreen(client, local.preferences, mobileLayout, model, { agentDetails = client to it },
                                        mobileTabs = { MobileConnectionTabs(connections, selected, mobileTabState,
                                            { model.selectConnection(it); explorer = false }, model::removeConnection) },
                                        openNavigation = tmuxNavigationClient === client, onNavigationOpened = { tmuxNavigationClient = null }) {
                                        model.selected.value === client && (wide || !explorer) &&
                                            !notifications && agentError == null && agentDetails == null && !settings && !adding && edit == null && deletion == null && error == null
                                    }
                                }
                                else Column(Modifier.align(Alignment.Center).padding(24.dp)) {
                                    Text("Keep the CLI.", style = MaterialTheme.typography.headlineMedium)
                                    Text(t("세션은 서버에, 작업은 그대로."), style = MaterialTheme.typography.bodyLarge)
                                    Spacer(Modifier.height(12.dp))
                                    Text(t("호스트에 연결하고 기존 tmux 세션을 선택하세요."))
                                }
                            }
                        }
                    }
                }
                if (workspaceMode) Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    val status = selected?.status?.collectAsState()?.value
                    selected?.let { InfoBadge(displayTreeText(it.host.alias), Modifier.widthIn(max = 120.dp)) }
                    Spacer(Modifier.width(8.dp))
                    Text(listOfNotNull(status?.phase?.displayLabel(), selected?.let { client ->
                        val remote by client.remote.collectAsState(); val session by client.session.collectAsState(); val pane by client.pane.collectAsState()
                        remote.targetLabel(session, pane)
                    }, status?.message?.takeIf { status.phase in setOf(Phase.Failed, Phase.Reconnecting, Phase.MissingSession) }).filter { it.isNotEmpty() }.distinct().joinToString(" · "),
                        Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall)
                    selected?.let { HostConnectionMenu(it, model) }
                    TextButton({ notifications = true }) { Text(tr("알림 {0}", agentNotices.count { !it.read })) }
                    TextButton({ settings = true }) { Text(t("설정")) }
                }
            }
        if (selected == null || explorer || settings || notifications || agentDetails != null) SideEffect { model.visibleConnection = null }
        agentDetails?.let { (client, pane) -> AgentDetails(client, model, pane) { agentDetails = null } }
        if (notifications && !mobileLayout) AgentNotifications(model) { notifications = false }
        agentError?.let { message -> AlertDialog(onDismissRequest = { model.agentNavigationError.value = null }, text = { Text(message) }, confirmButton = { TextButton({ model.agentNavigationError.value = null }) { Text(t("닫기")) } }) }
        if (adding || edit != null) HostEditor(model, edit, mobileLayout, { adding = false; edit = null })
        if (settings) SettingsDialog(model, mobileLayout, androidInput) { settings = false }
        deletion?.let { host -> ConfirmDialog(t("호스트 삭제"), tr("{0}의 로컬 설정과 저장된 자격 증명을 삭제합니다.", host.alias), {
            try {
                model.credentials.delete(host.credentialRef)
                model.update(local.copy(hosts = local.hosts.filter { it.id != host.id }))
                deletion = null
            } catch (_: Exception) { error = t("자격 증명을 삭제하지 못했습니다.") }
        }, { deletion = null }) }
        error?.let { message -> AlertDialog(onDismissRequest = { error = null }, title = { Text(t("오류")) }, text = { Text(message) }, confirmButton = { TextButton({ error = null }) { Text(t("닫기")) } }) }
          }
          }
        }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun ConnectionScreen(client: ClientConnection, preferences: Preferences, mobileLayout: Boolean, model: AppModel, showAgents: (String) -> Unit, workspace: Boolean = false, sharedFocus: TerminalFocus? = null,
    mobileTabs: @Composable () -> Unit = {},
    openNavigation: Boolean = false, onNavigationOpened: () -> Unit = {}, inputAllowed: () -> Boolean) {
    val status by client.status.collectAsState()
    val remote by client.remote.collectAsState()
    val refreshing by client.refreshing.collectAsState()
    val expanded = remember(client) { mutableStateMapOf<String, Boolean>() }
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val trust by client.trustRequest.collectAsState()
    val notice by client.notice.collectAsState()
    val tmuxProblem by client.tmuxProblem.collectAsState()
    var tmuxDetails by remember(tmuxProblem) { mutableStateOf(false) }
    var navigation by remember { mutableStateOf(!mobileLayout && !workspace) }
    LaunchedEffect(openNavigation) { if (openNavigation) { navigation = true; onNavigationOpened() } }
    val navigationSheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    LaunchedEffect(navigation, mobileLayout) {
        if (mobileLayout && navigation) {
            snapshotFlow { navigationSheet.hasExpandedState }.first { it }
            navigationSheet.show()
        }
    }
    var namePrompt by remember { mutableStateOf<Pair<String, (String) -> Unit>?>(null) }
    var deletion by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val agentNav by model.agentNavigation.collectAsState()
    LaunchedEffect(agentNav) {
        if (agentNav > 0) {
            navigation = false; tmuxDetails = false; namePrompt = null; deletion = null
        }
    }
    val localFocus = remember(client) { TerminalFocus() }
    val terminalFocus = sharedFocus ?: localFocus
    val composer = remember(client, model) { model.composer(client) }
    val composerOpen by composer.opened.collectAsState()
    val modalOwner = remember { Any() }
    val composerOwner = remember { Any() }
    WorkspaceInputBlock(model, modalOwner, (mobileLayout && navigation) || tmuxDetails || namePrompt != null || deletion != null || trust != null || notice != null)
    WorkspaceInputBlock(model, composerOwner, composerOpen && !workspace)
    val blocks by model.inputBlocks.collectAsState()
    val terminalBaseAllowed = {
        // Observe Compose state for button/bridge updates; the flow also closes the pre-recomposition input race.
        inputAllowed() && status.acceptsInput && client.status.value.acceptsInput && !composerOpen && !composer.opened.value && !(mobileLayout && navigation) && !tmuxDetails && namePrompt == null && deletion == null &&
            client.trustRequest.value == null && client.notice.value == null
    }
    val terminalInputAllowed = { terminalBaseAllowed() && blocks.isEmpty() && !model.inputBlocked() }
    terminalFocus.inputAllowed = terminalInputAllowed
    if (!workspace) SideEffect { model.visibleConnection = client.takeIf { terminalInputAllowed() } }
    DisposableEffect(client) { onDispose { if (model.visibleConnection === client) model.visibleConnection = null } }
    if (!workspace) PlatformInputRegistration(client, preferences, terminalInputAllowed)
    val currentPane = remote.panes.firstOrNull { it.id == pane }
    val currentSession = remote.sessions.firstOrNull { it.id == session }
    val canBrowse = status.phase in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)
    val sessionLabel = when (status.phase) {
        Phase.Shell -> t("없음 · 일반 SSH")
        Phase.Disconnected, Phase.Failed -> t("연결 안 됨")
        Phase.Connecting, Phase.Authenticating, Phase.TrustRequired -> t("확인 중")
        Phase.Reconnecting -> t("복구 중")
        Phase.MissingSession -> t("이전 세션 없음")
        else -> currentSession?.let { displayTreeText(it.name) } ?: t("선택 안 함")
    }
    val statusDetail = if (status.phase in setOf(Phase.Failed, Phase.Reconnecting, Phase.MissingSession) || status.canStopRetrying) displayTreeText(status.message) else ""
    val statusLabel = listOf(displayTreeText(client.host.alias), (if (status.phase == Phase.Restoring && !status.showsConnectionProgress) t("연결됨") else status.phase.displayLabel()), remote.targetLabel(session, pane), statusDetail).filter { it.isNotEmpty() }.joinToString(" · ")
    val agentNotices by model.agentNotifications.collectAsState()
    val hierarchy = remote.hierarchyRows(preferences.showAllHierarchy)
    val navigationContent: @Composable (Modifier) -> Unit = { modifier ->
        Column(modifier.heightIn(max = if (mobileLayout) 480.dp else 280.dp).verticalScroll(rememberScrollState())) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(t("세션"), Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
                TextButton({ namePrompt = t("세션 만들기") to client::createSession }, enabled = canBrowse) { Text("+") }
                TooltipBox(positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(), tooltip = { PlainTooltip { Text(t("세션 새로고침")) } }, state = rememberTooltipState()) {
                    IconButton({ client.refresh() }, enabled = canBrowse && !refreshing, modifier = Modifier.semantics { contentDescription = t("세션 새로고침") }) { Text("↻") }
                }
            }
            if (status.phase == Phase.Shell) Text(t("일반 SSH · tmux 목록 확인 불가"), Modifier.padding(16.dp))
            else if (remote.sessions.isEmpty()) Text(if (canBrowse) t("세션 없음 · + 버튼으로 생성") else t("목록 대기 중"), Modifier.padding(16.dp))
            hierarchy.forEach { row ->
                key(row.key) { HierarchyEntry(row, 0, client, remote, session, pane, expanded, canBrowse, agentNotices, showAgents,
                    { title, action -> namePrompt = title to action }, { title, action -> deletion = title to action },
                    { s, w, p -> if (model.workspaceEnabled) model.openSession(client, s, w, p) else client.selectTarget(s, w, p) }) }
            }
        }
    }
    Column(Modifier.fillMaxSize()) {
        if (!workspace && mobileLayout) {
            if (status.phase != Phase.Ready && !(status.phase == Phase.Restoring && !status.showsConnectionProgress)) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(listOf(status.phase.displayLabel(), statusDetail).filter { it.isNotEmpty() }.distinct().joinToString(" · "),
                        Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall)
                    if (status.phase == Phase.Shell && tmuxProblem != null) TmuxInfoButton { tmuxDetails = true }
                }
            }
        } else if (!workspace) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
                    Text(statusLabel, style = MaterialTheme.typography.labelSmall, modifier = Modifier.weight(1f, fill = false),
                        maxLines = 2, overflow = TextOverflow.Ellipsis)
                    if (status.phase == Phase.Shell && tmuxProblem != null) TmuxInfoButton { tmuxDetails = true }
                }
            }
        }
        if (mobileLayout && preferences.mobilePaneToolbarVisible) TmuxPaneToolbar(client, { terminalFocus.request() }) { target ->
            deletion = tr("패널 {0}", remote.panes.firstOrNull { it.id == target }?.treeLabel()?.title ?: t("제목 미확인")) to { client.manage(TmuxCommand.killPane(target)) }
        }
        if (!status.retryExhausted && (status.canRetryNow || status.canStopRetrying)) {
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
                if (status.canRetryNow) TextButton({ client.retryNow() }) { Text(t("새로고침 재시도")) }
                if (status.canStopRetrying) TextButton({ client.stopRetrying() }) { Text(t("재시도 정지")) }
            }
        }
        Box(Modifier.fillMaxWidth().height(4.dp)) {
            if (status.showsConnectionProgress) LinearProgressIndicator(Modifier.fillMaxWidth())
        }
        if (!mobileLayout && !workspace) Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(tr("현재 세션: {0}", sessionLabel), modifier = Modifier.weight(1f), style = MaterialTheme.typography.labelMedium,
                maxLines = 2, overflow = TextOverflow.Ellipsis)
            TextButton({ navigation = !navigation }) { Text(if (navigation) t("tmux 접기") else t("tmux 관리")) }
        }
        if (navigation && !mobileLayout) navigationContent(Modifier)
        if (currentPane?.mode?.isNotBlank() == true) Text(tr("tmux {0} · 아래 터미널은 일반 패널 출력입니다. 로컬 스크롤·선택을 사용할 수 있습니다.", currentPane.mode), Modifier.padding(8.dp), style = MaterialTheme.typography.labelSmall)
        if (!mobileLayout && !workspace) TmuxPaneToolbar(client, { terminalFocus.request() }) { target ->
            deletion = tr("패널 {0}", remote.panes.firstOrNull { it.id == target }?.treeLabel()?.title ?: t("제목 미확인")) to { client.manage(TmuxCommand.killPane(target)) }
        }
        Box(Modifier.weight(1f).fillMaxWidth().background(Color(0xff11151b))) {
            CompositionLocalProvider(LocalTerminalFocus provides terminalFocus) {
                PlatformTerminal(client, preferences, Modifier.fillMaxSize())
            }
            if (status.retryExhausted) {
                Surface(Modifier.align(Alignment.Center).padding(12.dp), shape = MaterialTheme.shapes.medium,
                    color = MaterialTheme.colorScheme.surfaceContainer) {
                    Column(Modifier.padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(t("재접속 실패"), style = MaterialTheme.typography.titleMedium)
                        Spacer(Modifier.height(8.dp))
                        Button({ client.retryNow() }) { Text(t("다시 재접속 시도")) }
                    }
                }
            }
            TimedSnackbar(client.inputNotice, Modifier.align(Alignment.BottomCenter))
        }
        if (!workspace && mobileLayout) mobileTabs()
        if (!workspace) TerminalShortcutBar(client, preferences.terminalShortcuts, terminalBaseAllowed, terminalFocus, mobileLayout, composer::open, inputGate = model)
    }
    if (mobileLayout && composerOpen) {
        DisposableEffect(composer) { onDispose { model.blockInput(composerOwner, false); if (model.selected.value === client && inputAllowed()) terminalFocus.request() } }
        TextInputDialog(composer, terminalFocus, { model.selected.value === client && inputAllowed() }, status.acceptsInput)
    }
    if (mobileLayout && navigation) ModalBottomSheet(onDismissRequest = { navigation = false }, sheetState = navigationSheet) {
        // A weighted child needs a finite height before the sheet computes its anchors.
        // Keep the return action visible when the keyboard or landscape reduces the space.
        Column(Modifier.fillMaxWidth().heightIn(max = 600.dp)) {
            Text(t("tmux 세션 · 창"), Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleMedium)
            navigationContent(Modifier.weight(1f, fill = false))
            TextButton({ navigation = false }, modifier = Modifier.fillMaxWidth()) { Text(t("터미널로 돌아가기")) }
        }
    }
    trust?.let { request ->
        AlertDialog(onDismissRequest = { client.trustRequest.value = null; client.disconnect() }, title = { Text(t("처음 연결하는 서버")) },
            text = { Column { Text(request.endpoint); Spacer(Modifier.height(8.dp)); SelectionContainer { Text(request.fingerprint) }; Spacer(Modifier.height(8.dp)); Text(t("서버 관리자에게 받은 SHA-256 지문과 일치하는지 확인하세요.")) } },
            confirmButton = { TextButton({ client.trust() }) { Text(t("신뢰하고 연결")) } }, dismissButton = { TextButton({ client.trustRequest.value = null; client.disconnect() }) { Text(t("취소")) } })
    }
    namePrompt?.let { (title, action) -> NameDialog(title, { action(it); namePrompt = null }, { namePrompt = null }) }
    if (tmuxDetails && status.phase == Phase.Shell) tmuxProblem?.let {
        TmuxFailureDialog(client.host, it) { tmuxDetails = false }
    }
    deletion?.let { (target, action) -> ConfirmDialog(tr("{0} 삭제", target), t("이 대상의 원격 프로세스가 종료됩니다. 삭제할까요?"), { action(); deletion = null }, { deletion = null }) }
    notice?.let { message -> AlertDialog(onDismissRequest = { client.notice.value = null }, text = { Text(message) }, confirmButton = { TextButton({ client.notice.value = null }) { Text(t("닫기")) } }) }
}

@Composable private fun hostConnectionActions(client: ClientConnection, model: AppModel): List<Pair<String, () -> Unit>> {
    val status by client.status.collectAsState()
    val hostId = client.host.id
    return buildList {
        if (!status.canRetryNow) add(t("재접속") to { model.reconnectSelected(client, hostId) })
        add(t("연결 종료") to { model.disconnectSelected(client, hostId) })
    }
}

@Composable private fun HostConnectionMenu(client: ClientConnection, model: AppModel) {
    TreeMenu(tr("현재 호스트 {0} 연결 메뉴", client.host.alias), hostConnectionActions(client, model))
}

@Composable internal fun HierarchyEntry(row: HierarchyRow, depth: Int, client: ClientConnection, remote: RemoteState,
    session: String?, pane: String?, expanded: MutableMap<String, Boolean>, enabled: Boolean, notices: List<AgentNotice>,
    showAgents: (String) -> Unit, prompt: (String, (String) -> Unit) -> Unit, delete: (String, () -> Unit) -> Unit,
    selectTarget: (String, String?, String?) -> Unit = client::selectTarget) {
    val actions = buildList<Pair<String, () -> Unit>> {
        if (row.kind == TreeKind.Session) {
            add(t("창 만들기") to { prompt(t("창 만들기")) { client.manage(TmuxCommand.newWindow(row.session, it)) } })
            add(t("세션 이름 변경") to { prompt(t("세션 이름 변경")) { client.manage(TmuxCommand.renameSession(row.session, it)) } })
            add(t("세션 삭제…") to { delete(tr("세션 {0}", row.label.title)) { client.manage(TmuxCommand.killSession(row.session)) } })
        }
        if (row.kind != TreeKind.Pane) row.window?.let { w ->
            add(t("창 이름 변경") to { prompt(t("창 이름 변경")) { client.manage(TmuxCommand.renameWindow(w, it)) } })
            add(t("창 삭제…") to { delete(t("창")) { client.manage(TmuxCommand.killWindow(w)) } })
        }
        remote.panes.firstOrNull { it.window == row.window && it.active }?.id?.let { p ->
            listOf(true to t("패널 좌우 나누기"), false to t("패널 상하 나누기")).forEach { (horizontal, label) ->
                add(label to { client.selectTarget(row.session, row.window, p); client.manage(TmuxCommand.split(p, horizontal)) })
            }
            add(t("패널 제목 변경") to { prompt(t("패널 제목 변경")) { client.manage(TmuxCommand.renamePane(p, it)) } })
            add(t("패널 삭제…") to { delete(tr("패널 {0}", row.label.title)) { client.manage(TmuxCommand.killPane(p)) } })
        }
    }
    TreeRow(row.label, depth, row.selected(remote, session, pane), if (row.children.isEmpty()) null else expanded[row.key] == true,
        { expanded[row.key] = expanded[row.key] != true }, { selectTarget(row.session, row.window, row.pane) }, enabled, actions,
        status = { row.window?.let { p -> AgentBadge(client, p, notices, compact = true) { showAgents(p) } } })
    if (expanded[row.key] == true) row.children.forEach { child ->
        key(child.key) { HierarchyEntry(child, depth + 1, client, remote, session, pane, expanded, enabled, notices, showAgents, prompt, delete, selectTarget) }
    }
}

@Composable internal fun TreeMenu(label: String, actions: List<Pair<String, () -> Unit>>, enabled: Boolean = true) {
    var menu by remember { mutableStateOf(false) }
    val model = LocalAppModel.current
    val owner = remember { Any() }
    if (model != null) WorkspaceInputBlock(model, owner, menu)
    Box(Modifier.size(48.dp)) {
        IconButton({ menu = true }, enabled = enabled, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) { Text("…") }
        DropdownMenu(menu, { menu = false; model?.blockInput(owner, false) }) {
            actions.forEach { (text, action) -> DropdownMenuItem(text = { Text(text) }, onClick = { menu = false; model?.blockInput(owner, false); action() }) }
        }
    }
}
@OptIn(ExperimentalMaterial3Api::class)
@Composable private fun TreeRow(label: TreeLabel, depth: Int, active: Boolean, expanded: Boolean?, toggle: () -> Unit,
    select: () -> Unit, enabled: Boolean, actions: List<Pair<String, () -> Unit>>, status: @Composable () -> Unit = {}) {
    val guideColor = MaterialTheme.colorScheme.outlineVariant
    val background = when {
        active -> MaterialTheme.colorScheme.secondaryContainer
        depth == 0 -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.06f)
            .compositeOver(MaterialTheme.colorScheme.surfaceContainer)
        else -> Color.Transparent
    }
    Row(Modifier.fillMaxWidth().background(background).drawBehind {
        repeat(depth) { level ->
            val x = (level * 20 + 4).dp.toPx()
            drawLine(guideColor, androidx.compose.ui.geometry.Offset(x, 0f), androidx.compose.ui.geometry.Offset(x, size.height), 1.dp.toPx())
        }
    }.padding(start = (depth * 20).dp).semantics { selected = active }, verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f)) {
            TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text(label.fullText) } }, state = rememberTooltipState(), modifier = Modifier.fillMaxWidth()) {
                TextButton(select, enabled = enabled, shape = RectangleShape, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)
                    .semantics { contentDescription = label.fullText }) {
                    Column(Modifier.fillMaxWidth().clearAndSetSemantics {}) {
                        Text(label.title, maxLines = 1, overflow = TextOverflow.Ellipsis,
                            fontSize = if (depth < 2) 16.sp else 14.sp,
                            lineHeight = if (depth < 2) 22.sp else 19.sp,
                            fontWeight = if (depth < 2) FontWeight.Bold else FontWeight.Medium)
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            if (label.subtitle.isNotEmpty()) Text(label.subtitle, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis,
                                fontSize = 12.sp, lineHeight = 17.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            InfoBadge(label.detail, Modifier.widthIn(max = 124.dp).then(if (label.subtitle.isEmpty()) Modifier else Modifier.weight(1f, fill = false)))
                        }
                    }
                }
            }
        }
        if (expanded != null) IconButton(toggle, modifier = Modifier.size(48.dp).semantics { contentDescription = tr("{0} {1}", label.fullText, if (expanded) t("접기") else t("펼치기")) }) { Text(if (expanded) "▾" else "▸") }
        TreeMenu(tr("{0} 메뉴", label.fullText), actions, enabled)
        status()
    }
}

@Composable private fun HostEditor(model: AppModel, original: HostProfile?, mobileLayout: Boolean, dismiss: () -> Unit) {
    var alias by remember { mutableStateOf(original?.alias ?: "") }
    var hostname by remember { mutableStateOf(original?.hostname ?: "") }
    var port by remember { mutableStateOf((original?.port ?: 22).toString()) }
    var username by remember { mutableStateOf(original?.username ?: "") }
    var auth by remember { mutableStateOf(original?.auth ?: AuthKind.Password) }
    var password by remember { mutableStateOf("") }
    var privateKey by remember { mutableStateOf("") }
    var passphrase by remember { mutableStateOf("") }
    var showPassword by remember(auth) { mutableStateOf(false) }
    var favorite by remember { mutableStateOf(original?.favorite ?: false) }
    var generatedPublic by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    EditorDialog(mobileLayout, onDismissRequest = dismiss, title = { Text(if (original == null) t("SSH 호스트 추가") else t("호스트 편집")) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(0.dp)) {
            OutlinedTextField(alias, { alias = it }, label = { Text(t("별칭")) }, singleLine = true)
            OutlinedTextField(hostname, { hostname = it }, label = { Text(t("호스트 / IP")) }, singleLine = true)
            Row {
                OutlinedTextField(port, { port = it }, label = { Text(t("포트")) }, singleLine = true, modifier = Modifier.width(90.dp))
                Spacer(Modifier.width(8.dp))
                OutlinedTextField(username, { username = it }, label = { Text(t("사용자 이름")) }, singleLine = true)
            }
            Row(verticalAlignment = Alignment.CenterVertically) { AccentCheckbox(favorite, { favorite = it }); Text(t("즐겨찾기")) }
            Row { FilterChip(auth == AuthKind.Password, { auth = AuthKind.Password }, label = { Text(t("비밀번호")) }); Spacer(Modifier.width(8.dp)); FilterChip(auth == AuthKind.PrivateKey, { auth = AuthKind.PrivateKey }, label = { Text(t("개인키")) }) }
            val passwordTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation()
            val showPasswordControl: @Composable () -> Unit = {
                Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(showPassword, role = Role.Checkbox, onValueChange = { showPassword = it }), verticalAlignment = Alignment.CenterVertically) {
                    AccentCheckbox(showPassword, onCheckedChange = null, modifier = Modifier.padding(12.dp))
                    Text(t("비밀번호 표시"))
                }
            }
            if (auth == AuthKind.Password) {
                OutlinedTextField(password, { password = it }, label = { Text(if (original == null) t("비밀번호") else t("새 비밀번호 (비우면 기존 유지)")) }, visualTransformation = passwordTransformation, singleLine = true)
                showPasswordControl()
            }
            else {
                ImportKeyButton({ privateKey = it; generatedPublic = null }, { error = it })
                Text(if (privateKey.isEmpty()) t("Ed25519 / RSA · 암호화 키 지원") else t("개인키를 가져왔습니다."), style = MaterialTheme.typography.labelSmall)
                OutlinedTextField(passphrase, { passphrase = it }, label = { Text(t("키 암호")) }, visualTransformation = passwordTransformation, singleLine = true)
                showPasswordControl()
                TextButton({ try { val generated = model.keys.ed25519(); privateKey = generated.credential.privateKey; passphrase = ""; generatedPublic = generated.publicKey } catch (_: Exception) { error = t("키 생성 실패") } }) { Text(t("Ed25519 키 생성")) }
                if (privateKey.isNotEmpty() || original?.auth == AuthKind.PrivateKey) {
                    Text(t("공개키를 서버의 authorized_keys에 등록하세요."))
                    TextButton({
                        try {
                            val credential = if (privateKey.isNotEmpty()) Credential(privateKey = privateKey, passphrase = passphrase)
                                else requireNotNull(model.credentials.read(original!!.credentialRef)).let { if (passphrase.isNotEmpty()) Credential(it.password, it.privateKey, passphrase) else it }
                            copyPublicKey(generatedPublic ?: model.keys.publicKey(credential))
                        } catch (_: Exception) { error = t("공개키를 읽지 못했습니다. 개인키와 키 암호 또는 보호 저장소를 확인하세요.") }
                    }) { Text(t("공개키 복사")) }
                }
            }
            Text(model.storageDescription, style = MaterialTheme.typography.labelSmall)
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }, confirmButton = {
        TextButton({
            try {
                val id = original?.id ?: UUID.randomUUID().toString()
                val profile = HostProfile(id, alias.trim(), hostname.trim(), port.toInt(), username.trim(), auth, original?.credentialRef ?: id, favorite, original?.lastConnected ?: 0, original?.lastSession, original?.lastPane)
                val changed = (auth == AuthKind.Password && password.isNotEmpty()) || (auth == AuthKind.PrivateKey && (privateKey.isNotEmpty() || passphrase.isNotEmpty()))
                if (original == null || original.auth != auth) require(changed)
                if (changed) {
                    val previous = if (original == null) Credential() else requireNotNull(model.credentials.read(profile.credentialRef))
                    val credential = if (auth == AuthKind.Password) Credential(password, previous.privateKey, previous.passphrase)
                        else Credential(previous.password, privateKey.ifEmpty { previous.privateKey }, passphrase)
                    if (auth == AuthKind.PrivateKey) model.keys.publicKey(credential)
                    model.credentials.write(profile.credentialRef, credential)
                }
                model.update(model.local.value.copy(hosts = model.local.value.hosts.filter { it.id != id } + profile))
                dismiss()
            } catch (_: IllegalArgumentException) { error = t("필수 입력과 포트(1–65535), 자격 증명을 확인하세요.") }
            catch (_: Exception) { error = t("보호 저장소에 저장하지 못했습니다. 저장을 중단했습니다.") }
        }) { Text(t("저장")) }
    }, dismissButton = { TextButton(dismiss) { Text(t("취소")) } })
}

@Composable private fun SettingsDialog(model: AppModel, mobileLayout: Boolean, androidInput: Boolean, dismiss: () -> Unit) {
    val local by model.local.collectAsState()
    var forget by remember { mutableStateOf<String?>(null) }
    var details by remember { mutableStateOf(false) }
    var restoreFocus by remember { mutableStateOf(false) }
    val mainScroll = rememberScrollState()
    val shortcutFocus = remember { androidx.compose.ui.focus.FocusRequester() }
    val back = { if (details) { details = false; restoreFocus = true } else dismiss() }
    LaunchedEffect(details) {
        if (!details && restoreFocus) { withFrameNanos { }; shortcutFocus.requestFocus(); restoreFocus = false }
    }
    EditorDialog(mobileLayout, settingsLayout = true, onDismissRequest = back, title = {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (details) TextButton(back) { Text(t("뒤로")) }
            Text(t(if (details) "하단 단축키" else "설정"))
        }
    }, text = {
        if (details) TerminalShortcutSettings(local.preferences.terminalShortcuts) { shortcuts ->
            model.modify { it.copy(preferences = it.preferences.copy(terminalShortcuts = shortcuts)) }
        } else Column(Modifier.verticalScroll(mainScroll), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            SettingsCard(t("화면·언어"), t("앱의 언어와 모양을 선택합니다.")) {
                Text(t("언어"), style = MaterialTheme.typography.titleSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("system" to t("시스템"), "ko" to "한국어", "en" to "English").forEach { (value, label) ->
                        val choice = local.preferences.language.takeIf { it == "ko" || it == "en" } ?: "system"
                        FilterChip(choice == value, { model.modify { it.copy(preferences = it.preferences.copy(language = value)) } }, label = { Text(label) })
                    }
                }
                Text(t("테마"), style = MaterialTheme.typography.titleSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("system" to t("시스템"), "light" to t("밝게"), "dark" to t("어둡게")).forEach { (value, label) ->
                        FilterChip(local.preferences.theme == value, { model.modify { it.copy(preferences = it.preferences.copy(theme = value)) } }, label = { Text(label) })
                    }
                }
                AccentSettingsUi(local.preferences.accent) { accent -> model.modify { it.copy(preferences = it.preferences.copy(accent = accent)) } }
            }
            SettingsCard(t("터미널·입력"), t("터미널 글자와 입력 도구를 조정합니다.")) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(tr("터미널 글자 크기: {0}", local.preferences.fontSize), Modifier.weight(1f))
                    TextButton({ model.modify { it.copy(preferences = it.preferences.copy(fontSize = (it.preferences.fontSize - 1).coerceAtLeast(10))) } }) { Text(t("작게")) }
                    TextButton({ model.modify { it.copy(preferences = it.preferences.copy(fontSize = (it.preferences.fontSize + 1).coerceAtMost(28))) } }) { Text(t("크게")) }
                }
                Text(tr("스크롤백: {0}줄 · 터미널에서 스크롤·선택·복사", local.preferences.scrollback), style = MaterialTheme.typography.bodySmall)
                TextButton({ details = true }, modifier = Modifier.fillMaxWidth().then(Modifier.focusRequester(shortcutFocus))) {
                    Text(t("하단 단축키"), Modifier.weight(1f)); Text(tr("{0}개 · 순서 및 표시 편집", local.preferences.terminalShortcuts.size)); Text(" ›")
                }
                if (androidInput) PlatformInputSettings(local.preferences) { preferences -> model.update(model.local.value.copy(preferences = preferences)) }
            }
            SettingsCard(t("작업 공간"), t("계층 표시와 화면 전환을 선택합니다.")) {
                SettingsSwitch(t("단일 창도 표시"), local.preferences.showAllHierarchy) { enabled -> model.modify { it.copy(preferences = it.preferences.copy(showAllHierarchy = enabled)) } }
                if (androidInput) {
                    SettingsSwitch(t("부드러운 터미널 화면 전환"), local.preferences.smoothTerminalTransitions) { enabled -> model.modify { it.copy(preferences = it.preferences.copy(smoothTerminalTransitions = enabled)) } }
                    Text(t("재접속 시 크기·페이드·블러 효과 적용"), style = MaterialTheme.typography.bodySmall)
                }
            }
            if (androidInput) SettingsCard(t("연결"), t("백그라운드 연결과 배터리 사용을 관리합니다.")) { PlatformConnectionSettings(model) }
            SettingsCard(t("알림"), t("에이전트 알림과 OS 알림을 선택합니다.")) { AgentNotificationSettingsUi(model) }
            SettingsCard(t("동기화"), t("기기 간 공통 설정과 호스트를 공유합니다.")) { CloudSyncSettings(model) }
            SettingsCard(t("보안·저장"), t("신뢰한 서버와 이 기기의 저장 위치를 확인합니다.")) {
                Text(t("신뢰한 서버"), style = MaterialTheme.typography.titleSmall)
                local.trust.forEach { (endpoint, fingerprint) ->
                    Column { Text(endpoint); SelectionContainer { Text(fingerprint, style = MaterialTheme.typography.labelSmall) }; TextButton({ forget = endpoint }) { Text(t("신뢰 키 삭제")) } }
                }
                Text(model.storageDescription, style = MaterialTheme.typography.bodySmall)
            }
        }
    }, confirmButton = { TextButton(back) { Text(t(if (details) "뒤로" else "닫기")) } })
    forget?.let { endpoint -> ConfirmDialog(t("신뢰 키 삭제"), tr("{0} 키를 삭제합니다. 다음 연결에서 지문을 다시 확인해야 합니다.", endpoint), { model.modify { it.copy(trust = it.trust - endpoint) }; forget = null }, { forget = null }) }
}

@Composable private fun SettingsSwitch(label: String, selected: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(selected, role = Role.Switch, onValueChange = change), verticalAlignment = Alignment.CenterVertically) {
        Text(label, Modifier.weight(1f)); AccentSwitch(selected, null)
    }
}

@Composable internal fun NameDialog(title: String, accept: (String) -> Unit, dismiss: () -> Unit) {
    var value by remember { mutableStateOf("") }
    AlertDialog(onDismissRequest = dismiss, title = { Text(title) }, text = { OutlinedTextField(value, { value = it.take(128) }, label = { Text(t("이름")) }, singleLine = true) },
        confirmButton = { TextButton({ accept(value) }, enabled = value.isNotBlank() && value.none { it == '.' || it == ':' || it.code < 32 }) { Text(t("확인")) } }, dismissButton = { TextButton(dismiss) { Text(t("취소")) } })
}
@Composable internal fun ConfirmDialog(title: String, message: String, accept: () -> Unit, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(title) }, text = { Text(message) }, confirmButton = { TextButton(accept) { Text(t("삭제")) } }, dismissButton = { TextButton(dismiss) { Text(t("취소")) } })
}

@Composable private fun EditorDialog(
    fullscreen: Boolean,
    settingsLayout: Boolean = false,
    onDismissRequest: () -> Unit,
    title: @Composable () -> Unit,
    text: @Composable () -> Unit,
    confirmButton: @Composable () -> Unit,
    dismissButton: @Composable () -> Unit = {},
) {
    if (!fullscreen && !settingsLayout) AlertDialog(onDismissRequest = onDismissRequest, title = title, text = text, confirmButton = confirmButton, dismissButton = dismissButton)
    else if (!fullscreen) Dialog(onDismissRequest, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(24.dp), contentAlignment = Alignment.Center) {
            Surface(Modifier.widthIn(max = 720.dp).fillMaxWidth().heightIn(max = maxHeight), shape = MaterialTheme.shapes.large) {
                Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    ProvideTextStyle(MaterialTheme.typography.titleLarge) { title() }
                    Box(Modifier.weight(1f, fill = false).fillMaxWidth()) { text() }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { dismissButton(); confirmButton() }
                }
            }
        }
    }
    else Dialog(onDismissRequest, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                ProvideTextStyle(MaterialTheme.typography.titleLarge) { title() }
                Box(Modifier.weight(1f).fillMaxWidth()) { text() }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { dismissButton(); confirmButton() }
            }
        }
    }
}
