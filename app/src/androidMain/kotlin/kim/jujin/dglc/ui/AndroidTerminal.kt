package kim.jujin.dglc.ui

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.inputmethod.InputMethodManager
import android.view.WindowInsets as AndroidInsets
import android.webkit.*
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.BackHandler
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.SmallFloatingActionButton
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Alignment
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import kim.jujin.dglc.ClientConnection
import kim.jujin.dglc.AppModel
import kim.jujin.dglc.SnackbarDuration
import kim.jujin.dglc.pasteTerminalText
import kim.jujin.dglc.terminalTextWithoutLineBreaks
import kim.jujin.dglc.terminalWebLink
import kim.jujin.dglc.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.util.Base64
import java.lang.ref.WeakReference

internal actual fun mobileInputDialogProperties() = androidx.compose.ui.window.DialogProperties(
    usePlatformDefaultWidth = false, decorFitsSystemWindows = false)

// Take keys/back and consume outside touches without replacing the terminal as the IME target.
internal actual fun imePreservingPopupProperties() = androidx.compose.ui.window.PopupProperties(
    flags = android.view.WindowManager.LayoutParams.FLAG_WATCH_OUTSIDE_TOUCH or android.view.WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM,
    inheritSecurePolicy = true)

@Composable internal actual fun rememberAppFocusCapture(): () -> (() -> Unit) {
    val view = LocalView.current
    return remember(view) { {
        val previous = WeakReference(view.rootView.findFocus())
        val restore: () -> Unit = {
            view.post { previous.get()?.takeIf { it.isAttachedToWindow }?.requestFocus() }
        }
        restore
    } }
}

private class TerminalEntry(val terminal: AndroidTerminal) {
    var focus = TerminalFocus()
    var visible = false
    var restoreIme = false
    val keyboardVisibility = TerminalKeyboardVisibility()
    var updateKeyboard: () -> Unit = {}
    var selection by mutableStateOf<TerminalSelection?>(null)
}
private tailrec fun Context.terminalLifecycleOwner(): LifecycleOwner? = when (this) {
    is LifecycleOwner -> this
    is ContextWrapper -> baseContext.terminalLifecycleOwner()
    else -> null
}
private class TerminalResources(val model: AppModel) {
    val entries = java.util.IdentityHashMap<ClientConnection, TerminalEntry>()
    fun remove(client: ClientConnection) { entries.remove(client)?.let { client.unbindSurface(it.terminal); it.terminal.close() } }
    fun close() { entries.keys.toList().forEach(::remove) }
}
private val LocalTerminalResources = staticCompositionLocalOf<TerminalResources?> { null }

/** Activity-owned renderers, independent of group membership and window width. */
@Composable fun AndroidTerminalResources(model: AppModel, content: @Composable () -> Unit) {
    val resources = remember(model) { TerminalResources(model) }
    val clients = model.connections.collectAsState().value
    SideEffect { resources.entries.keys.toList().filter { it !in clients }.forEach(resources::remove) }
    DisposableEffect(resources) { onDispose { resources.close() } }
    CompositionLocalProvider(LocalTerminalResources provides resources, content = content)
}

