package kim.jujin.rovessh.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import kim.jujin.rovessh.*
import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.KeyGeneration
import java.util.UUID

@Composable expect fun PlatformTerminal(client: ClientModel, preferences: Preferences, modifier: Modifier)
@Composable expect fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit)
expect fun copyPublicKey(value: String)

@Composable fun App(model: AppModel) {
    val local by model.local.collectAsState()
    val connections by model.connections.collectAsState()
    val selected by model.selected.collectAsState()
    val dark = when (local.preferences.theme) { "dark" -> true; "light" -> false; else -> isSystemInDarkTheme() }
    var edit by remember { mutableStateOf<HostProfile?>(null) }
    var adding by remember { mutableStateOf(false) }
    var settings by remember { mutableStateOf(false) }
    var explorer by remember { mutableStateOf(false) }
    var deletion by remember { mutableStateOf<HostProfile?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    MaterialTheme(colorScheme = if (dark) darkColorScheme(primary = Color(0xff8acdd0)) else lightColorScheme(primary = Color(0xff00696e))) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding()) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("dgl", style = MaterialTheme.typography.titleLarge)
                    Spacer(Modifier.width(8.dp))
                    Text("SSH · tmux", style = MaterialTheme.typography.labelMedium, modifier = Modifier.weight(1f))
                    TextButton({ explorer = !explorer }) { Text(if (explorer) "터미널" else "호스트") }
                    TextButton({ settings = true }) { Text("설정") }
                }
                if (connections.isNotEmpty()) Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
                    connections.forEach { connection ->
                        FilterChip(selected == connection, { model.selected.value = connection; explorer = false }, label = { Text(connection.host.alias) })
                        TextButton({ model.removeConnection(connection) }) { Text("×") }
                    }
                }
                BoxWithConstraints(Modifier.weight(1f)) {
                    val wide = maxWidth >= 840.dp
                    Row(Modifier.fillMaxSize()) {
                        if (wide || selected == null || explorer) {
                            Column(if (wide) Modifier.width(272.dp).fillMaxHeight() else Modifier.fillMaxSize()) {
                                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                                    Text("연결", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                                    TextButton({ adding = true }) { Text("+ 호스트") }
                                }
                                LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                                    if (local.hosts.isEmpty()) item { Text("SSH 호스트를 추가하여 시작하세요.\n원격 서버에는 SSH와 tmux만 필요합니다.", Modifier.padding(12.dp)) }
                                    items(local.hosts.sortedWith(compareByDescending<HostProfile> { it.favorite }.thenByDescending { it.lastConnected }), key = { it.id }) { host ->
                                        OutlinedCard(Modifier.fillMaxWidth()) {
                                            Column(Modifier.padding(12.dp)) {
                                                Text((if (host.favorite) "★ " else "") + host.alias, style = MaterialTheme.typography.titleMedium)
                                                Text("${host.username}@${host.hostname}:${host.port}", style = MaterialTheme.typography.bodySmall)
                                                if (host.lastConnected != 0L) Text("최근 연결: " + java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT).format(java.util.Date(host.lastConnected)), style = MaterialTheme.typography.labelSmall)
                                                Row {
                                                    TextButton({ model.open(host); explorer = false }) { Text("연결") }
                                                    TextButton({ edit = host }) { Text("편집") }
                                                    TextButton({ deletion = host }) { Text("삭제") }
                                                }
                                            }
                                        }
                                    }
                                }
                                Text(model.storageDescription, Modifier.padding(12.dp), style = MaterialTheme.typography.labelSmall)
                            }
                        }
                        if (wide || (selected != null && !explorer)) {
                            Box(Modifier.weight(1f).fillMaxHeight()) {
                                if (selected != null) key(selected) { ConnectionScreen(selected!!, local.preferences) }
                                else Column(Modifier.align(Alignment.Center).padding(24.dp)) {
                                    Text("Keep the CLI.", style = MaterialTheme.typography.headlineMedium)
                                    Text("세션은 서버에, 작업은 그대로.", style = MaterialTheme.typography.bodyLarge)
                                    Spacer(Modifier.height(12.dp))
                                    Text("호스트에 연결하고 기존 tmux 세션을 선택하세요.")
                                }
                            }
                        }
                    }
                }
            }
        }
        if (adding || edit != null) HostEditor(model, edit, { adding = false; edit = null })
        if (settings) SettingsDialog(model) { settings = false }
        deletion?.let { host -> ConfirmDialog("호스트 삭제", "${host.alias}의 로컬 설정과 저장된 자격 증명을 삭제합니다.", {
            try {
                model.credentials.delete(host.credentialRef)
                model.update(local.copy(hosts = local.hosts.filter { it.id != host.id }))
                deletion = null
            } catch (_: Exception) { error = "자격 증명을 삭제하지 못했습니다." }
        }, { deletion = null }) }
        error?.let { message -> AlertDialog(onDismissRequest = { error = null }, title = { Text("오류") }, text = { Text(message) }, confirmButton = { TextButton({ error = null }) { Text("닫기") } }) }
    }
}

