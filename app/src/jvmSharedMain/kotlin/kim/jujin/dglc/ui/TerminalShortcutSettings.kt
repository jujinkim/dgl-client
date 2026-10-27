package kim.jujin.dglc.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.*
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.zIndex
import androidx.compose.ui.semantics.clearAndSetSemantics
import kotlin.math.roundToInt
import kim.jujin.dglc.core.*

@Composable internal fun SettingsCard(title: String, description: String, content: @Composable ColumnScope.() -> Unit) {
    Surface(shape = RoundedCornerShape(14.dp), color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(), tonalElevation = 1.dp) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
            Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            content()
        }
    }
}

@Composable internal fun TerminalShortcutSettings(shortcuts: List<TerminalShortcut>, update: (List<TerminalShortcut>) -> Unit) {
    val list = rememberLazyListState()
    val edge = with(LocalDensity.current) { 64.dp.toPx() }
    val step = with(LocalDensity.current) { 12.dp.toPx() }
    var dragging by remember { mutableStateOf<TerminalShortcut?>(null) }
    var destination by remember { mutableIntStateOf(-1) }
    var pointerY by remember { mutableFloatStateOf(0f) }
    fun rowIndex(key: Any) = (key as? String)?.takeIf { it.startsWith("active-") }?.split('-')?.getOrNull(1)?.toIntOrNull()
    fun locate() {
        val rows = list.layoutInfo.visibleItemsInfo.filter { rowIndex(it.key) != null }
        val slot = rows.firstOrNull { pointerY < it.offset + it.size / 2 }?.let { rowIndex(it.key) }
            ?: rows.lastOrNull()?.let { rowIndex(it.key)?.plus(1) } ?: return
        val from = shortcuts.indexOf(dragging)
        destination = (slot - if (from < slot) 1 else 0).coerceIn(0, shortcuts.lastIndex)
    }
    LaunchedEffect(dragging) {
        while (dragging != null) {
            withFrameNanos { }
            val info = list.layoutInfo
            val speed = when {
                pointerY < info.viewportStartOffset + edge -> -step
                pointerY > info.viewportEndOffset - edge -> step
                else -> 0f
            }
            if (speed != 0f) { list.scrollBy(speed); locate() }
        }
    }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(state = list, verticalArrangement = Arrangement.spacedBy(6.dp),
            modifier = Modifier.fillMaxSize().onPreviewKeyEvent {
                if (it.type == KeyEventType.KeyDown && it.key == Key.Escape && dragging != null) {
                    dragging = null; destination = -1; true
                } else false
            }) {
            item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(t("사용 중인 단축키"), style = MaterialTheme.typography.titleMedium)
                    Text(t("핸들을 끌어 순서를 바꾸세요. 변경은 놓을 때 저장됩니다."), style = MaterialTheme.typography.bodySmall)
                    Text(t("Ctrl·Shift·Alt는 다음 키 입력에 한 번 적용됩니다."), style = MaterialTheme.typography.bodySmall)
                }
            }
            if (shortcuts.isEmpty()) item { Text(t("사용 중인 단축키가 없습니다."), Modifier.padding(12.dp)) }
            itemsIndexed(shortcuts, key = { index, key -> "active-$index-${key.name}" }) { index, key ->
                val active = key == dragging
                val insertion = dragging != null && destination == index
                val color = if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainer
                Row(Modifier.fillMaxWidth().background(color, RoundedCornerShape(8.dp))
                    .border(if (insertion) 2.dp else 1.dp, if (insertion) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
                    .padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(48.dp).semantics { contentDescription = tr("{0} 순서 이동", key.label) }
                        .pointerInput(shortcuts) {
                            detectDragGestures(onDragStart = { offset ->
                                dragging = key; destination = index
                                pointerY = (list.layoutInfo.visibleItemsInfo.firstOrNull { rowIndex(it.key) == index }?.offset ?: 0) + offset.y
                            }, onDragCancel = { dragging = null; destination = -1 }, onDragEnd = {
                                if (dragging != null && destination in shortcuts.indices) update(moveShortcut(shortcuts, index, destination))
                                dragging = null; destination = -1
                            }, onDrag = { change, amount -> change.consume(); pointerY += amount.y; locate() })
                        }, contentAlignment = Alignment.Center) { UiGlyph(UiIcons.grip) }
                    Text(key.label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
                    TextButton({ update(moveShortcut(shortcuts, index, index - 1)) }, enabled = index > 0 && dragging == null,
                        modifier = Modifier.semantics { contentDescription = tr("{0} 위로 이동", key.label) }, contentPadding = PaddingValues(4.dp)) { UiGlyph(UiIcons.arrowUp) }
                    TextButton({ update(moveShortcut(shortcuts, index, index + 1)) }, enabled = index < shortcuts.lastIndex && dragging == null,
                        modifier = Modifier.semantics { contentDescription = tr("{0} 아래로 이동", key.label) }, contentPadding = PaddingValues(4.dp)) { UiGlyph(UiIcons.arrowDown) }
                    TextButton({ update(shortcuts.filterNot { it == key }) }, enabled = dragging == null,
                        modifier = Modifier.semantics { contentDescription = tr("{0} 제외", key.label) }, contentPadding = PaddingValues(4.dp)) { Text(t("제외")) }
                }
            }
            item { Text(t("제외된 단축키"), Modifier.padding(top = 20.dp, bottom = 8.dp), style = MaterialTheme.typography.titleMedium) }
            itemsIndexed(TerminalShortcut.all.filterNot { it in shortcuts }, key = { _, key -> "excluded-${key.name}" }) { _, key ->
                Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(key.label, Modifier.weight(1f))
                    TextButton({ update(shortcuts + key) }, enabled = dragging == null,
                        modifier = Modifier.semantics { contentDescription = tr("{0} 추가", key.label) }) { Text(t("추가")) }
                }
            }
            item { TextButton({ update(TerminalShortcut.defaultShortcuts) }, enabled = dragging == null) { Text(t("단축키 기본값 복원")) } }
        }
        dragging?.let { key ->
            Surface(Modifier.offset { IntOffset(48.dp.roundToPx(), pointerY.roundToInt() - 24.dp.roundToPx()) }
                .zIndex(1f).clearAndSetSemantics {}, shape = RoundedCornerShape(8.dp), shadowElevation = 6.dp,
                color = MaterialTheme.colorScheme.primaryContainer) {
                Text(key.label, Modifier.padding(horizontal = 20.dp, vertical = 12.dp), style = MaterialTheme.typography.titleMedium)
            }
        }
    }
}
