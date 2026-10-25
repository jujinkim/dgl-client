package kim.jujin.rovessh.ui

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.view.inputmethod.InputMethodManager
import android.view.WindowInsets as AndroidInsets
import android.webkit.*
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.util.Base64
import java.lang.ref.WeakReference

internal actual fun mobileInputDialogProperties() = androidx.compose.ui.window.DialogProperties(
    usePlatformDefaultWidth = false, decorFitsSystemWindows = false)

@Composable actual fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier) {
    val context = LocalContext.current
    val composeView = LocalView.current
    val focus = LocalTerminalFocus.current
    val terminal = remember(client, preferences.fontSize, preferences.scrollback) {
        AndroidTerminal(context, preferences, { if (focus.inputAllowed()) client.input(it) }, client::resize,
            { pane, target, bytes -> if (focus.inputAllowed()) client.paneInput(pane, target, bytes) }, client::focusPane, client::resizePane)
    }
    val allowed = focus.inputAllowed()
    LaunchedEffect(terminal, allowed) { terminal.setInputEnabled(allowed) }
    LaunchedEffect(terminal, preferences.smoothTerminalTransitions) {
        terminal.setSmoothTransitions(preferences.smoothTerminalTransitions)
    }
    DisposableEffect(terminal, focus, composeView) {
        // Read the same native insets used at click time. Compose's target IME visibility
        // can lag a cancelled show/hide animation while the WebView owns input focus.
        var observing = true
        val updateIme = { if (observing) {
            val next = terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) == true
            focus.imeVisible = next
        } }
        val observer = android.view.ViewTreeObserver.OnPreDrawListener { updateIme(); true }
        val treeObserver = composeView.viewTreeObserver
        treeObserver.addOnPreDrawListener(observer)
        terminal.view.setOnApplyWindowInsetsListener { view, insets ->
            view.post { updateIme() }
            view.onApplyWindowInsets(insets)
        }
        terminal.view.setWindowInsetsAnimationCallback(object : android.view.WindowInsetsAnimation.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
            override fun onProgress(insets: AndroidInsets, animations: MutableList<android.view.WindowInsetsAnimation>): AndroidInsets {
                updateIme(); return insets
            }
            override fun onEnd(animation: android.view.WindowInsetsAnimation) { terminal.view.post { updateIme() } }
        })
        terminal.view.post { updateIme() }
        focus.request = { terminal.focus() }
        focus.keyboard = { terminal.keyboard(terminal.view.rootWindowInsets?.isVisible(AndroidInsets.Type.ime()) != true, focus.inputAllowed) }
        focus.preparePaste = terminal::preparePaste
        onDispose {
            observing = false
            if (treeObserver.isAlive) treeObserver.removeOnPreDrawListener(observer)
            terminal.view.setOnApplyWindowInsetsListener(null)
            terminal.view.setWindowInsetsAnimationCallback(null)
            focus.request = {}; focus.keyboard = {}; focus.imeVisible = false
            focus.preparePaste = { _, _ -> error(t("터미널이 닫혔습니다.")) }
        }
    }
    DisposableEffect(terminal) {
        client.bindSurface(terminal)
        onDispose { client.unbindSurface(terminal); terminal.close() }
    }
    key(terminal) { AndroidView(factory = { terminal.view }, modifier = modifier) }
}