@Composable private fun ConnectionScreen(client: ClientModel, preferences: Preferences) {
    val status by client.status.collectAsState()
    val remote by client.remote.collectAsState()
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val trust by client.trustRequest.collectAsState()
    val notice by client.notice.collectAsState()
    var navigation by remember { mutableStateOf(true) }
    var namePrompt by remember { mutableStateOf<Pair<String, (String) -> Unit>?>(null) }
    var deletion by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val currentPane = remote.panes.firstOrNull { it.id == pane }
    val currentWindow = remote.windows.firstOrNull { it.id == currentPane?.window && it.session == session }
    val currentSession = remote.sessions.firstOrNull { it.id == session }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(status.message, style = MaterialTheme.typography.labelSmall, modifier = Modifier.weight(1f), maxLines = 2)
            TextButton({ navigation = !navigation }) { Text("탐색") }
            if (status.phase == Phase.Disconnected || status.phase == Phase.Failed || status.phase == Phase.MissingSession) TextButton({ client.connect() }) { Text("재연결") }
            else TextButton({ client.disconnect() }) { Text("연결 종료") }
        }
        Box(Modifier.fillMaxWidth().height(4.dp)) {
            if (status.phase == Phase.Connecting || status.phase == Phase.Restoring || status.phase == Phase.Reconnecting) LinearProgressIndicator(Modifier.fillMaxWidth())
        }
        if (navigation && status.phase != Phase.Shell) {
            Column(Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
                Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    remote.sessions.forEach { s -> FilterChip(session == s.id, { client.selectSession(s.id) }, label = { Text("${s.name} · ${s.attached}") }) }
                    TextButton({ namePrompt = "세션 만들기" to client::createSession }, enabled = status.phase in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)) { Text("+ 세션") }
                    TextButton({ client.refresh() }) { Text("새로고침") }
                }
                if (session != null) Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    remote.windows.filter { it.session == session }.forEach { window ->
                        FilterChip(currentWindow?.id == window.id, {
                            remote.panes.firstOrNull { it.window == window.id && it.active }?.let { client.selectPane(it.id) }
                        }, label = { Text(window.name) })
                    }
                    TextButton({ namePrompt = "창 만들기" to { client.manage(TmuxCommand.newWindow(session!!, it)) } }) { Text("+ 창") }
                }
                if (currentWindow != null) Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    remote.panes.filter { it.window == currentWindow.id }.forEach { p ->
                        FilterChip(pane == p.id, { client.selectPane(p.id) }, label = { Text("${p.id} ${p.command} ${p.width}×${p.height}" + if (p.mode.isNotEmpty()) " · ${p.mode}" else "") })
                    }
                }
                // Keep the layout stable while a resize snapshot is restoring. Hiding this
                // row during Restoring changes the viewport and starts another resize.
                if (pane != null) Row(Modifier.horizontalScroll(rememberScrollState())) {
                    TextButton({ client.manage(TmuxCommand.split(pane!!, true)) }, enabled = status.phase == Phase.Ready) { Text("좌우 분할") }
                    TextButton({ client.manage(TmuxCommand.split(pane!!, false)) }, enabled = status.phase == Phase.Ready) { Text("상하 분할") }
                    listOf("←" to 'L', "→" to 'R', "↑" to 'U', "↓" to 'D').forEach { (label, direction) -> TextButton({ client.manage(TmuxCommand.resizePane(pane!!, direction)) }, enabled = status.phase == Phase.Ready) { Text("크기 $label") } }
                    TextButton({ namePrompt = "세션 이름 변경" to { client.manage(TmuxCommand.renameSession(session!!, it)) } }, enabled = status.phase == Phase.Ready) { Text("세션 이름") }
                    TextButton({ namePrompt = "창 이름 변경" to { client.manage(TmuxCommand.renameWindow(currentWindow!!.id, it)) } }, enabled = status.phase == Phase.Ready) { Text("창 이름") }
                    TextButton({ val target = pane!!; deletion = "패널 $target (${currentPane?.command})" to { client.manage(TmuxCommand.killPane(target)) } }, enabled = status.phase == Phase.Ready) { Text("패널 삭제") }
                    TextButton({ val target = currentWindow!!.id; deletion = "창 ${currentWindow.name}" to { client.manage(TmuxCommand.killWindow(target)) } }, enabled = status.phase == Phase.Ready) { Text("창 삭제") }
                    TextButton({ val target = session!!; deletion = "세션 ${currentSession?.name}" to { client.manage(TmuxCommand.killSession(target)) } }, enabled = status.phase == Phase.Ready) { Text("세션 삭제") }
                }
            }
        }
        if (currentPane?.mode?.isNotBlank() == true) Text("tmux ${currentPane.mode} · 아래 터미널은 일반 패널 출력입니다. 로컬 스크롤·선택을 사용할 수 있습니다.", Modifier.padding(8.dp), style = MaterialTheme.typography.labelSmall)
        Box(Modifier.weight(1f).fillMaxWidth().background(Color(0xff11151b))) {
            PlatformTerminal(client, preferences, Modifier.fillMaxSize())
        }
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            listOf("Esc" to "\u001b", "Tab" to "\t", "Ctrl+C" to "\u0003", "Ctrl+D" to "\u0004", "Ctrl+L" to "\u000c", "↑" to "\u001b[A", "↓" to "\u001b[B", "←" to "\u001b[D", "→" to "\u001b[C", "Home" to "\u001b[H", "End" to "\u001b[F").forEach { (label, bytes) ->
                TextButton({ client.input(bytes.encodeToByteArray()) }, enabled = status.acceptsInput, contentPadding = PaddingValues(horizontal = 10.dp)) { Text(label) }
            }
        }
    }
    trust?.let { request ->
        AlertDialog(onDismissRequest = { client.trustRequest.value = null; client.disconnect() }, title = { Text("처음 연결하는 서버") },
            text = { Column { Text(request.endpoint); Spacer(Modifier.height(8.dp)); SelectionContainer { Text(request.fingerprint) }; Spacer(Modifier.height(8.dp)); Text("서버 관리자에게 받은 SHA-256 지문과 일치하는지 확인하세요.") } },
            confirmButton = { TextButton({ client.trust() }) { Text("신뢰하고 연결") } }, dismissButton = { TextButton({ client.trustRequest.value = null; client.disconnect() }) { Text("취소") } })
    }
    namePrompt?.let { (title, action) -> NameDialog(title, { action(it); namePrompt = null }, { namePrompt = null }) }
    deletion?.let { (target, action) -> ConfirmDialog("$target 삭제", "이 대상의 원격 프로세스가 종료됩니다. 삭제할까요?", { action(); deletion = null }, { deletion = null }) }
    notice?.let { message -> AlertDialog(onDismissRequest = { client.notice.value = null }, text = { Text(message) }, confirmButton = { TextButton({ client.notice.value = null }) { Text("닫기") } }) }
}

