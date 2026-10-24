package kim.jujin.rovessh.ui

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.webkit.*
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
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

@Composable actual fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier) {
    val context = LocalContext.current
    val terminal = remember(client, preferences.fontSize, preferences.scrollback) { AndroidTerminal(context, preferences, client::input, client::resize) }
    val focus = LocalTerminalFocus.current
    DisposableEffect(terminal, focus) {
        focus.request = { terminal.view.post {
            terminal.view.requestFocus()
            terminal.view.evaluateJavascript("window.focusTerminal && window.focusTerminal()", null)
        }; Unit }
        onDispose { focus.request = {} }
    }
    DisposableEffect(terminal) {
        client.bindSurface(terminal)
        onDispose { client.unbindSurface(terminal); terminal.close() }
    }
    key(terminal) { AndroidView(factory = { terminal.view }, modifier = modifier) }
}

@SuppressLint("SetJavaScriptEnabled")
class AndroidTerminal(context: Context, preferences: Preferences, private val input: (ByteArray) -> Unit, private val resize: (Size) -> Unit) : TerminalSurface {
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
                when (json.getString("type")) {
                    "ready" -> {
                        view.evaluateJavascript("window.configure(${preferences.fontSize.coerceIn(10, 28)},${preferences.scrollback.coerceIn(100, 10000)})", null)
                        ready.complete(Unit)
                    }
                    "input" -> {
                        val bytes = json.getString("data").toByteArray(Charsets.UTF_8)
                        if (bytes.size <= 65536) input(bytes)
                    }
                    "input-binary" -> input(kim.jujin.rovessh.core.TerminalInput.decodeBinary(json.getString("data")))
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
    override suspend fun write(bytes: ByteArray) { deliver(bytes, false) }
    override suspend fun reset(bytes: ByteArray) { deliver(bytes, true) }
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
            } ?: onError("파일을 열 수 없습니다.")
        } catch (_: Exception) { onError("개인키를 읽지 못했습니다. 최대 크기는 128 KiB입니다.") }
    }
    TextButton({ launcher.launch(arrayOf("*/*")) }) { Text("개인키 파일 가져오기") }
}
actual fun copyPublicKey(value: String) {
    val context = clipboardContext?.get() ?: return
    context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("SSH public key", value))
}