@Composable actual fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier) {
    val context = LocalContext.current
    val composeView = LocalView.current
    val focus = LocalTerminalFocus.current
    val resources = LocalTerminalResources.current
    val entry = remember(client, resources) {
        fun create(): TerminalEntry {
            lateinit var entry: TerminalEntry
            entry = TerminalEntry(AndroidTerminal(context, preferences,
                { if (entry.visible && entry.focus.inputAllowed()) client.input(it) },
                { if (entry.visible) client.resize(it) },
                { pane, target, bytes -> if (entry.visible && entry.focus.inputAllowed()) client.paneInput(pane, target, bytes) },
                { pane, target -> if (entry.visible) { entry.focus.activate(); if (entry.focus.inputAllowed()) client.focusPane(pane, target) } },
                { pane, target, size -> if (entry.visible && entry.focus.inputAllowed()) client.resizePane(pane, target, size) },
                resources?.model?.terminalFont(client) ?: TerminalFontSize(preferences.fontSize),
                blockedInput = { if (entry.visible) client.inputBlocked() },
                pointerInput = { pane, target, bytes -> if (entry.visible && entry.focus.inputAllowed()) client.pointerInput(pane, target, bytes) },
                keyboardInput = { pane, target, key, text, physical, cursor ->
                    if (entry.visible && entry.focus.inputAllowed() && target != null &&
                        client.captureInputTarget()?.identity == kim.jujin.dglc.inputTargetIdentity(target)) {
                        resources?.model?.shortcutInput(client)?.encode(key, text, physical, cursor)?.let { client.paneInput(pane, target, it) }
                    }
                },
                selection = { if (entry.visible) entry.selection = it },
                // Before the user's own input only; the bridge never marks emulator replies.
                userInput = { if (entry.visible && entry.focus.inputAllowed()) client.claimSize(force = false) }))
            return entry
        }
        resources?.entries?.getOrPut(client, ::create) ?: create()
    }
    val terminal = entry.terminal
    val alwaysShowKeyboard by rememberUpdatedState(preferences.androidKeyboardAlwaysVisible)
    SideEffect { entry.focus = focus; entry.updateKeyboard() }
    // IME Back normally hides the keyboard first. Consume any Back that reaches
    // the Activity during a show/hide handoff instead of leaving the terminal.
    BackHandler(enabled = preferences.androidKeyboardAlwaysVisible && focus.keyboardAllowed()) {
        if (alwaysShowKeyboard && entry.focus.keyboardAllowed()) {
            entry.keyboardVisibility.update(TerminalKeyboardVisibility.State())
            entry.updateKeyboard()
        }
    }
    val allowed = focus.inputAllowed()
    val connectionStatus by client.status.collectAsState()
    LaunchedEffect(terminal, connectionStatus.acceptsPointer) { terminal.setPointerEnabled(connectionStatus.acceptsPointer) }
    LaunchedEffect(terminal, allowed) {
        terminal.setInputEnabled(allowed)
        if (allowed && entry.visible && entry.restoreIme) {
            entry.restoreIme = false
            terminal.keyboard(true) { entry.visible && entry.focus.inputAllowed() }
        }
    }
    LaunchedEffect(terminal, preferences.smoothTerminalTransitions) {
        terminal.setSmoothTransitions(preferences.smoothTerminalTransitions)
    }
    DisposableEffect(terminal, focus, composeView) {
        terminal.view.setOnTouchListener { _, event ->
            if (event.actionMasked == android.view.MotionEvent.ACTION_DOWN) entry.focus.activate()
            false
        }
        terminal.view.onFocusChangeListener = android.view.View.OnFocusChangeListener { _, hasFocus ->
            if (hasFocus) entry.focus.activate()
            entry.updateKeyboard()
        }
        // Read the same native insets used at click time. Compose's target IME visibility
        // can lag a cancelled show/hide animation while the WebView owns input focus.
        var observing = true
        var imeAnimation: android.view.WindowInsetsAnimation? = null
        var imeVersion = 0L
        val lifecycle = context.terminalLifecycleOwner()?.lifecycle
        fun keyboardEligible() = observing && entry.visible && terminal.keyboardReady && terminal.view.isAttachedToWindow &&
            lifecycle?.currentState?.isAtLeast(Lifecycle.State.RESUMED) == true && terminal.view.hasWindowFocus() &&
            entry.focus.keyboardAllowed() && (resources == null ||
                (resources.model.selected.value === client && !resources.model.inputBlocked()))
        entry.updateKeyboard = {
            val request = entry.keyboardVisibility.update(TerminalKeyboardVisibility.State(
                enabled = alwaysShowKeyboard, eligible = keyboardEligible(),
                visible = terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) == true,
                focused = terminal.view.hasFocus() && terminal.editorFocused,
                animating = imeAnimation != null))
            if (request != null) terminal.view.post {
                if (entry.keyboardVisibility.isCurrent(request) && alwaysShowKeyboard && keyboardEligible()) {
                    terminal.keyboard(true) {
                        entry.keyboardVisibility.isCurrent(request) && alwaysShowKeyboard && keyboardEligible()
                    }
                }
            }
        }
        terminal.keyboardFocusChanged = { entry.updateKeyboard() }
        val updateIme = { if (observing) {
            val next = terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) == true
            focus.imeVisible = next
            entry.updateKeyboard()
        } }
        val observer = android.view.ViewTreeObserver.OnPreDrawListener { updateIme(); true }
        val treeObserver = composeView.viewTreeObserver
        treeObserver.addOnPreDrawListener(observer)
        val windowObserver = android.view.ViewTreeObserver.OnWindowFocusChangeListener { updateIme() }
        treeObserver.addOnWindowFocusChangeListener(windowObserver)
        val lifecycleObserver = LifecycleEventObserver { _, _ -> terminal.view.post { updateIme() } }
        lifecycle?.addObserver(lifecycleObserver)
        terminal.view.setOnApplyWindowInsetsListener { view, insets ->
            view.post { updateIme() }
            view.onApplyWindowInsets(insets)
        }
        terminal.view.setWindowInsetsAnimationCallback(object : android.view.WindowInsetsAnimation.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
            override fun onPrepare(animation: android.view.WindowInsetsAnimation) {
                if (animation.typeMask and AndroidInsets.Type.ime() != 0) {
                    imeAnimation = animation
                    imeVersion = terminal.beginImeAnimation()
                    entry.updateKeyboard()
                }
            }
            override fun onProgress(insets: AndroidInsets, animations: MutableList<android.view.WindowInsetsAnimation>): AndroidInsets {
                updateIme(); return insets
            }
            override fun onEnd(animation: android.view.WindowInsetsAnimation) {
                terminal.view.post {
                    if (observing && imeAnimation === animation) {
                        imeAnimation = null
                        terminal.endImeAnimation(imeVersion)
                    }
                    updateIme()
                }
            }
        })
        terminal.refreshViewport()
        terminal.view.post { updateIme() }
        focus.request = { terminal.focus { entry.visible && entry.focus.inputAllowed() } }
        focus.keyboard = {
            if (!alwaysShowKeyboard) terminal.keyboard(terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) != true, focus.inputAllowed)
        }
        focus.preparePaste = terminal::preparePaste
        focus.shortcut = terminal::shortcut
        onDispose {
            terminal.finishFontGesture()
            terminal.cancelImeAnimation()
            // Detach effects can run in either order. Retain the native snapshot captured by
            // the terminal effect even if Compose has already observed the IME disappearing.
            entry.restoreIme = entry.restoreIme || (entry.focus.inputAllowed() &&
                (focus.imeVisible || terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) == true))
            observing = false
            entry.keyboardVisibility.update(TerminalKeyboardVisibility.State())
            entry.updateKeyboard = {}
            terminal.keyboardFocusChanged = {}
            lifecycle?.removeObserver(lifecycleObserver)
            if (treeObserver.isAlive) {
                treeObserver.removeOnPreDrawListener(observer)
                treeObserver.removeOnWindowFocusChangeListener(windowObserver)
            }
            terminal.view.setOnApplyWindowInsetsListener(null)
            terminal.view.setWindowInsetsAnimationCallback(null)
            terminal.view.setOnTouchListener(null)
            terminal.view.onFocusChangeListener = null
            focus.request = {}; focus.keyboard = {}; focus.shortcut = {}; focus.imeVisible = false
            focus.preparePaste = { _ -> error(t("terminal.closed")) }
        }
    }
    DisposableEffect(terminal) {
        entry.visible = true
        client.bindSurface(terminal)
        // Cover a retained WebView before its very first reattachment frame, or keep its frozen
        // last frame when it reattaches to the window it presented last.
        client.status.value.let { terminal.preparePresentation(it.presenting, it.presentingPreview) }
        if (entry.restoreIme && client.status.value.acceptsInput) {
            entry.restoreIme = false
            terminal.view.post { terminal.view.postOnAnimation {
                if (entry.visible) terminal.keyboard(true) { entry.visible && entry.focus.inputAllowed() }
            } }
        }
        onDispose {
            entry.restoreIme = entry.restoreIme || (entry.focus.inputAllowed() && terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) == true)
            entry.visible = false; client.unbindSurface(terminal)
            entry.selection = null; terminal.clearSelection()
            terminal.preparePresentation(true)
            terminal.releasePresentationFrame()
            if (resources == null) terminal.close()
        }
    }
    LaunchedEffect(terminal, preferences.fontSize, preferences.terminalFontFamily, preferences.scrollback, preferences.effectiveAndroidKeyboardMode) { terminal.configure(preferences) }
    val status by client.status.collectAsState()
    val presenting = terminal.screenPending || status.presenting
    // A reattachment keeps the tab's last frame; failures and session selection still need the opaque curtain.
    val presentationNotice = status.presentationNotice
    val previewing = presenting && terminal.presentationPreview && !presentationNotice
    var previewSlow by remember(terminal) { mutableStateOf(false) }
    LaunchedEffect(terminal, previewing) {
        previewSlow = false
        if (previewing) { delay(250); previewSlow = true }
    }
    val fontOverride by terminal.fontSizeOverride.collectAsState()
    val occlusion = LocalTerminalOcclusion.current
    var terminalBounds by remember { mutableStateOf(Rect.Zero) }
    val menuScope = rememberCoroutineScope()
    val overlay = LocalCliMenuOverlay.current
    Box(modifier.clipToBounds().onGloballyPositioned { terminalBounds = it.boundsInWindow() }) {
        key(terminal) { AndroidView(factory = {
            (terminal.view.parent as? android.view.ViewGroup)?.removeView(terminal.view)
            terminal.view
        }, modifier = Modifier.fillMaxSize().layout { measurable, constraints ->
            val hidden = occlusion.getBottom(this)
            val visible = constraints.maxHeight
            val logical = visible + hidden
            val placeable = measurable.measure(Constraints.fixed(constraints.maxWidth, logical))
            terminal.setViewport(TerminalViewport(placeable.width, logical, visible, hidden > 0))
            layout(placeable.width, visible) { placeable.place(0, 0) }
        }) }
        fontOverride?.let { size ->
            SmallFloatingActionButton(onClick = {
                terminal.resetFontSize(resources?.model?.local?.value?.preferences?.fontSize ?: preferences.fontSize)
            },
                modifier = Modifier.align(Alignment.TopEnd).padding(8.dp).height(28.dp)
                    .focusProperties { canFocus = false }.semantics {
                        contentDescription = t("terminal.reset_session_font_size")
                        stateDescription = tr("terminal.current_font_size", size)
                    }, shape = androidx.compose.foundation.shape.CircleShape) {
                Row(Modifier.padding(horizontal = 10.dp).clearAndSetSemantics {},
                    horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    UiGlyph(UiIcons.fontSize)
                    Text(size.toString(), fontSize = 14.sp, lineHeight = 20.sp)
                    UiGlyph(UiIcons.refresh)
                }
            }
        }
        if (previewing) {
            if (previewSlow) Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = .25f)), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(Modifier.size(28.dp).semantics { contentDescription = t("terminal.restoring") },
                    color = Color.White, strokeWidth = 3.dp)
            }
        } else if (presenting) {
            Box(Modifier.fillMaxSize().background(Color(0xff11151b)), contentAlignment = Alignment.Center) {
                Text(status.presentationMessage, color = Color.White, modifier = Modifier.padding(24.dp))
            }
        }
        status.recoveryMessage?.takeUnless { status.retryExhausted || presenting }?.let { message ->
            Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = .45f)), contentAlignment = Alignment.Center) {
                Text(message, color = Color.White, modifier = Modifier.padding(24.dp))
            }
        }
    }
    val selection = entry.selection
    if (selection != null && overlay != null && !presenting)
        TerminalSelectionPopup(client, entry, selection, overlay, terminalBounds, menuScope, allowed && status.acceptsInput)
}

