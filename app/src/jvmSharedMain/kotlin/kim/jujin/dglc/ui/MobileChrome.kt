package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.ViewConfiguration
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.*
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*
import kotlinx.coroutines.flow.first

/** Capture before opening a focusable popup; restore after its window is removed. */
@Composable internal expect fun rememberAppFocusCapture(): () -> (() -> Unit)
internal expect fun mobileNotificationPopupProperties(): PopupProperties

private object MobileHeaderIcons {
    private fun outline(name: String, path: String) = ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
        addPath(PathParser().parsePathString(path).toNodes(), fill = null, stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.8f, strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round)
    }.build()
    val hosts = outline("Hosts", "M4 4H20V10H4Z M4 14H20V20H4Z M7 7H8 M7 17H8 M16 7H17 M16 17H17")
    val terminal = outline("Terminal", "M3 4H21V20H3Z M7 9L10 12L7 15 M13 15H17")
    val bell = outline("Notifications", "M18 8A6 6 0 0 0 6 8C6 15 3 16 3 17H21C21 16 18 15 18 8Z M10 21H14")
    val chevron = outline("Expand", "M7 10L12 15L17 10")
    val more = ImageVector.Builder("More", 24.dp, 24.dp, 24f, 24f).apply {
        addPath(PathParser().parsePathString("M6 12A1 1 0 1 1 4 12A1 1 0 1 1 6 12Z M13 12A1 1 0 1 1 11 12A1 1 0 1 1 13 12Z M20 12A1 1 0 1 1 18 12A1 1 0 1 1 20 12Z").toNodes(),
            fill = SolidColor(Color.Black))
    }.build()
}

@Composable internal fun MobileHeader(hostName: String?, showingHosts: Boolean, toggleHosts: () -> Unit, openTmux: () -> Unit,
    menu: @Composable () -> Unit, notification: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().background(colors.surface)) {
        Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            MobileAppIcon()
            Spacer(Modifier.width(4.dp))
            IconButton(toggleHosts, enabled = hostName != null, modifier = Modifier.size(48.dp)
                .semantics { contentDescription = t(if (showingHosts) "터미널" else "호스트") }) {
                Icon(if (showingHosts) MobileHeaderIcons.terminal else MobileHeaderIcons.hosts, null, Modifier.size(20.dp))
            }
            Spacer(Modifier.weight(1f))
            menu()
            notification()
        }
        Row(Modifier.fillMaxWidth().height(48.dp).padding(start = 12.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(buildAnnotatedString {
                withStyle(SpanStyle(color = colors.onSurfaceVariant, fontSize = 12.sp)) { append(tr("현재 호스트: {0}", "")) }
                withStyle(SpanStyle(fontWeight = FontWeight.Medium)) { append(hostName?.let(::displayTreeText) ?: "—") }
            }, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.width(8.dp))
            TextButton(openTmux, enabled = hostName != null, modifier = Modifier.height(48.dp)
                .semantics { contentDescription = t("tmux 관리") }, contentPadding = PaddingValues(horizontal = 10.dp)) {
                Text("tmux", style = MaterialTheme.typography.labelMedium, modifier = Modifier.clearAndSetSemantics {})
                Spacer(Modifier.width(4.dp))
                Icon(MobileHeaderIcons.chevron, null, Modifier.size(14.dp))
            }
        }
        HorizontalDivider()
    }
}

@Composable internal fun MobileHeaderMenu(label: String, actions: List<Pair<String, () -> Unit>>) {
    var expanded by remember { mutableStateOf(false) }
    val model = LocalAppModel.current
    val owner = remember { Any() }
    if (model != null) WorkspaceInputBlock(model, owner, expanded)
    fun dismiss() { expanded = false; model?.blockInput(owner, false) }
    Box(Modifier.size(48.dp)) {
        IconButton({ expanded = true }, modifier = Modifier.size(48.dp).semantics { contentDescription = label }) {
            Icon(MobileHeaderIcons.more, null, Modifier.size(20.dp))
        }
        DropdownMenu(expanded, ::dismiss, shadowElevation = 0.dp, tonalElevation = 0.dp,
            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
            actions.forEachIndexed { index, (text, action) ->
                if (index > 0 && index == actions.lastIndex) HorizontalDivider()
                DropdownMenuItem(text = { Text(text) }, onClick = { dismiss(); action() })
            }
        }
    }
}

