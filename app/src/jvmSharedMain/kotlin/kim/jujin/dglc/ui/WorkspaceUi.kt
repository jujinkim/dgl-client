package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.*
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.key.*
import androidx.compose.ui.input.pointer.*
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.*
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*

@Composable internal fun workspaceTabTitle(client: ClientConnection, sessionOnly: Boolean = false): String {
    val session by client.session.collectAsState()
    val remote by client.remote.collectAsState()
    val localCache = remember(client) { SessionTitleCache() }
    return (LocalAppModel.current?.connectionTitles(client) ?: localCache).title(remote, session, sessionOnly)
}

@Composable internal fun InfoBadge(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, androidx.compose.foundation.shape.RoundedCornerShape(3.dp))
        .padding(horizontal = 4.dp, vertical = 1.dp),
        color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 10.sp, lineHeight = 12.sp,
        maxLines = 1, overflow = TextOverflow.Ellipsis)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun ConnectionTitle(client: ClientConnection, sessionOnly: Boolean = false, draggable: Boolean = false) {
    val title = workspaceTabTitle(client, sessionOnly)
    val host = displayTreeText(client.host.alias)
    val full = "$host · $title"
    TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text(full) } }, state = rememberTooltipState(), enableUserInput = !draggable) {
        Column(Modifier.clearAndSetSemantics { contentDescription = full },
            horizontalAlignment = Alignment.Start) {
            Text(compactTabTitle(title), maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelLarge)
            InfoBadge(host, Modifier.widthIn(max = 120.dp))
        }
    }
}

@Composable internal fun WorkspaceInputBlock(model: AppModel, owner: Any, blocked: Boolean) {
    SideEffect { model.blockInput(owner, blocked) }
    DisposableEffect(model, owner) { onDispose { model.blockInput(owner, false) } }
}

@Composable internal fun WorkspaceNavigation(client: ClientConnection, preferences: Preferences, model: AppModel,
    modifier: Modifier, showAgents: (ClientConnection, String) -> Unit) {
    val status by client.status.collectAsState()
    val remote by client.remote.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val refreshing by client.refreshing.collectAsState()
    val notices by model.agentNotifications.collectAsState()
    val expanded = remember(client) { mutableStateMapOf<String, Boolean>() }
    var prompt by remember(client) { mutableStateOf<Pair<String, (String) -> Unit>?>(null) }
    var deletion by remember(client) { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val owner = remember { Any() }
    WorkspaceInputBlock(model, owner, prompt != null || deletion != null)
    val enabled = status.phase in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)
    val generation = client.generation
    Column(modifier) {
        HorizontalDivider()
        Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(t("세션"), Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
            IconButton({ prompt = t("세션 만들기") to client::createSession }, enabled = enabled,
                modifier = Modifier.semantics { contentDescription = t("세션 만들기") }) { Text("+") }
            IconButton(client::refresh, enabled = enabled && !refreshing,
                modifier = Modifier.semantics { contentDescription = t("세션 새로고침") }) { Text("↻") }
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            if (remote.sessions.isEmpty()) Text(t("세션 없음 · + 버튼으로 생성"), Modifier.padding(12.dp))
            remote.hierarchyRows(preferences.showAllHierarchy).forEach { row -> key(row.key) {
                HierarchyEntry(row, 0, client, remote, session, pane, expanded, enabled, notices,
                    { showAgents(client, it) }, { title, action -> prompt = title to action }, { title, action -> deletion = title to action },
                    { s, w, p -> model.openSession(client, s, w, p, generation) })
            } }
        }
    }
    prompt?.let { (title, action) -> NameDialog(title, { if (client.generation == generation) action(it); prompt = null }, { prompt = null }) }
    deletion?.let { (title, action) -> ConfirmDialog(title, t("이 대상의 원격 프로세스가 종료됩니다. 삭제할까요?"),
        { if (client.generation == generation) action(); deletion = null }, { deletion = null }) }
}

private data class TabDrag(val tab: String, val position: Offset)

