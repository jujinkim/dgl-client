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
import kotlin.math.roundToInt

internal class CliMenuOverlay {
    class Entry(val anchor: () -> Rect, val dismiss: () -> Unit, val content: @Composable () -> Unit)
    var entry by mutableStateOf<Entry?>(null)
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
                        val anchor = entry.anchor()
                        val top = (anchor.top - bounds.top).roundToInt().coerceIn(0, constraints.maxHeight)
                        val menu = measurables.single().measure(constraints.copy(minWidth = 0, minHeight = 0, maxHeight = top))
                        val left = (anchor.right - bounds.left).roundToInt() - menu.width
                        layout(constraints.maxWidth, constraints.maxHeight) {
                            // Wait for the host's window coordinates before its first visible frame.
                            if (bounds != Rect.Zero)
                                menu.place(left.coerceIn(0, (constraints.maxWidth - menu.width).coerceAtLeast(0)), top - menu.height)
                        }
                    }
                }
            }
        }
    }
}

@Composable internal fun RegisterCliMenuOverlay(overlay: CliMenuOverlay, anchor: Rect, dismiss: () -> Unit,
    content: @Composable () -> Unit) {
    val currentAnchor by rememberUpdatedState(anchor)
    val currentDismiss by rememberUpdatedState(dismiss)
    val currentContent by rememberUpdatedState(content)
    DisposableEffect(overlay) {
        val entry = CliMenuOverlay.Entry({ currentAnchor }, { currentDismiss() }, { currentContent() })
        overlay.entry = entry
        onDispose { if (overlay.entry === entry) overlay.entry = null }
    }
}

@Composable internal expect fun CliMenuDismissHandler(dismiss: () -> Unit)