@Composable private fun HostEditor(model: AppModel, original: HostProfile?, dismiss: () -> Unit) {
    var alias by remember { mutableStateOf(original?.alias ?: "") }
    var hostname by remember { mutableStateOf(original?.hostname ?: "") }
    var port by remember { mutableStateOf((original?.port ?: 22).toString()) }
    var username by remember { mutableStateOf(original?.username ?: "") }
    var auth by remember { mutableStateOf(original?.auth ?: AuthKind.Password) }
    var password by remember { mutableStateOf("") }
    var privateKey by remember { mutableStateOf("") }
    var passphrase by remember { mutableStateOf("") }
    var favorite by remember { mutableStateOf(original?.favorite ?: false) }
    var generatedPublic by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    AlertDialog(onDismissRequest = dismiss, title = { Text(if (original == null) "SSH 호스트 추가" else "호스트 편집") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(alias, { alias = it }, label = { Text("별칭") }, singleLine = true)
            OutlinedTextField(hostname, { hostname = it }, label = { Text("호스트 / IP") }, singleLine = true)
            Row {
                OutlinedTextField(port, { port = it }, label = { Text("포트") }, singleLine = true, modifier = Modifier.width(90.dp))
                Spacer(Modifier.width(8.dp))
                OutlinedTextField(username, { username = it }, label = { Text("사용자 이름") }, singleLine = true)
            }
            Row(verticalAlignment = Alignment.CenterVertically) { Checkbox(favorite, { favorite = it }); Text("즐겨찾기") }
            Row { FilterChip(auth == AuthKind.Password, { auth = AuthKind.Password }, label = { Text("비밀번호") }); Spacer(Modifier.width(8.dp)); FilterChip(auth == AuthKind.PrivateKey, { auth = AuthKind.PrivateKey }, label = { Text("개인키") }) }
            if (auth == AuthKind.Password) OutlinedTextField(password, { password = it }, label = { Text(if (original == null) "비밀번호" else "새 비밀번호 (비우면 기존 유지)") }, visualTransformation = PasswordVisualTransformation(), singleLine = true)
            else {
                ImportKeyButton({ privateKey = it; generatedPublic = null }, { error = it })
                Text(if (privateKey.isEmpty()) "Ed25519 / RSA · 암호화 키 지원" else "개인키를 가져왔습니다.", style = MaterialTheme.typography.labelSmall)
                OutlinedTextField(passphrase, { passphrase = it }, label = { Text("키 암호") }, visualTransformation = PasswordVisualTransformation(), singleLine = true)
                TextButton({ try { val generated = KeyGeneration.ed25519(); privateKey = generated.credential.privateKey; passphrase = ""; generatedPublic = generated.publicKey } catch (_: Exception) { error = "키 생성 실패" } }) { Text("Ed25519 키 생성") }
                generatedPublic?.let { key -> Text("공개키를 서버의 authorized_keys에 등록하세요."); TextButton({ copyPublicKey(key) }) { Text("공개키 복사") } }
            }
            Text(model.storageDescription, style = MaterialTheme.typography.labelSmall)
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }, confirmButton = {
        TextButton({
            try {
                val id = original?.id ?: UUID.randomUUID().toString()
                val profile = HostProfile(id, alias.trim(), hostname.trim(), port.toInt(), username.trim(), auth, original?.credentialRef ?: id, favorite, original?.lastConnected ?: 0, original?.lastSession, original?.lastPane)
                val changed = (auth == AuthKind.Password && password.isNotEmpty()) || (auth == AuthKind.PrivateKey && privateKey.isNotEmpty())
                if (original == null || original.auth != auth) require(changed)
                if (changed) model.credentials.write(profile.credentialRef, Credential(password, privateKey, passphrase))
                model.update(model.local.value.copy(hosts = model.local.value.hosts.filter { it.id != id } + profile))
                dismiss()
            } catch (_: IllegalArgumentException) { error = "필수 입력과 포트(1–65535), 자격 증명을 확인하세요." }
            catch (_: Exception) { error = "보호 저장소에 저장하지 못했습니다. 저장을 중단했습니다." }
        }) { Text("저장") }
    }, dismissButton = { TextButton(dismiss) { Text("취소") } })
}

