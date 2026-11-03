package kim.jujin.dglc.ui

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt

internal class CliMenuOverlay {
    class Entry(val anchor: () -> Rect, val dismiss: () -> Unit, val content: @Composable () -> Unit,
        val placement: CliMenuPlacement = CliMenuPlacement.AboveEnd)
    var entry by mutableStateOf<Entry?>(null)
}

/** The launcher opens above its button's end; a text selection menu flips around its selection. */
internal enum class CliMenuPlacement { AboveEnd, AroundCenter }

/** Menu offset inside a [width]x[height] overlay. [anchor] uses the overlay's coordinates. */
internal fun cliMenuPosition(placement: CliMenuPlacement, anchor: Rect, menu: IntSize, width: Int, height: Int, gap: Int): IntOffset {
    fun clampX(x: Int) = x.coerceIn(0, (width - menu.width).coerceAtLeast(0))
    return when (placement) {
        CliMenuPlacement.AboveEnd -> IntOffset(clampX(anchor.right.roundToInt() - menu.width),
            anchor.top.roundToInt().coerceIn(0, height) - menu.height)
        CliMenuPlacement.AroundCenter -> {
            val above = anchor.top.roundToInt() - gap - menu.height
            val below = anchor.bottom.roundToInt() + gap
            // A selection taller than the free space keeps the menu inside the overlay.
            val y = if (above >= 0) above else if (below + menu.height <= height) below
                else above.coerceIn(0, (height - menu.height).coerceAtLeast(0))
            IntOffset(clampX(anchor.center.x.roundToInt() - menu.width / 2), y)
        }
    }
}

internal val LocalCliMenuOverlay = staticCompositionLocalOf<CliMenuOverlay?> { null }

/** Android keeps the menu in the editor's Activity window, without moving input focus. */
@Composable internal fun CliMenuOverlayHost(content: @Composable () -> Unit) {
    val overlay = remember { CliMenuOverlay() }
    CompositionLocalProvider(LocalCliMenuOverlay provides overlay) {
        Box(Modifier.fillMaxSize()) {
            content()
            overlay.entry?.let { entry ->
                CliMenuDismissHandler(entry.dismiss)
                var bounds by remember { mutableStateOf(Rect.Zero) }
                Box(Modifier.matchParentSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
                    // This sibling owns the whole outside gesture, so dismissing cannot click
                    // the composer, switch panes, or send a terminal mouse event underneath.
                    Box(Modifier.matchParentSize().pointerInput(entry) {
                        awaitEachGesture {
                            awaitFirstDown(requireUnconsumed = false).consume()
                            do {
                                val event = awaitPointerEvent()
                                event.changes.forEach { it.consume() }
                            } while (event.changes.any { it.pressed })
                            entry.dismiss()
                        }
                    })
                    Layout(content = entry.content, modifier = Modifier.fillMaxSize()
                        .onGloballyPositioned { bounds = it.boundsInWindow() }) { measurables, constraints ->
                        val anchor = entry.anchor().translate(-bounds.left, -bounds.top)
                        val maxHeight = if (entry.placement == CliMenuPlacement.AboveEnd)
                            anchor.top.roundToInt().coerceIn(0, constraints.maxHeight) else constraints.maxHeight
                        val menu = measurables.single().measure(constraints.copy(minWidth = 0, minHeight = 0, maxHeight = maxHeight))
                        val position = cliMenuPosition(entry.placement, anchor, IntSize(menu.width, menu.height),
                            constraints.maxWidth, constraints.maxHeight, 8.dp.roundToPx())
                        layout(constraints.maxWidth, constraints.maxHeight) {
                            // Wait for the host's window coordinates before its first visible frame.
                            if (bounds != Rect.Zero) menu.place(position)
                        }
                    }
                }
            }
        }
    }
}

@Composable internal fun RegisterCliMenuOverlay(overlay: CliMenuOverlay, anchor: Rect, dismiss: () -> Unit,
    placement: CliMenuPlacement = CliMenuPlacement.AboveEnd, content: @Composable () -> Unit) {
    val currentAnchor by rememberUpdatedState(anchor)
    val currentDismiss by rememberUpdatedState(dismiss)
    val currentContent by rememberUpdatedState(content)
    DisposableEffect(overlay, placement) {
        val entry = CliMenuOverlay.Entry({ currentAnchor }, { currentDismiss() }, { currentContent() }, placement)
        overlay.entry = entry
        onDispose { if (overlay.entry === entry) overlay.entry = null }
    }
}

@Composable internal expect fun CliMenuDismissHandler(dismiss: () -> Unit)
