package kim.jujin.rovessh

import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kim.jujin.rovessh.storage.*
import kim.jujin.rovessh.ui.App
import java.io.File

fun main() = application {
    val model = remember {
        val windows = System.getProperty("os.name").startsWith("Windows")
        val directory = if (windows) File(System.getenv("LOCALAPPDATA") ?: error("LOCALAPPDATA unavailable"), "dgl-client")
            else File(System.getProperty("user.home"), ".local/share/dgl-client")
        AppModel(LocalRepository(directory), if (windows) WindowsCredentialStore(File(directory, "credentials")) else MemoryCredentialStore(),
            if (windows) "자격 증명: Windows 사용자 DPAPI" else "개발용 실행: 자격 증명은 메모리에만 보관하며 앱 종료 시 사라집니다.")
    }
    DisposableEffect(Unit) { onDispose { model.close() } }
    Window(onCloseRequest = { model.close(); exitApplication() }, title = "dgl client", state = rememberWindowState(width = 1200.dp, height = 800.dp)) { App(model) }
}