/** Flat keyed leaves keep composition identity independent of the shape of the split tree. */
@Composable internal fun WorkspaceContent(model: AppModel, preferences: Preferences, showAgents: (ClientConnection, String) -> Unit) {
    val workspace by model.workspace.collectAsState()
    val activeClient by model.selected.collectAsState()
    val blocks by model.inputBlocks.collectAsState()
    val focuses = remember(model) { java.util.IdentityHashMap<ClientConnection, TerminalFocus>() }
    val clients = model.connections.collectAsState().value
    SideEffect { focuses.keys.retainAll(clients.toSet()) }
    fun focus(client: ClientConnection) = focuses.getOrPut(client) { TerminalFocus() }
    LaunchedEffect(activeClient) { withFrameNanos { }; activeClient?.let { focus(it).request() } }
    var drag by remember { mutableStateOf<TabDrag?>(null) }
    var preview by remember { mutableStateOf<WorkspaceState?>(null) }
    val tabBounds = remember { mutableMapOf<String, Rect>() }
    SideEffect { tabBounds.keys.retainAll(workspace.groups.values.flatMap { it.tabs }.toSet()) }
    val localDensity = LocalDensity.current
    val density = localDensity.density
    Column(Modifier.fillMaxSize().onPreviewKeyEvent {
        if (it.type == KeyEventType.KeyDown && it.key == Key.Escape && (drag != null || preview != null)) {
            drag = null; preview = null; true
        } else false
    }) {
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth().semantics { contentDescription = t("분할 작업 공간") }) {
            val width = maxWidth.value; val height = maxHeight.value
            val logicalHeight = height + LocalTerminalOcclusion.current.value
            var origin by remember { mutableStateOf(Offset.Zero) }
            val layout = preview?.takeIf { it.groups == workspace.groups } ?: workspace
            val projection = layout.project(width, logicalHeight, height)
            val fits = projection.fits
            val rects = projection.visible
            val shown = rects.keys.mapNotNull { model.tabClient(workspace.groups[it]?.selected) }.toSet()
            val visibleClients = if (blocks.isEmpty()) shown else emptySet()
            SideEffect { model.visibleConnections = visibleClients }
            DisposableEffect(model) { onDispose { model.visibleConnections = emptySet() } }
            val candidate = drag?.let { moving ->
                val point = (moving.position - origin) / density
                rects.entries.firstOrNull { it.value.contains(point.x, point.y) }?.let { (id, rect) ->
                    val local = point - Offset(rect.x, rect.y)
                    val edge = when {
                        local.y < 48f -> DropEdge.Center
                        local.x < minOf(56f, rect.width / 4) -> DropEdge.Left
                        local.x > rect.width - minOf(56f, rect.width / 4) -> DropEdge.Right
                        local.y < minOf(80f, rect.height / 4) -> DropEdge.Top
                        local.y > rect.height - minOf(56f, rect.height / 4) -> DropEdge.Bottom
                        else -> DropEdge.Center
                    }
                    val index = workspace.groups.getValue(id).tabs.filter { it != moving.tab }.indexOfFirst {
                        tabBounds[it]?.let { bounds -> moving.position.x < bounds.center.x } == true
                    }.takeIf { it >= 0 } ?: Int.MAX_VALUE
                    WorkspaceDrop(id, edge, index).takeIf { workspace.canDrop(moving.tab, it, width, logicalHeight) && (fits || it.edge == DropEdge.Center) }
                }
            }
            val currentCandidate by rememberUpdatedState(candidate)
            val currentDrag by rememberUpdatedState(drag)
            Box(Modifier.fillMaxSize().onGloballyPositioned { origin = it.boundsInRoot().topLeft }) {
                rects.forEach { (groupId, rect) -> key(groupId) {
                    val group = workspace.groups.getValue(groupId)
                    val active = workspace.activeGroup == groupId
                    Column(Modifier.offset { IntOffset((rect.x * density).toInt(), (rect.y * density).toInt()) }
                        .size(rect.width.dp, rect.height.dp)
                        .border(if (active) 2.dp else 1.dp, if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant)
                        .semantics { contentDescription = tr("터미널 그룹 {0}", workspace.root.leaves().indexOfFirst { it.id == groupId } + 1); selected = active }) {
                        Row(Modifier.fillMaxWidth().height(48.dp).background(MaterialTheme.colorScheme.surfaceContainer), verticalAlignment = Alignment.CenterVertically) {
                            Row(Modifier.weight(1f).horizontalScroll(rememberScrollState())) {
                                group.tabs.forEach { tab -> key(tab) { model.tabClient(tab)?.let { client ->
                                    val title = displayTreeText(client.host.alias) + " · " + workspaceTabTitle(client)
                                    var bounds by remember { mutableStateOf(Rect.Zero) }
                                    Row(Modifier.onGloballyPositioned { bounds = it.boundsInRoot(); tabBounds[tab] = bounds }
                                        .background(if (group.selected == tab) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainer)
                                        .workspaceTabDrag(tab, { bounds.topLeft }, { drag = TabDrag(tab, it) }, { drag = drag?.copy(position = it) }, {
                                            val moving = currentDrag; val target = currentCandidate
                                            if (moving != null && target != null) model.changeWorkspace { it.drop(moving.tab, target, width, logicalHeight) }
                                            drag = null
                                        }, { drag = null }), verticalAlignment = Alignment.CenterVertically) {
                                        TextButton({ model.selectConnection(client) }, contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp), modifier = Modifier.widthIn(max = 220.dp).height(48.dp)
                                            .semantics { contentDescription = title; selected = group.selected == tab }) {
                                            ConnectionTitle(client, draggable = true)
                                        }
                                        TreeMenu(tr("{0} 탭 메뉴", title), buildList {
                                            add(t("탭 닫기") to { model.removeConnection(client) })
                                            val index = group.tabs.indexOf(tab)
                                            if (index > 0) add(t("탭 앞으로") to { model.changeWorkspace { it.move(tab, groupId, index - 1) } })
                                            if (index < group.tabs.lastIndex) add(t("탭 뒤로") to { model.changeWorkspace { it.move(tab, groupId, index + 1) } })
                                            workspace.root.leaves().filter { it.id != groupId }.forEach { leaf ->
                                                add(tr("그룹 {0}으로 이동", workspace.root.leaves().indexOf(leaf) + 1) to { model.changeWorkspace { it.move(tab, leaf.id) } })
                                            }
                                        })
                                    }
                                } } }
                            }
                            listOf(SplitAxis.Horizontal to t("앱 화면 좌우 분할"), SplitAxis.Vertical to t("앱 화면 상하 분할")).forEach { (axis, label) ->
                                IconButton({ model.changeWorkspace { it.split(groupId, axis, width, logicalHeight) } },
                                    enabled = workspace.split(groupId, axis, width, logicalHeight) != workspace,
                                    modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
                                    Text(if (axis == SplitAxis.Horizontal) "↔" else "↕")
                                }
                            }
                            TreeMenu(t("앱 화면 분할 메뉴"), buildList {
                                if (workspace.groups.size > 1) add(t("인접 그룹과 합치기") to { model.changeWorkspace { it.merge(groupId) } })
                            }, enabled = workspace.groups.size > 1)
                        }
                        Box(Modifier.weight(1f).fillMaxWidth()) {
                            val client = model.tabClient(group.selected)
                            if (client != null) key(client) {
                                val terminalFocus = focus(client)
                                terminalFocus.activate = { if (!model.inputBlocked()) model.selectConnection(client) }
                                CompositionLocalProvider(LocalTerminalOcclusion provides
                                    with(localDensity) {
                                        (projection.logical.getValue(groupId).height.dp.roundToPx() - rect.height.dp.roundToPx()).coerceAtLeast(0).toDp()
                                    }) {
                                    ConnectionScreen(client, preferences, false, model, { showAgents(client, it) }, workspace = true, sharedFocus = terminalFocus) {
                                        model.selected.value === client
                                    }
                                }
                            } else Box(Modifier.fillMaxSize().clickable { model.changeWorkspace { it.activate(groupId) } }, contentAlignment = Alignment.Center) {
                                Text(t("호스트나 세션을 선택하여 여기에 연결하세요."), Modifier.padding(16.dp))
                            }
                        }
                    }
                } }
                if (fits) layout.root.nodeRectangles(width, height).forEach { (id, rect) ->
                    val split = findWorkspaceSplit(layout.root, id) ?: return@forEach
                    val a = layout.root.nodeRectangles(width, height).getValue(split.first.id)
                    val horizontal = split.axis == SplitAxis.Horizontal
                    val x = if (horizontal) a.x + a.width + WorkspaceState.DIVIDER / 2 - 24 else rect.x
                    val y = if (horizontal) rect.y else a.y + a.height + WorkspaceState.DIVIDER / 2 - 24
                    var original: WorkspaceState? = null
                    var delta = 0f
                    Box(Modifier.offset { IntOffset((x * density).toInt(), (y * density).toInt()) }
                        .size(if (horizontal) 48.dp else rect.width.dp, if (horizontal) rect.height.dp else 48.dp)
                        .semantics { contentDescription = t("분할 경계 조절") }
                        .pointerInput(id, width, height, workspace.root) {
                            detectDragGestures(onDragStart = { original = workspace; delta = 0f }, onDragCancel = { preview = null },
                                onDragEnd = { preview?.let { value -> model.changeWorkspace {
                                    if (it.root == original?.root) it.copy(root = value.root) else it
                                } }; preview = null }) { change, amount ->
                                change.consume(); delta += (if (horizontal) amount.x else amount.y) / density
                                val extent = (if (horizontal) rect.width else rect.height) - WorkspaceState.DIVIDER
                                preview = original?.resize(id, split.ratio + delta / extent, width, logicalHeight)
                            }
                        }, contentAlignment = Alignment.Center) {
                        Box((if (horizontal) Modifier.width(WorkspaceState.DIVIDER.dp).fillMaxHeight() else Modifier.height(WorkspaceState.DIVIDER.dp).fillMaxWidth())
                            .background(MaterialTheme.colorScheme.outlineVariant))
                    }
                }
                candidate?.let { target -> rects[target.group]?.let { rect ->
                    val p = when (target.edge) {
                        DropEdge.Left -> rect.copy(width = rect.width / 2)
                        DropEdge.Right -> rect.copy(x = rect.x + rect.width / 2, width = rect.width / 2)
                        DropEdge.Top -> rect.copy(height = rect.height / 2)
                        DropEdge.Bottom -> rect.copy(y = rect.y + rect.height / 2, height = rect.height / 2)
                        DropEdge.Center -> rect
                    }
                    Box(Modifier.offset { IntOffset((p.x * density).toInt(), (p.y * density).toInt()) }.size(p.width.dp, p.height.dp)
                        .background(MaterialTheme.colorScheme.primary.copy(alpha = .18f)).border(2.dp, MaterialTheme.colorScheme.primary)
                        .semantics { contentDescription = t("탭 배치 미리보기") })
                } }
            }
        }
        activeClient?.let { client -> key(client) { WorkspaceInputTools(client, model, preferences, focus(client)) } }
    }
}