/** The selection menu above the terminal. Every action, an outside tap and Back clear the selection. */
@Composable private fun TerminalSelectionPopup(client: ClientConnection, entry: TerminalEntry, selection: TerminalSelection,
    overlay: CliMenuOverlay, terminalBounds: Rect, scope: CoroutineScope, inputAllowed: Boolean) {
    val context = LocalContext.current
    val terminal = entry.terminal
    val clipboard = remember(context) { context.applicationContext.getSystemService(ClipboardManager::class.java) }
    val dismiss = { if (entry.selection === selection) { entry.selection = null; terminal.clearSelection() } }
    fun notice(message: String) = client.inputNotice.show(message, SnackbarDuration.Short)
    // Android 13+ confirms clipboard writes itself, so copying shows no app notice.
    fun clip(text: String) = clipboard.setPrimaryClip(ClipData.newPlainText("DaGaLi", text))
    // Read at action time, then close the selection before writing the clipboard.
    fun copySelection(transform: (String) -> String) = scope.launch {
        val text = terminal.selectedText()?.let(transform)
        dismiss()
        if (text.isNullOrEmpty()) notice(t("terminal.nothing_to_copy")) else clip(text)
    }
    // Paste only reaches the pane that was the input target when the menu opened.
    val pasteTarget = remember(selection) {
        selection.target?.let { raw -> runCatching { kim.jujin.dglc.inputTargetIdentity(raw) }.getOrNull() }
            ?.let { identity -> client.captureInputTarget()?.takeIf { it.identity == identity } }
    }
    val anchor = with(selection.bounds.translate(terminalBounds.topLeft)) {
        Rect(left.coerceIn(terminalBounds.left, terminalBounds.right), top.coerceIn(terminalBounds.top, terminalBounds.bottom),
            right.coerceIn(terminalBounds.left, terminalBounds.right), bottom.coerceIn(terminalBounds.top, terminalBounds.bottom))
    }
    RegisterCliMenuOverlay(overlay, anchor, dismiss, CliMenuPlacement.AroundCenter) {
        TerminalSelectionMenu(selection.url,
            paste = pasteTarget?.let { target -> {
                dismiss()
                val text = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString().orEmpty()
                scope.launch {
                    pasteTerminalText(client, target, { entry.visible && entry.focus.inputAllowed() }, text, terminal::preparePaste)
                        ?.let(::notice)
                }
            } },
            pasteEnabled = inputAllowed,
            copy = { copySelection { it } },
            copyWithoutLineBreaks = if (selection.lineBreaks) { { copySelection(::terminalTextWithoutLineBreaks) } } else null,
            openLink = {
                val url = selection.url
                dismiss()
                if (url != null) try {
                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addCategory(Intent.CATEGORY_BROWSABLE))
                } catch (_: ActivityNotFoundException) { notice(t("terminal.no_link_handler")) }
            },
            copyLink = { selection.url?.let(::clip); dismiss() })
    }
}

