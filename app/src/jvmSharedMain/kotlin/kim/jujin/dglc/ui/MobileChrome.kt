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
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.ViewConfiguration
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.style.TextOverflow
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
        IconButton(open, modifier = Modifier.semantics { contentDescription = tr("알림 {0}", unread) }) {
            Box {
                Text("🔔", Modifier.clearAndSetSemantics {})
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
