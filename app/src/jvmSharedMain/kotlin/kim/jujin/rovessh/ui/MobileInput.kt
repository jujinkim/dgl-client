package kim.jujin.rovessh.ui

import kim.jujin.rovessh.core.t
import kim.jujin.rovessh.core.tr
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kim.jujin.rovessh.TextComposer

internal expect fun mobileInputDialogProperties(): DialogProperties

@Composable internal fun PaneToolbarToggle(visible: Boolean, toggle: () -> Unit) {
    IconToggleButton(checked = visible, onCheckedChange = { toggle() }, modifier = Modifier.semantics {
        contentDescription = if (visible) t("패널 도구 접기") else t("패널 도구 펼치기")
        stateDescription = if (visible) t("펼침") else t("접힘")
    }) {
        val color = if (visible) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant
        Canvas(Modifier.size(22.dp)) {
            drawRect(color, topLeft = Offset(1.dp.toPx(), 2.dp.toPx()), size = Size(size.width - 2.dp.toPx(), size.height - 4.dp.toPx()), style = Stroke(1.5.dp.toPx()))
            drawLine(color, Offset(size.width / 2, 2.dp.toPx()), Offset(size.width / 2, size.height - 2.dp.toPx()), 1.5.dp.toPx())
        }
    }
}

@Composable internal fun KeyboardIcon() {
    val color = LocalContentColor.current
    Canvas(Modifier.size(20.dp)) {
        drawRect(color, topLeft = Offset(0f, size.height * .15f), size = Size(size.width, size.height * .7f), style = Stroke(1.dp.toPx()))
        for (row in 0..1) for (column in 0..4) {
            drawRect(color, Offset(size.width * (.12f + column * .16f), size.height * (.3f + row * .18f)), Size(1.5.dp.toPx(), 1.5.dp.toPx()))
        }
        drawLine(color, Offset(size.width * .3f, size.height * .7f), Offset(size.width * .7f, size.height * .7f), 1.dp.toPx())
    }
}

@Composable internal fun TextInputDialog(composer: TextComposer, focus: TerminalFocus, current: () -> Boolean, ready: Boolean) {
    val value by composer.value.collectAsState()
    val enter by composer.includeEnter.collectAsState()
    val sending by composer.sending.collectAsState()
    val error by composer.error.collectAsState()
    val requester = remember { FocusRequester() }
    Dialog(onDismissRequest = composer::dismiss,
        properties = mobileInputDialogProperties()) {
        BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(horizontal = 12.dp, vertical = 4.dp),
            contentAlignment = Alignment.Center) {
            val compact = maxHeight < 300.dp
            val keyboard = LocalSoftwareKeyboardController.current
            val windowFocused = LocalWindowInfo.current.isWindowFocused
            LaunchedEffect(windowFocused, maxWidth) {
                if (windowFocused) { withFrameNanos { }; requester.requestFocus(); keyboard?.show() }
            }
            val option: @Composable (Modifier) -> Unit = { modifier ->
                Row(modifier.toggleable(enter, enabled = !sending, role = Role.Checkbox,
                    onValueChange = { composer.includeEnter.value = it }), verticalAlignment = Alignment.CenterVertically) {
                    AccentCheckbox(enter, null, enabled = !sending, modifier = if (compact) Modifier.size(32.dp) else Modifier)
                    Text(t("Enter 함께 보내기"), fontSize = if (compact) 12.sp else 14.sp, maxLines = 1)
                }
            }
            Surface(Modifier.widthIn(max = 560.dp).fillMaxWidth(), color = MaterialTheme.colorScheme.surfaceContainerHigh) {
                Column(Modifier.heightIn(max = minOf(maxHeight, 420.dp)).padding(if (compact) 4.dp else 16.dp),
                    verticalArrangement = Arrangement.spacedBy(if (compact) 2.dp else 8.dp)) {
                    if (!compact) Text(t("텍스트 작성"), style = MaterialTheme.typography.titleLarge)
                    val field = Modifier.weight(1f, fill = false).fillMaxWidth().focusRequester(requester)
                        .semantics { contentDescription = t("전송할 텍스트") }
                    // Keep one editor/input session when rotation or IME insets change the layout.
                    BasicTextField(value, composer::edit, readOnly = sending,
                        modifier = field.border(1.dp, MaterialTheme.colorScheme.primary).padding(if (compact) 4.dp else 12.dp),
                        textStyle = (if (compact) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge)
                            .copy(color = MaterialTheme.colorScheme.onSurface),
                        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary), minLines = if (compact) 1 else 3, maxLines = 8)
                    if (!compact) Text(t("Enter는 줄바꿈 · 전송 버튼으로 보내기"), fontSize = 12.sp)
                    if (!compact) option(Modifier.fillMaxWidth())
                    error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall,
                        maxLines = if (compact) 1 else 3, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis) }
                    CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides if (compact) 0.dp else 48.dp) {
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.End) {
                            if (compact) option(Modifier.weight(1f))
                            val button = if (compact) Modifier.height(32.dp) else Modifier
                            val padding = if (compact) PaddingValues(horizontal = 8.dp, vertical = 2.dp) else ButtonDefaults.TextButtonContentPadding
                            TextButton(composer::dismiss, modifier = button, contentPadding = padding) { Text(t("취소")) }
                            TextButton({ composer.send(current, focus.preparePaste) }, enabled = !sending && value.text.isNotEmpty() && ready,
                                modifier = button, contentPadding = padding) { Text(if (sending) t("전송 중…") else t("전송")) }
                        }
                    }
                }
            }
        }
    }
}