@SuppressLint("SetJavaScriptEnabled")
class AndroidTerminal(context: Context, preferences: Preferences, private val input: (ByteArray) -> Unit, private val resize: (Size) -> Unit,
    private val paneInput: ((String, String?, ByteArray) -> Unit)? = null,
    private val focusPane: (String, String?) -> Unit = { _, _ -> },
    private val resizePane: (String, String?, Size) -> Unit = { _, _, _ -> },
    private val font: TerminalFontSize = TerminalFontSize(preferences.fontSize),
    private val blockedInput: () -> Unit = {},
    private val pointerInput: ((String, String?, ByteArray) -> Unit)? = null,
    private val keyboardInput: (String, String?, String, String?, TerminalModifiers, Boolean) -> Unit = { _, _, _, _, _, _ -> },
    private val selection: (TerminalSelection?) -> Unit = {},
    private val userInput: () -> Unit = {}) : TerminalSurface {
    var screenPending by mutableStateOf(false); private set
    /** A reattachment shows the frozen last frame (input still blocked) instead of the curtain. */
    var presentationPreview by mutableStateOf(false); private set
    internal var editorFocused = false; private set
    internal var keyboardFocusChanged: () -> Unit = {}
    fun preparePresentation(active: Boolean, preview: Boolean = false) {
        val keep = active && preview
        screenPending = active && !keep
        presentationPreview = keep
        view.importantForAccessibility = if (active) android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            else android.view.View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        // Freeze before any reattachment layout/reset reaches the page; repeated calls keep the first copy.
        if (keep && !closed && ready.isCompleted) view.evaluateJavascript("window.setPresentationCache(true)", null)
        view.invalidate()
    }
    /** Releases the frozen frame. Only after presentation is confirmed, on cleanup, or on detach. */
    fun releasePresentationFrame() {
        if (!closed && ready.isCompleted) view.evaluateJavascript("window.setPresentationCache(false)", null)
    }
    override suspend fun presentationPending(active: Boolean, preview: Boolean) {
        withContext(Dispatchers.Main.immediate) { if (!closed) preparePresentation(active, preview) }
    }
    override suspend fun releasePresentationCache() {
        withContext(Dispatchers.Main.immediate) { releasePresentationFrame() }
    }
    val fontSizeOverride get() = font.overrideSize
    private var scrollback = preferences.scrollback.coerceIn(100, 10000)
    private var keyboardMode = preferences.effectiveAndroidKeyboardMode
    /** The installed face served at `/fonts/<token>`; read by the WebView's request thread. */
    @Volatile private var terminalFont: Pair<String, TerminalFontFile>? = null
    private var consumingPinch = false
    private var managedViewport = false
    private val scaleDetector = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean { font.beginGesture(); return true }
        override fun onScale(detector: ScaleGestureDetector): Boolean {
            if (font.scale(detector.scaleFactor.toDouble())) applyConfiguration()
            return true
        }
    }).apply { isQuickScaleEnabled = false; isStylusScaleEnabled = false }
    val view = object : WebView(context) {
        override fun onDraw(canvas: android.graphics.Canvas) {
            super.onDraw(canvas)
            // Keep measuring/painting behind an opaque curtain. GONE/INVISIBLE
            // would stop the render acknowledgements that release it.
            if (screenPending) canvas.drawColor(android.graphics.Color.rgb(17, 21, 27))
        }
        // DGL measures this WebView at the keyboard-closed logical height and clips it outside.
        // Chromium M139+ would also shrink/scroll its visual viewport for these insets, shifting
        // fixed terminal content and the panel scrollbar above the tabs. Zero them (never consume)
        // so WebView also clears any earlier inset state.
        override fun onApplyWindowInsets(insets: AndroidInsets): AndroidInsets = super.onApplyWindowInsets(
            AndroidInsets.Builder(insets).setInsets(AndroidInsets.Type.ime() or AndroidInsets.Type.systemBars() or
                AndroidInsets.Type.displayCutout(), android.graphics.Insets.NONE).build())
        override fun onAttachedToWindow() { super.onAttachedToWindow(); refreshViewport() }
        override fun onDetachedFromWindow() { cancelImeAnimation(); super.onDetachedFromWindow() }
        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
            super.onSizeChanged(w, h, oldw, oldh)
            // Standalone renderer fixtures have no Compose geometry provider.
            if (!managedViewport && viewport.update(TerminalViewport(w, h, h, false))) dispatchViewport()
        }
        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(event: MotionEvent): Boolean {
            if (closed) return false
            if (event.actionMasked == MotionEvent.ACTION_DOWN) finishFontGesture()
            if (event.pointerCount > 1 && !consumingPinch) {
                consumingPinch = true
                // Cancel the first finger before xterm can turn the pinch into
                // scrolling, a click, a mouse report or a pane boundary drag.
                val cancel = MotionEvent.obtain(event).apply { action = MotionEvent.ACTION_CANCEL }
                try { super.onTouchEvent(cancel) } finally { cancel.recycle() }
                parent?.requestDisallowInterceptTouchEvent(true)
                if (ready.isCompleted) evaluateJavascript("window.setFontGesture(true)", null)
            }
            scaleDetector.onTouchEvent(event)
            if (!consumingPinch) return super.onTouchEvent(event)
            if (event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL) finishFontGesture()
            return true
        }
    }
    private val ready = CompletableDeferred<Unit>()
    private val mutex = Mutex()
    private var serial = 0L
    private var awaiting: Pair<Long, CompletableDeferred<Unit>>? = null
    private var closed = false
    internal val keyboardReady get() = ready.isCompleted && !closed
    private val imeAnimation = TerminalImeAnimation()
    private val viewport = TerminalViewportState()
    private var viewportScheduled = false
    private val viewportFrame = Runnable {
        viewportScheduled = false
        val value = viewport.viewport
        if (!closed && ready.isCompleted && viewport.accepts(viewport.version) && value != null) {
            val density = view.resources.displayMetrics.density.toDouble()
            val geometry = JSONObject().put("version", viewport.version).put("width", value.width / density)
                .put("logicalHeight", value.logicalHeight / density).put("visibleHeight", value.visibleHeight / density)
                .put("imeVisible", value.imeVisible)
            view.evaluateJavascript("window.setViewport($geometry)", null)
        }
    }
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    init {
        font.configure(preferences.fontSize)
        val loader = WebViewAssetLoader.Builder().addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(context)).build()
        with(view.settings) {
            javaScriptEnabled = true
            allowFileAccess = false; allowContentAccess = false
            domStorageEnabled = false; databaseEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
            setSupportZoom(false)
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            blockNetworkLoads = true
            safeBrowsingEnabled = true
            mediaPlaybackRequiresUserGesture = true
        }
        view.isFocusableInTouchMode = true
        view.isVerticalScrollBarEnabled = false
        view.isHorizontalScrollBarEnabled = false
        view.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?) = true
            override fun shouldInterceptRequest(view: WebView?, request: WebResourceRequest): WebResourceResponse {
                val uri = request.url
                if (uri.scheme != "https" || uri.host != "appassets.androidplatform.net") return blocked()
                if (uri.path in assets) return loader.shouldInterceptRequest(uri) ?: blocked()
                // Only the face selected now, never a path the page chooses.
                val (token, face) = terminalFont ?: return blocked()
                if (uri.path != "/fonts/$token" || !uri.query.isNullOrEmpty()) return blocked()
                return try {
                    val bytes = face.standalone()
                    val type = if (bytes.size >= 4 && bytes.decodeToString(0, 4) == "OTTO") "font/otf" else "font/ttf"
                    WebResourceResponse(type, null, 200, "OK", mapOf("Cache-Control" to "no-store"), ByteArrayInputStream(bytes))
                } catch (_: Exception) { blocked() }
            }
        }
        view.webChromeClient = object : WebChromeClient() {
            override fun onPermissionRequest(request: PermissionRequest) { request.deny() }
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
        WebViewCompat.addWebMessageListener(view, "dgl", setOf("https://appassets.androidplatform.net")) { _, message, origin, isMainFrame, _ ->
            if (!isMainFrame || origin.toString() != "https://appassets.androidplatform.net" || closed) return@addWebMessageListener
            val raw = message.data ?: return@addWebMessageListener
            if (raw.length > 65536) return@addWebMessageListener
            try {
                val json = JSONObject(raw)
                fun target() = json.optString("target").takeUnless { it.isEmpty() || it == "null" }
                fun inputForPane(bytes: ByteArray) {
                    if (bytes.size <= 65536) {
                        if (json.optBoolean("pointer") && pointerInput != null) pointerInput.invoke(json.getString("pane"), target(), bytes)
                        else if (paneInput != null) paneInput.invoke(json.getString("pane"), target(), bytes) else input(bytes)
                    }
                }
                val type = json.getString("type")
                // Runs before the input it precedes, so the core takes the window size back first.
                if (kim.jujin.dglc.core.TerminalSizeClaims.fromBridge(type, json.optBoolean("user"))) userInput()
                when (type) {
                    "ready" -> {
                        applyConfiguration()
                        applyTerminalFont()
                        ready.complete(Unit)
                        dispatchViewport()
                        keyboardFocusChanged()
                    }
                    "editor-focus" -> {
                        editorFocused = json.getBoolean("focused")
                        keyboardFocusChanged()
                    }
                    "blocked-input" -> blockedInput()
                    "input" -> {
                        val bytes = json.getString("data").toByteArray(Charsets.UTF_8)
                        inputForPane(bytes)
                    }
                    "keyboard" -> keyboardInput(json.getString("pane"), target(), json.getString("key"),
                        if (json.isNull("text")) null else json.getString("text"),
                        TerminalModifiers(json.optBoolean("control"), json.optBoolean("shift"), json.optBoolean("alt")), json.optBoolean("applicationCursor"))
                    "input-binary" -> inputForPane(kim.jujin.dglc.core.TerminalInput.decodeBinary(json.getString("data")))
                    "focus" -> focusPane(json.getString("pane"), target())
                    "resize-pane" -> resizePane(json.getString("pane"), target(), Size(json.getInt("columns"), json.getInt("rows")))
                    "resize" -> {
                        val columns = json.getInt("columns"); val rows = json.getInt("rows")
                        if (columns in 2..500 && rows in 2..250) resize(Size(columns, rows))
                    }
                    "ack" -> if (json.getLong("id") == awaiting?.first) awaiting?.second?.complete(Unit)
                    "selection" -> when (json.getString("state")) {
                        "start" -> {
                            // Keep a sideways selection drag inside the WebView.
                            view.parent?.requestDisallowInterceptTouchEvent(true)
                            view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                        }
                        "menu" -> {
                            val pane = json.getString("pane").takeIf { it.length in 1..64 } ?: return@addWebMessageListener
                            val density = view.resources.displayMetrics.density
                            fun px(name: String, limit: Int): Float {
                                val value = json.getDouble(name)
                                require(value.isFinite())
                                return (value * density).toFloat().coerceIn(0f, limit.toFloat())
                            }
                            val left = px("left", view.width); val right = px("right", view.width)
                            val top = px("top", view.height); val bottom = px("bottom", view.height)
                            val url = if (json.isNull("url")) null else terminalWebLink(json.getString("url"))
                            selection(TerminalSelection(pane, target(), url, Rect(minOf(left, right), minOf(top, bottom), maxOf(left, right), maxOf(top, bottom)),
                                lineBreaks = json.optBoolean("lineBreaks")))
                        }
                        "cleared" -> selection(null)
                    }
                }
            } catch (_: Exception) { /* Untrusted bridge messages never reach the model. */ }
        }
        } else error("Update Android System WebView")
        view.loadUrl("https://appassets.androidplatform.net/assets/terminal/index.html")
    }
    override suspend fun layout(remote: RemoteState, window: String?, pane: String?, inputTarget: String?) {
        withTimeout(15_000) { ready.await() }
        val w = remote.windows.firstOrNull { it.id == window }
        val value = JSONObject().put("window", window ?: JSONObject.NULL).put("active", pane ?: JSONObject.NULL)
            .put("inputTarget", inputTarget ?: JSONObject.NULL)
            .put("inputIdentity", inputTarget?.let { kim.jujin.dglc.inputTargetIdentity(it) } ?: JSONObject.NULL)
            .put("columns", w?.width ?: 80).put("rows", w?.height ?: 24)
            .put("panes", org.json.JSONArray().apply { remote.panes.forEach { p -> put(JSONObject()
                .put("id", p.id).put("left", p.left).put("top", p.top).put("width", p.width).put("height", p.height)
                .put("visible", p.window == window && (w?.zoomed != true || p.active))) } })
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setLayout($value)", null) }
    }
    override suspend fun outputPane(pane: String, target: String) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.outputPane(${JSONObject.quote(pane)},${JSONObject.quote(target)})", null) }
    }
    override suspend fun write(bytes: ByteArray) { deliver(bytes, false) }
    override suspend fun reset(bytes: ByteArray) { deliver(bytes, true) }
    fun focus(allowed: () -> Boolean = { true }) { view.post {
        scope.launch {
            withTimeoutOrNull(15_000) { ready.await() } ?: return@launch
            if (!closed && allowed()) { view.requestFocus(); view.evaluateJavascript("window.focusTerminal && window.focusTerminal()", null) }
        }
    } }
    fun keyboard(show: Boolean, allowed: () -> Boolean = { true }) {
        if (closed) return
        val ime = view.context.getSystemService(InputMethodManager::class.java)
        if (!show) {
            view.windowInsetsController?.hide(AndroidInsets.Type.ime()) ?: ime.hideSoftInputFromWindow(view.windowToken, 0)
        }
        else scope.launch {
            // A freshly bound WebView may still be loading its textarea.
            withTimeoutOrNull(15_000) { ready.await() } ?: return@launch
            if (closed || !allowed()) return@launch
            view.requestFocus()
            view.evaluateJavascript("window.focusTerminal && window.focusTerminal()") {
                view.post { if (!closed && allowed()) {
                    view.windowInsetsController?.show(AndroidInsets.Type.ime()) ?: ime.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT)
                } }
            }
        }
    }
    fun shortcut(shortcut: TerminalShortcut) {
        val (key, own) = shortcut.key?.semanticInput() ?: return
        if (!closed && ready.isCompleted) view.evaluateJavascript("window.sendShortcut(${JSONObject.quote(key)},${own.control},${own.shift},${own.alt})", null)
    }
    suspend fun setInputEnabled(enabled: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setInputEnabled($enabled)", null) }
    }
    suspend fun setPointerEnabled(enabled: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setPointerEnabled($enabled)", null) }
    }
    suspend fun configure(preferences: Preferences) {
        withTimeout(15_000) { ready.await() }
        // A family missing on this device keeps the WebView default monospace.
        val face = try { AndroidTerminalFonts.find(preferences.terminalFontFamily) } catch (_: Exception) { null }
        withContext(Dispatchers.Main.immediate) {
            font.configure(preferences.fontSize)
            scrollback = preferences.scrollback.coerceIn(100, 10000)
            keyboardMode = preferences.effectiveAndroidKeyboardMode
            applyConfiguration()
            val current = terminalFont?.second
            if (if (face == null) current != null else !face.sameFace(current)) {
                terminalFont = face?.let { "f${fontTokens.incrementAndGet()}" to it }
                applyTerminalFont()
            }
        }
    }
    private fun applyTerminalFont() {
        if (!closed) view.evaluateJavascript("window.setTerminalFont(${terminalFont?.first?.let(JSONObject::quote) ?: "null"})", null)
    }
    private fun applyConfiguration() {
        if (!closed) view.evaluateJavascript("window.configure(${font.effective},$scrollback,${JSONObject.quote(keyboardMode)})", null)
    }
    fun finishFontGesture() {
        if (!consumingPinch) return
        consumingPinch = false; font.endGesture()
        view.parent?.requestDisallowInterceptTouchEvent(false)
        if (!closed && ready.isCompleted) view.evaluateJavascript("window.setFontGesture(false)", null)
    }
    fun resetFontSize(default: Int) {
        if (closed) return
        font.configure(default)
        font.reset()
        applyConfiguration()
    }
    suspend fun preparePaste(text: String): ByteArray = withTimeout(15_000) {
        ready.await()
        withContext(Dispatchers.Main.immediate) {
            check(!closed) { t("terminal.closed") }
            val result = CompletableDeferred<ByteArray>()
            view.evaluateJavascript("window.preparePaste(${JSONObject.quote(text)})") { raw ->
                runCatching {
                    check(raw != null && raw != "null") { t("input.too_large_with_paste") }
                    org.json.JSONTokener(raw).nextValue().let { it as String }.toByteArray(Charsets.UTF_8)
                }.fold({ result.complete(it) }, { result.completeExceptionally(it) })
            }
            result.await()
        }
    }
    /** The long-press selection's text, read when the user copies it. */
    suspend fun selectedText(): String? = withContext(Dispatchers.Main.immediate) {
        if (closed || !ready.isCompleted) return@withContext null
        val result = CompletableDeferred<String?>()
        view.evaluateJavascript("window.selectedText()") { raw ->
            result.complete(runCatching { org.json.JSONTokener(raw ?: "null").nextValue() as? String }.getOrNull())
        }
        withTimeoutOrNull(15_000) { result.await() }
    }
    /** Native closed the selection menu; the bridge does not report this clear back. */
    fun clearSelection() {
        if (!closed && ready.isCompleted) view.evaluateJavascript("window.clearSelection()", null)
    }
    suspend fun setSmoothTransitions(enabled: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setSmoothTransitions($enabled)", null) }
    }
    fun beginImeAnimation(): Long = imeAnimation.begin().also { dispatchImeAnimation(true, it) }
    fun endImeAnimation(version: Long) {
        if (imeAnimation.end(version)) dispatchImeAnimation(false, version)
    }
    fun cancelImeAnimation() {
        view.removeCallbacks(viewportFrame); viewportScheduled = false
        dispatchImeAnimation(false, imeAnimation.cancel())
        viewport.detach()
        val version = viewport.version
        if (!closed && ready.isCompleted) view.evaluateJavascript("window.detachViewport($version)", null)
    }
    fun refreshViewport() {
        viewport.attach()
        dispatchViewport()
    }
    fun setViewport(value: TerminalViewport) {
        managedViewport = true
        if (viewport.update(value)) dispatchViewport()
    }
    private fun dispatchViewport() {
        if (closed || viewportScheduled) return
        viewportScheduled = true
        view.postOnAnimation(viewportFrame)
    }
    private fun dispatchImeAnimation(active: Boolean, version: Long) {
        scope.launch {
            withTimeoutOrNull(15_000) { ready.await() } ?: return@launch
            if (!closed && version == imeAnimation.version) view.evaluateJavascript("window.setImeAnimation($active,$version)", null)
        }
    }
    override suspend fun beginSnapshot() {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.beginSnapshot()", null) }
    }
    override suspend fun connectionRecovery(active: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) {
            if (!closed) view.evaluateJavascript("window.setConnectionRecovery($active)", null)
        }
    }
    override suspend fun endSnapshot() = mutex.withLock {
        if (closed) return@withLock
        acknowledged { id -> "window.endSnapshot($id)" }
    }
    override suspend fun cancelSnapshot() {
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.cancelSnapshot && window.cancelSnapshot()", null) }
    }
    override suspend fun resizeRemote(size: Size) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { view.evaluateJavascript("window.resizePane(${size.columns},${size.rows})", null) }
    }
    private suspend fun deliver(bytes: ByteArray, reset: Boolean) = mutex.withLock {
        withTimeout(15_000) { ready.await() }
        if (closed) return@withLock
        var offset = 0
        do {
            val end = minOf(offset + 16384, bytes.size)
            val encoded = Base64.getEncoder().encodeToString(bytes.copyOfRange(offset, end))
            acknowledged { id -> "window.receive('$encoded',${reset && offset == 0},$id)" }
            offset = end
        } while (offset < bytes.size)
    }
    private suspend fun acknowledged(script: (Long) -> String) {
        val ack = CompletableDeferred<Unit>()
        val id = ++serial
        try {
            withContext(Dispatchers.Main.immediate) {
                awaiting = id to ack
                view.evaluateJavascript(script(id), null)
            }
            withTimeout(15_000) { ack.await() }
        } finally {
            withContext(NonCancellable + Dispatchers.Main.immediate) {
                // A delayed bridge callback cannot complete a replacement write.
                if (awaiting?.first == id) awaiting = null
            }
        }
    }
    override fun close() {
        view.removeCallbacks(viewportFrame); viewportScheduled = false
        finishFontGesture()
        closed = true; ready.cancel(); awaiting?.second?.cancel()
        scope.launch {
            if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) WebViewCompat.removeWebMessageListener(view, "dgl")
            view.stopLoading(); view.destroy(); scope.cancel()
        }
    }
    companion object {
        private val assets = setOf("/assets/terminal/index.html", "/assets/terminal/terminal.js", "/assets/terminal/xterm.js", "/assets/terminal/xterm.css", "/assets/terminal/style.css")
        /** Process-wide, so the shared WebView cache never pairs a token with another face. */
        private val fontTokens = java.util.concurrent.atomic.AtomicLong()
        private fun blocked() = WebResourceResponse("text/plain", "UTF-8", 403, "Blocked", emptyMap(), ByteArrayInputStream(byteArrayOf()))
    }
}

@Composable actual fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit) {
    val context = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri != null) try {
            context.contentResolver.openInputStream(uri)?.use {
                val bytes = it.readNBytes(131073)
                require(bytes.size in 32..131072)
                onKey(bytes.toString(Charsets.UTF_8))
            } ?: onError(t("terminal.file_open_failed"))
        } catch (_: Exception) { onError(t("host.key_file_read_failed_size")) }
    }
    BorderedButton({ launcher.launch(arrayOf("*/*")) }) { Text(t("host.import_private_key_file")) }
}
@Composable actual fun rememberPublicKeyCopier(): (String) -> Unit {
    val context = LocalContext.current.applicationContext
    return remember(context) { { value: String ->
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("SSH public key", value))
    } }
}