@SuppressLint("SetJavaScriptEnabled")
class AndroidTerminal(context: Context, preferences: Preferences, private val input: (ByteArray) -> Unit, private val resize: (Size) -> Unit,
    private val paneInput: ((String, String?, ByteArray) -> Unit)? = null,
    private val focusPane: (String, String?) -> Unit = { _, _ -> },
    private val resizePane: (String, String?, Size) -> Unit = { _, _, _ -> }) : TerminalSurface {
    val view = WebView(context)
    private val ready = CompletableDeferred<Unit>()
    private val mutex = Mutex()
    private var serial = 0L
    private var awaiting: Pair<Long, CompletableDeferred<Unit>>? = null
    private var closed = false
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    init {
        val loader = WebViewAssetLoader.Builder().addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(context)).build()
        with(view.settings) {
            javaScriptEnabled = true
            allowFileAccess = false; allowContentAccess = false
            domStorageEnabled = false; databaseEnabled = false
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            blockNetworkLoads = true
            safeBrowsingEnabled = true
            mediaPlaybackRequiresUserGesture = true
        }
        view.isFocusableInTouchMode = true
        view.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?) = true
            override fun shouldInterceptRequest(view: WebView?, request: WebResourceRequest): WebResourceResponse {
                val uri = request.url
                return if (uri.scheme == "https" && uri.host == "appassets.androidplatform.net" && uri.path in assets) {
                    loader.shouldInterceptRequest(uri) ?: blocked()
                } else blocked()
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
                        if (paneInput != null) paneInput.invoke(json.getString("pane"), target(), bytes) else input(bytes)
                    }
                }
                when (json.getString("type")) {
                    "ready" -> {
                        view.evaluateJavascript("window.configure(${preferences.fontSize.coerceIn(10, 28)},${preferences.scrollback.coerceIn(100, 10000)})", null)
                        ready.complete(Unit)
                    }
                    "input" -> {
                        val bytes = json.getString("data").toByteArray(Charsets.UTF_8)
                        inputForPane(bytes)
                    }
                    "input-binary" -> inputForPane(kim.jujin.rovessh.core.TerminalInput.decodeBinary(json.getString("data")))
                    "focus" -> focusPane(json.getString("pane"), target())
                    "resize-pane" -> resizePane(json.getString("pane"), target(), Size(json.getInt("columns"), json.getInt("rows")))
                    "resize" -> {
                        val columns = json.getInt("columns"); val rows = json.getInt("rows")
                        if (columns in 2..500 && rows in 2..250) resize(Size(columns, rows))
                    }
                    "ack" -> if (json.getLong("id") == awaiting?.first) awaiting?.second?.complete(Unit)
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
            .put("inputTarget", inputTarget ?: JSONObject.NULL).put("columns", w?.width ?: 80).put("rows", w?.height ?: 24)
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
    fun focus() { view.post {
        if (!closed) { view.requestFocus(); view.evaluateJavascript("window.focusTerminal && window.focusTerminal()", null) }
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
    suspend fun setInputEnabled(enabled: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setInputEnabled($enabled)", null) }
    }
    suspend fun preparePaste(text: String, enter: Boolean): ByteArray = withTimeout(15_000) {
        ready.await()
        withContext(Dispatchers.Main.immediate) {
            check(!closed) { t("터미널이 닫혔습니다.") }
            val result = CompletableDeferred<ByteArray>()
            view.evaluateJavascript("window.preparePaste(${JSONObject.quote(text)},$enter)") { raw ->
                runCatching {
                    check(raw != null && raw != "null") { t("입력은 붙여넣기 제어 문자와 Enter를 포함하여 UTF-8 64 KiB 이하여야 합니다.") }
                    org.json.JSONTokener(raw).nextValue().let { it as String }.toByteArray(Charsets.UTF_8)
                }.fold({ result.complete(it) }, { result.completeExceptionally(it) })
            }
            result.await()
        }
    }
    suspend fun setSmoothTransitions(enabled: Boolean) {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.setSmoothTransitions($enabled)", null) }
    }
    override suspend fun beginSnapshot() {
        withTimeout(15_000) { ready.await() }
        withContext(Dispatchers.Main.immediate) { if (!closed) view.evaluateJavascript("window.beginSnapshot()", null) }
    }
    override suspend fun endSnapshot() = mutex.withLock {
        if (closed) return@withLock
        val ack = CompletableDeferred<Unit>()
        val id = ++serial
        withContext(Dispatchers.Main.immediate) {
            awaiting = id to ack
            view.evaluateJavascript("window.endSnapshot($id)", null)
        }
        withTimeout(15_000) { ack.await() }
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
            val ack = CompletableDeferred<Unit>()
            val id = ++serial
            withContext(Dispatchers.Main.immediate) {
                awaiting = id to ack
                view.evaluateJavascript("window.receive('$encoded',${reset && offset == 0},$id)", null)
            }
            withTimeout(15_000) { ack.await() }
            offset = end
        } while (offset < bytes.size)
    }
    override fun close() {
        closed = true; ready.cancel(); awaiting?.second?.cancel()
        scope.launch {
            if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) WebViewCompat.removeWebMessageListener(view, "dgl")
            view.stopLoading(); view.destroy(); scope.cancel()
        }
    }
    companion object {
        private val assets = setOf("/assets/terminal/index.html", "/assets/terminal/terminal.js", "/assets/terminal/xterm.js", "/assets/terminal/xterm.css", "/assets/terminal/addon-fit.js", "/assets/terminal/style.css")
        private fun blocked() = WebResourceResponse("text/plain", "UTF-8", 403, "Blocked", emptyMap(), ByteArrayInputStream(byteArrayOf()))
    }
}

private var clipboardContext: WeakReference<Context>? = null
@Composable actual fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit) {
    val context = LocalContext.current
    SideEffect { clipboardContext = WeakReference(context.applicationContext) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri != null) try {
            context.contentResolver.openInputStream(uri)?.use {
                val bytes = it.readNBytes(131073)
                require(bytes.size in 32..131072)
                onKey(bytes.toString(Charsets.UTF_8))
            } ?: onError(t("파일을 열 수 없습니다."))
        } catch (_: Exception) { onError(t("개인키를 읽지 못했습니다. 최대 크기는 128 KiB입니다.")) }
    }
    TextButton({ launcher.launch(arrayOf("*/*")) }) { Text(t("개인키 파일 가져오기")) }
}
actual fun copyPublicKey(value: String) {
    val context = clipboardContext?.get() ?: return
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("SSH public key", value))
}
