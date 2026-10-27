package kim.jujin.dglc.ui

import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.SwingPanel
import com.jediterm.core.util.TermSize
import com.jediterm.terminal.TtyConnector
import com.jediterm.terminal.TerminalColor
import com.jediterm.terminal.RequestOrigin
import com.jediterm.terminal.ui.JediTermWidget
import com.jediterm.terminal.ui.settings.DefaultSettingsProvider
import kim.jujin.dglc.ClientConnection
import kim.jujin.dglc.core.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runInterruptible
import kotlinx.coroutines.withContext
import java.awt.FileDialog
import java.awt.Frame
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import java.io.*
import javax.swing.SwingUtilities

internal actual fun mobileInputDialogProperties() = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false)
internal actual fun mobileNotificationPopupProperties() = androidx.compose.ui.window.PopupProperties(focusable = true)

@Composable internal actual fun rememberAppFocusCapture(): () -> (() -> Unit) = { {} }

@Composable actual fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier) {
    val connector = remember(client, preferences.fontSize, preferences.scrollback) { DesktopConnector(client::input, client::resize) }
    val widget = remember(connector) {
        JediTermWidget(object : DefaultSettingsProvider() {
            override fun getTerminalFontSize() = preferences.fontSize.toFloat()
            override fun getBufferMaxLinesCount() = preferences.scrollback.coerceIn(100, 10000)
            override fun getDefaultForeground() = TerminalColor(220, 225, 232)
            override fun getDefaultBackground() = TerminalColor(17, 21, 27)
        }).apply {
            ttyConnector = connector
            connector.remoteResize = { size -> terminal.resize(TermSize(size.columns, size.rows), RequestOrigin.Remote) }
            start()
        }
    }
    DisposableEffect(connector) {
        client.bindSurface(connector.surface)
        onDispose { client.unbindSurface(connector.surface); connector.close(); SwingUtilities.invokeLater { widget.close() } }
    }
    val focus = LocalTerminalFocus.current
    DisposableEffect(widget, focus) {
        focus.request = { SwingUtilities.invokeLater { widget.requestFocusInWindow() } }
        onDispose { focus.request = {} }
    }
    key(connector) { SwingPanel(factory = { widget }, modifier = modifier) }
}

/** UTF-8 decoder spans all chunks; the 256 KiB pipe applies backpressure to the SSH reader. */
class DesktopConnector(private val input: (ByteArray) -> Unit, private val resized: (Size) -> Unit) : TtyConnector {
    private val pipe = PipedInputStream(256 * 1024)
    private val output = PipedOutputStream(pipe)
    private val reader = InputStreamReader(pipe, Charsets.UTF_8)
    @Volatile private var open = true
    var remoteResize: (Size) -> Unit = {}
    override fun read(buf: CharArray, offset: Int, length: Int) = reader.read(buf, offset, length)
    override fun write(bytes: ByteArray) { input(bytes) }
    override fun write(string: String) { input(string.toByteArray(Charsets.UTF_8)) }
    override fun isConnected() = open
    override fun ready() = reader.ready()
    override fun getName() = "DaGaLi terminal"
    override fun waitFor(): Int { while (open) Thread.sleep(50); return 0 }
    override fun resize(termSize: TermSize) { if (termSize.columns in 2..500 && termSize.rows in 2..250) resized(Size(termSize.columns, termSize.rows)) }
    // TtyConnector.write is terminal input. Surface output is a separate adapter below.
    suspend fun append(bytes: ByteArray) = runInterruptible(Dispatchers.IO) { if (open) { output.write(bytes); output.flush() } }
    val surface = object : TerminalSurface {
        override suspend fun write(bytes: ByteArray) { append(bytes) }
        override suspend fun reset(bytes: ByteArray) { append(bytes) }
        override suspend fun resizeRemote(size: Size) { withContext(Dispatchers.Main) { remoteResize(size) } }
        override fun close() { this@DesktopConnector.close() }
    }
    override fun close() { open = false; runCatching { output.close() }; runCatching { pipe.close() } }
}

@Composable actual fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit) {
    TextButton({
        val dialog = FileDialog(null as Frame?, "SSH 개인키 가져오기", FileDialog.LOAD)
        dialog.isVisible = true
        val selected = dialog.file
        if (selected != null) try {
            val file = File(dialog.directory, selected)
            require(file.length() in 32..131072)
            onKey(file.readText(Charsets.UTF_8))
        } catch (_: Exception) { onError("개인키 파일을 읽지 못했습니다. 최대 크기는 128 KiB입니다.") }
        dialog.dispose()
    }) { Text("개인키 파일 가져오기") }
}

actual fun copyPublicKey(value: String) { Toolkit.getDefaultToolkit().systemClipboard.setContents(StringSelection(value), null) }
@Composable actual fun PlatformInputRegistration(client: ClientConnection, preferences: Preferences, allowed: () -> Boolean) {}
@Composable actual fun PlatformConnectionSettings(model: kim.jujin.dglc.AppModel) {}
@Composable actual fun PlatformInputSettings(preferences: Preferences, update: (Preferences) -> Unit) {}