@Composable private fun SettingsDialog(model: AppModel, dismiss: () -> Unit) {
    val local by model.local.collectAsState()
    var forget by remember { mutableStateOf<String?>(null) }
    AlertDialog(onDismissRequest = dismiss, title = { Text("설정") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("테마", style = MaterialTheme.typography.titleSmall)
            Row { listOf("system" to "시스템", "light" to "밝게", "dark" to "어둡게").forEach { (value, label) -> FilterChip(local.preferences.theme == value, { model.update(local.copy(preferences = local.preferences.copy(theme = value))) }, label = { Text(label) }) } }
            Text("터미널 글자 크기: ${local.preferences.fontSize}")
            Row {
                TextButton({ model.update(local.copy(preferences = local.preferences.copy(fontSize = (local.preferences.fontSize - 1).coerceAtLeast(10)))) }) { Text("작게") }
                TextButton({ model.update(local.copy(preferences = local.preferences.copy(fontSize = (local.preferences.fontSize + 1).coerceAtMost(28)))) }) { Text("크게") }
            }
            Text("스크롤백: ${local.preferences.scrollback}줄 · 터미널에서 스크롤·선택·복사")
            Text("신뢰한 서버", style = MaterialTheme.typography.titleSmall)
            local.trust.forEach { (endpoint, fingerprint) ->
                Column { Text(endpoint); SelectionContainer { Text(fingerprint, style = MaterialTheme.typography.labelSmall) }; TextButton({ forget = endpoint }) { Text("신뢰 키 삭제") } }
            }
            Text(model.storageDescription)
        }
    }, confirmButton = { TextButton(dismiss) { Text("닫기") } })
    forget?.let { endpoint -> ConfirmDialog("신뢰 키 삭제", "$endpoint 키를 삭제합니다. 다음 연결에서 지문을 다시 확인해야 합니다.", { model.update(local.copy(trust = local.trust - endpoint)); forget = null }, { forget = null }) }
}

@Composable private fun NameDialog(title: String, accept: (String) -> Unit, dismiss: () -> Unit) {
    var value by remember { mutableStateOf("") }
    AlertDialog(onDismissRequest = dismiss, title = { Text(title) }, text = { OutlinedTextField(value, { value = it.take(128) }, label = { Text("이름") }, singleLine = true) },
        confirmButton = { TextButton({ accept(value) }, enabled = value.isNotBlank() && value.none { it == '.' || it == ':' || it.code < 32 }) { Text("확인") } }, dismissButton = { TextButton(dismiss) { Text("취소") } })
}
@Composable private fun ConfirmDialog(title: String, message: String, accept: () -> Unit, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(title) }, text = { Text(message) }, confirmButton = { TextButton(accept) { Text("삭제") } }, dismissButton = { TextButton(dismiss) { Text("취소") } })
}