@Composable private fun WorkspaceInputTools(client: ClientConnection, model: AppModel, preferences: Preferences, focus: TerminalFocus) {
    val composer = remember(client) { model.composer(client) }
    val opened by composer.opened.collectAsState()
    val status by client.status.collectAsState()
    val blocks by model.inputBlocks.collectAsState()
    val owner = remember { Any() }
    val deleteOwner = remember { Any() }
    var deleting by remember { mutableStateOf<String?>(null) }
    WorkspaceInputBlock(model, owner, opened)
    WorkspaceInputBlock(model, deleteOwner, deleting != null)
    val allowed = { blocks.isEmpty() && model.selected.value === client && !model.inputBlocked() }
    PlatformInputRegistration(client, preferences, allowed)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(t("tmux 패널 도구"), Modifier.weight(1f).padding(start = 8.dp), style = MaterialTheme.typography.labelSmall)
        PaneToolbarToggle(preferences.mobilePaneToolbarVisible) {
            model.modify { it.copy(preferences = it.preferences.copy(mobilePaneToolbarVisible = !it.preferences.mobilePaneToolbarVisible)) }
        }
    }
    if (preferences.mobilePaneToolbarVisible) TmuxPaneToolbar(client, { focus.request() }) { deleting = it }
    TerminalShortcutBar(client, preferences.terminalShortcuts, { model.selected.value === client }, focus, mobile = true,
        openComposer = composer::open, workspace = true, inputGate = model)
    if (opened) {
        DisposableEffect(composer) { onDispose { model.blockInput(owner, false); if (model.selected.value === client) focus.request() } }
        TextInputDialog(composer, focus, { model.selected.value === client && !model.inputBlocked(owner) }, status.acceptsInput)
    }
    deleting?.let { pane -> ConfirmDialog(t("현재 tmux 패널 닫기"), t("이 대상의 원격 프로세스가 종료됩니다. 삭제할까요?"),
        { client.manage(TmuxCommand.killPane(pane)); deleting = null }, { deleting = null }) }
}