@Composable internal fun MobileConnectionTabs(connections: List<ClientConnection>, selected: ClientConnection?, state: LazyListState,
    select: (ClientConnection) -> Unit, close: (ClientConnection) -> Unit) {
    val model = LocalAppModel.current
    var viewportWidth by remember { mutableIntStateOf(0) }
    val viewConfiguration = LocalViewConfiguration.current
    val compactConfiguration = remember(viewConfiguration) { object : ViewConfiguration by viewConfiguration {
        override val minimumTouchTargetSize = DpSize(30.dp, 30.dp)
    } }
    LaunchedEffect(selected, connections, viewportWidth) {
        val index = connections.indexOf(selected)
        if (index >= 0) {
            // Wait for additions to be measured before deciding whether the whole tab is visible.
            snapshotFlow { state.layoutInfo.totalItemsCount }.first { it == connections.size }
            val layout = state.layoutInfo
            val item = layout.visibleItemsInfo.firstOrNull { it.index == index }
            if (item == null || item.offset < layout.viewportStartOffset || item.offset + item.size > layout.viewportEndOffset) {
                state.animateScrollToItem(index)
            }
        }
    }
    CompositionLocalProvider(LocalViewConfiguration provides compactConfiguration) {
    LazyRow(state = state, modifier = Modifier.fillMaxWidth().height(36.dp).onSizeChanged { viewportWidth = it.width }
        .semantics { contentDescription = t("열린 연결 탭") }, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        // Workspace IDs are Bundle-safe and remain stable across duplicate hosts and tab switches.
        // The positional fallback covers the first frame before App reconciles new connections.
        items(connections, key = { model?.tabId(it) ?: connections.indexOf(it) }) { client ->
            val title = "${displayTreeText(client.host.alias)} · ${workspaceTabTitle(client, sessionOnly = true)}"
            val active = client === selected
            Row(Modifier.widthIn(min = 112.dp, max = 240.dp).width(IntrinsicSize.Max).fillMaxHeight()
                .background(if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant)
                .selectable(active, role = Role.Tab, onClick = { select(client) })
                .semantics { contentDescription = title }.padding(start = 8.dp, end = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(title, Modifier.weight(1f).clearAndSetSemantics {}, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (active) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface)
                Box(Modifier.size(30.dp).clickable(role = Role.Button, onClick = { close(client) })
                    .semantics { contentDescription = tr("{0} 닫기", title) }, contentAlignment = Alignment.Center) {
                    Text("×", Modifier.clearAndSetSemantics {})
                }
            }
        }
    }
    }
}

@Composable internal fun MobileNotificationButton(unread: Int, open: () -> Unit, popup: @Composable (Rect) -> Unit) {
    var anchor by remember { mutableStateOf(Rect.Zero) }
    Box(Modifier.onGloballyPositioned { anchor = it.boundsInWindow() }) {
        IconButton(open, modifier = Modifier.size(48.dp).semantics { contentDescription = tr("알림 {0}", unread) }) {
            Box(Modifier.size(20.dp)) {
                Icon(MobileHeaderIcons.bell, null, Modifier.fillMaxSize())
                if (unread > 0) Box(Modifier.align(Alignment.TopEnd).size(6.dp)
                    .background(MaterialTheme.colorScheme.primary, androidx.compose.foundation.shape.CircleShape))
            }
        }
        popup(anchor)
    }
}

internal class NotificationPopupPosition(private val available: IntRect, private val margin: Int) : PopupPositionProvider {
    override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize): IntOffset {
        val left = maxOf(margin, available.left + margin)
        val right = minOf(windowSize.width - margin, available.right - margin)
        val bottom = minOf(windowSize.height - margin, available.bottom - margin)
        return IntOffset((anchorBounds.right - popupContentSize.width).coerceIn(left, maxOf(left, right - popupContentSize.width)),
            anchorBounds.bottom.coerceAtMost(maxOf(available.top + margin, bottom - popupContentSize.height)))
    }
}

@Composable internal fun MobileAgentNotifications(model: AppModel, availableBounds: Rect, anchor: Rect, restoreFocus: () -> Unit, dismiss: () -> Unit) {
    val notices by model.agentNotifications.collectAsState()
    val density = LocalDensity.current
    if (anchor == Rect.Zero || availableBounds == Rect.Zero) return
    val margin = with(density) { 8.dp.roundToPx() }
    val bounds = IntRect(availableBounds.left.toInt(), availableBounds.top.toInt(), availableBounds.right.toInt(), availableBounds.bottom.toInt())
    val width = with(density) { (availableBounds.width - 2 * margin).coerceAtLeast(0f).toDp() }.coerceAtMost(360.dp)
    val height = with(density) { (availableBounds.bottom - anchor.bottom - margin).coerceAtLeast(0f).toDp() }.coerceAtMost(480.dp)
    DisposableEffect(Unit) { onDispose { restoreFocus() } }
    Popup(popupPositionProvider = remember(bounds, margin) { NotificationPopupPosition(bounds, margin) }, onDismissRequest = dismiss,
        properties = mobileNotificationPopupProperties()) {
        Surface(Modifier.width(width).heightIn(max = height).semantics { paneTitle = t("알림") }, shadowElevation = 8.dp,
            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)) {
            Column {
                Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(tr("최근 알림 · {0} 읽지 않음", notices.count { !it.read }), Modifier.weight(1f),
                        style = MaterialTheme.typography.titleSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    TextButton(dismiss) { Text(t("닫기")) }
                }
                HorizontalDivider()
                androidx.compose.foundation.lazy.LazyColumn(Modifier.weight(1f, fill = false)) {
                    if (notices.isEmpty()) item { Text(t("수신한 알림이 없습니다."), Modifier.padding(12.dp)) }
                    items(notices, key = { it.id }) { notice ->
                        Column(Modifier.fillMaxWidth().clickable { model.openAgentNotice(notice.id); dismiss() }
                            .semantics(mergeDescendants = true) { stateDescription = t(if (notice.read) "읽음" else "읽지 않음") }
                            .padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                                Text(agentName(notice.event.agent), style = MaterialTheme.typography.titleSmall)
                                Text(t(if (notice.read) "읽음" else "읽지 않음"), style = MaterialTheme.typography.labelSmall)
                            }
                            Text(agentMessage(notice.event.kind), style = MaterialTheme.typography.bodyMedium)
                            Text(displayTreeText(model.connections.value.firstOrNull { it.agentEndpoint == notice.endpoint }?.host?.alias ?: notice.endpoint),
                                style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT)
                                .format(java.util.Date(notice.event.at)), style = MaterialTheme.typography.labelSmall)
                        }
                        HorizontalDivider()
                    }
                }
            }
        }
    }
}
