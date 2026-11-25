package kim.jujin.dglc.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
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

/**
 * The shortcut page. It is lazy and owns its scrolling, so the settings detail never wraps it in
 * another vertical scroll; [header] adds the page heading and other settings ahead of the list.
 */
@Composable internal fun TerminalShortcutSettings(shortcuts: List<TerminalShortcut>, contentPadding: PaddingValues = PaddingValues(0.dp),
    header: LazyListScope.() -> Unit = {}, update: (List<TerminalShortcut>) -> Unit) {
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
        LazyColumn(state = list, verticalArrangement = Arrangement.spacedBy(6.dp), contentPadding = contentPadding,
            modifier = Modifier.fillMaxSize().onPreviewKeyEvent {
                if (it.type == KeyEventType.KeyDown && it.key == Key.Escape && dragging != null) {
                    dragging = null; destination = -1; true
                } else false
            }) {
            header()
            item {
                Column(Modifier.padding(top = SettingsSpacing.groupGap - 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    ShortcutSectionTitle(t("shortcut.active"))
                    Text(t("shortcut.drag_hint"), Modifier.padding(horizontal = 4.dp), style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(t("shortcut.modifiers_once"), Modifier.padding(horizontal = 4.dp), style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            if (shortcuts.isEmpty()) item { SettingsNote(t("shortcut.none_active"), Modifier.padding(vertical = 6.dp)) }
            itemsIndexed(shortcuts, key = { index, key -> "active-$index-${key.name}" }) { index, key ->
                val active = key == dragging
                val insertion = dragging != null && destination == index
                val color = if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainer
                Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight).background(color, SettingsShapes.control)
                    .border(if (insertion) 2.dp else 1.dp, if (insertion) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant, SettingsShapes.control)
                    .padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Box(Modifier.size(48.dp).semantics { contentDescription = tr("shortcut.reorder_named", key.label) }
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
                    // Icon buttons keep a 48dp target.
                    BorderedButton({ update(moveShortcut(shortcuts, index, index - 1)) }, enabled = index > 0 && dragging == null,
                        modifier = Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp).semantics { contentDescription = tr("shortcut.move_up_named", key.label) },
                        shape = SettingsShapes.control, contentPadding = PaddingValues(12.dp)) { UiGlyph(UiIcons.arrowUp) }
                    BorderedButton({ update(moveShortcut(shortcuts, index, index + 1)) }, enabled = index < shortcuts.lastIndex && dragging == null,
                        modifier = Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp).semantics { contentDescription = tr("shortcut.move_down_named", key.label) },
                        shape = SettingsShapes.control, contentPadding = PaddingValues(12.dp)) { UiGlyph(UiIcons.arrowDown) }
                    SettingsActionButton(t("shortcut.exclude"), { update(shortcuts.filterNot { it == key }) }, enabled = dragging == null,
                        modifier = Modifier.heightIn(min = 48.dp).semantics { contentDescription = tr("shortcut.exclude_named", key.label) })
                }
            }
            item { ShortcutSectionTitle(t("shortcut.excluded"), Modifier.padding(top = SettingsSpacing.groupGap - 6.dp)) }
            itemsIndexed(TerminalShortcut.all.filterNot { it in shortcuts }, key = { _, key -> "excluded-${key.name}" }) { _, key ->
                Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight).border(1.dp, MaterialTheme.colorScheme.outlineVariant, SettingsShapes.control)
                    .padding(start = SettingsSpacing.rowH, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(key.label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
                    SettingsActionButton(t("common.add"), { update(shortcuts + key) }, enabled = dragging == null,
                        modifier = Modifier.heightIn(min = 48.dp).semantics { contentDescription = tr("shortcut.add_named", key.label) })
                }
            }
            item {
                Box(Modifier.padding(top = 12.dp)) {
                    SettingsActionButton(t("shortcut.reset"), { update(TerminalShortcut.defaultShortcuts) }, enabled = dragging == null)
                }
            }
        }
        dragging?.let { key ->
            // Item offsets start after the top content padding; the overlay is placed in the box.
            val start = contentPadding.calculateLeftPadding(LocalLayoutDirection.current)
            val top = contentPadding.calculateTopPadding()
            Surface(Modifier.offset { IntOffset((start + 48.dp).roundToPx(), (pointerY + top.toPx()).roundToInt() - 24.dp.roundToPx()) }
                .zIndex(1f).clearAndSetSemantics {}, shape = SettingsShapes.control, shadowElevation = 6.dp,
                color = MaterialTheme.colorScheme.primaryContainer) {
                Text(key.label, Modifier.padding(horizontal = 20.dp, vertical = 12.dp), style = MaterialTheme.typography.titleMedium)
            }
        }
    }
}

@Composable private fun ShortcutSectionTitle(text: String, modifier: Modifier = Modifier) {
    SettingsSubheading(text, modifier.padding(horizontal = 4.dp))
}