private fun findWorkspaceSplit(node: WorkspaceNode, id: String): WorkspaceNode.Split? = when (node) {
    is WorkspaceNode.Leaf -> null
    is WorkspaceNode.Split -> if (node.id == id) node else findWorkspaceSplit(node.first, id) ?: findWorkspaceSplit(node.second, id)
}

@Composable private fun Modifier.workspaceTabDrag(tab: String, origin: () -> Offset, start: (Offset) -> Unit, move: (Offset) -> Unit,
    end: () -> Unit, cancel: () -> Unit): Modifier {
    val latestOrigin by rememberUpdatedState(origin)
    val latestStart by rememberUpdatedState(start)
    val latestMove by rememberUpdatedState(move)
    val latestEnd by rememberUpdatedState(end)
    val latestCancel by rememberUpdatedState(cancel)
    return pointerInput(tab) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        val begun = if (down.type == PointerType.Mouse) awaitTouchSlopOrCancellation(down.id) { change, _ -> change.consume() }
            else awaitLongPressOrCancellation(down.id)
        if (begun != null) {
            latestStart(latestOrigin() + begun.position)
            try {
                val finished = drag(begun.id) { change -> latestMove(latestOrigin() + change.position); change.consume() }
                if (finished) latestEnd() else latestCancel()
            } catch (cancelled: kotlinx.coroutines.CancellationException) { latestCancel(); throw cancelled }
        }
    }
    }
}
