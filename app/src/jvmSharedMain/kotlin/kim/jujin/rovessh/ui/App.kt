package kim.jujin.rovessh.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kim.jujin.rovessh.*
import kim.jujin.rovessh.core.*
import java.util.UUID

@Composable expect fun PlatformTerminal(client: ClientConnection, preferences: Preferences, modifier: Modifier)
@Composable expect fun ImportKeyButton(onKey: (String) -> Unit, onError: (String) -> Unit)
@Composable expect fun PlatformInputRegistration(client: ClientConnection, preferences: Preferences, allowed: () -> Boolean)
@Composable expect fun PlatformInputSettings(preferences: Preferences, update: (Preferences) -> Unit)
expect fun copyPublicKey(value: String)

@Composable fun App(model: AppModel, mobileLayout: Boolean = false) {
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
    val colors = if (dark) darkColorScheme(
        primary = Color(0xff75aaff), secondary = Color(0xff75aaff),
        primaryContainer = Color(0xff2b4264), secondaryContainer = Color(0xff2b4264),
        background = Color(0xff1e1f22), surface = Color(0xff1e1f22), surfaceTint = Color.Transparent,
        onBackground = Color(0xffdadce0), onSurface = Color(0xffdadce0),
        surfaceVariant = Color(0xff25262a), onSurfaceVariant = Color(0xff9ca1aa), surfaceContainer = Color(0xff25262a), surfaceContainerLow = Color(0xff25262a), surfaceContainerHigh = Color(0xff25262a), surfaceContainerHighest = Color(0xff25262a), surfaceContainerLowest = Color(0xff1e1f22),
        outlineVariant = Color(0xff3b3d42),
    ) else lightColorScheme(
        primary = Color(0xff2468c9), secondary = Color(0xff2468c9),
        primaryContainer = Color(0xffdbe9ff), secondaryContainer = Color(0xffdbe9ff),
        background = Color(0xfff7f7f8), surface = Color(0xfff7f7f8), surfaceTint = Color.Transparent,
        onBackground = Color(0xff24262b), onSurface = Color(0xff24262b),
        surfaceVariant = Color(0xffeeeef0), onSurfaceVariant = Color(0xff626670), surfaceContainer = Color(0xffeeeef0), surfaceContainerLow = Color(0xffeeeef0), surfaceContainerHigh = Color(0xffeeeef0), surfaceContainerHighest = Color(0xffeeeef0), surfaceContainerLowest = Color.White,
        outlineVariant = Color(0xffd7d9df),
    )
    MaterialTheme(colorScheme = colors, shapes = Shapes(
        extraSmall = androidx.compose.foundation.shape.RoundedCornerShape(0.dp), small = androidx.compose.foundation.shape.RoundedCornerShape(0.dp),
        medium = androidx.compose.foundation.shape.RoundedCornerShape(0.dp), large = androidx.compose.foundation.shape.RoundedCornerShape(0.dp),
        extraLarge = androidx.compose.foundation.shape.RoundedCornerShape(0.dp))) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding()) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("dgl", style = MaterialTheme.typography.titleLarge)
                    Spacer(Modifier.width(8.dp))
                    Text("SSH · tmux", style = MaterialTheme.typography.labelMedium, modifier = Modifier.weight(1f))
                    TextButton({ explorer = !explorer }) { Text(if (explorer) "터미널" else "호스트") }
                    TextButton({ settings = true }) { Text("설정") }
                }
                if (mobileLayout && connections.isNotEmpty()) {
                    var switching by remember { mutableStateOf(false) }
                    Box(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
                        TextButton({ switching = true }) { Text((selected?.host?.alias ?: "연결 선택") + " ▾", maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        DropdownMenu(switching, { switching = false }) {
                            connections.forEach { connection -> DropdownMenuItem(text = { Text(connection.host.alias, maxLines = 2, overflow = TextOverflow.Ellipsis) }, onClick = { model.selected.value = connection; explorer = false; switching = false }) }
                        }
                    }
                } else if (connections.isNotEmpty()) Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
                    connections.forEach { connection ->
                        FilterChip(selected == connection, { model.selected.value = connection; explorer = false }, label = { Text(connection.host.alias) })
                        TextButton({ model.removeConnection(connection) }) { Text("×") }
                    }
                }
                BoxWithConstraints(Modifier.weight(1f)) {
                    val wide = !mobileLayout && maxWidth >= 840.dp
                    Row(Modifier.fillMaxSize()) {
                        if (wide || selected == null || explorer) {
                            Column(if (wide) Modifier.width(272.dp).fillMaxHeight() else Modifier.fillMaxSize()) {
                                Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                                    Text("연결", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                                    TextButton({ adding = true }) { Text("+ 호스트") }
                                }
                                LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(8.dp), verticalArrangement = Arrangement.spacedBy(0.dp)) {
                                    if (local.hosts.isEmpty()) item { Text("SSH 호스트를 추가하여 시작하세요.\n원격 서버에는 SSH와 tmux만 필요합니다.", Modifier.padding(12.dp)) }
                                    items(local.hosts.sortedWith(compareByDescending<HostProfile> { it.favorite }.thenByDescending { it.lastConnected }), key = { it.id }) { host ->
                                        Column(Modifier.fillMaxWidth()) {
                                            Column(Modifier.padding(12.dp)) {
                                                Text((if (host.favorite) "★ " else "") + host.alias, style = MaterialTheme.typography.titleMedium)
                                                Text("${host.username}@${host.hostname}:${host.port}", style = MaterialTheme.typography.bodySmall)
                                                if (host.lastConnected != 0L) Text("최근 연결: " + java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT).format(java.util.Date(host.lastConnected)), style = MaterialTheme.typography.labelSmall)
                                                Row(verticalAlignment = Alignment.CenterVertically) {
                                                    TextButton({ model.open(host); explorer = false }) { Text("연결") }
                                                    Spacer(Modifier.weight(1f))
                                                    TreeMenu("${host.alias} 메뉴", listOf("편집" to { edit = host }, "삭제…" to { deletion = host }))
                                                }
                                            }
                                            HorizontalDivider()
                                        }
                                    }
                                }
                                Text(model.storageDescription, Modifier.padding(12.dp), style = MaterialTheme.typography.labelSmall)
                            }
                        }
                        if (wide || (selected != null && !explorer)) {
                            Box(Modifier.weight(1f).fillMaxHeight()) {
                                if (selected != null) key(selected) {
                                    val client = selected!!
                                    ConnectionScreen(client, local.preferences, mobileLayout) {
                                        model.selected.value === client && (wide || !explorer) &&
                                            !settings && !adding && edit == null && deletion == null && error == null
                                    }
                                }
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
        if (adding || edit != null) HostEditor(model, edit, mobileLayout, { adding = false; edit = null })
        if (settings) SettingsDialog(model, mobileLayout) { settings = false }
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable private fun ConnectionScreen(client: ClientConnection, preferences: Preferences, mobileLayout: Boolean, inputAllowed: () -> Boolean) {
    val status by client.status.collectAsState()
    val remote by client.remote.collectAsState()
    val refreshing by client.refreshing.collectAsState()
    val expanded = remember(client) { mutableStateMapOf<String, Boolean>() }
    val session by client.session.collectAsState()
    val pane by client.pane.collectAsState()
    val trust by client.trustRequest.collectAsState()
    val notice by client.notice.collectAsState()
    val tmuxProblem by client.tmuxProblem.collectAsState()
    var tmuxDetails by remember(tmuxProblem) { mutableStateOf(false) }
    var navigation by remember { mutableStateOf(!mobileLayout) }
    var namePrompt by remember { mutableStateOf<Pair<String, (String) -> Unit>?>(null) }
    var deletion by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    PlatformInputRegistration(client, preferences) {
        inputAllowed() && !(mobileLayout && navigation) && !tmuxDetails && namePrompt == null && deletion == null &&
            client.trustRequest.value == null && client.notice.value == null
    }
    val currentPane = remote.panes.firstOrNull { it.id == pane }
    val currentWindow = remote.windows.firstOrNull { it.id == currentPane?.window && it.session == session }
    val currentSession = remote.sessions.firstOrNull { it.id == session }
    val canBrowse = status.phase in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)
    val canManage = status.phase == Phase.Ready
    val sessionLabel = when (status.phase) {
        Phase.Shell -> "없음 · 일반 SSH"
        Phase.Disconnected, Phase.Failed -> "연결 안 됨"
        Phase.Connecting, Phase.Authenticating, Phase.TrustRequired -> "확인 중"
        Phase.Reconnecting -> "복구 중"
        Phase.MissingSession -> "이전 세션 없음"
        else -> currentSession?.let { "${it.name} (${it.id})" } ?: "선택 안 함"
    }
    val navigationContent: @Composable (Modifier) -> Unit = { modifier ->
        Column(modifier.heightIn(max = if (mobileLayout) 480.dp else 280.dp).verticalScroll(rememberScrollState())) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("세션", Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
                TextButton({ namePrompt = "세션 만들기" to client::createSession }, enabled = canBrowse) { Text("+") }
                TooltipBox(positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(), tooltip = { PlainTooltip { Text("세션 새로고침") } }, state = rememberTooltipState()) {
                    IconButton({ client.refresh() }, enabled = canBrowse && !refreshing, modifier = Modifier.semantics { contentDescription = "세션 새로고침" }) { Text("↻") }
                }
            }
            if (status.phase == Phase.Shell) Text("일반 SSH · tmux 목록 확인 불가", Modifier.padding(16.dp))
            else if (remote.sessions.isEmpty()) Text(if (canBrowse) "세션 없음 · + 버튼으로 생성" else "목록 대기 중", Modifier.padding(16.dp))
            remote.sessions.forEach { s ->
                TreeRow(s.name, 0, session == s.id, expanded[s.id] == true, { expanded[s.id] = expanded[s.id] != true },
                    { client.selectTarget(s.id) }, canBrowse, listOf(
                        "+ 창" to { namePrompt = "창 만들기" to { client.manage(TmuxCommand.newWindow(s.id, it)) } },
                        "이름 변경" to { namePrompt = "세션 이름 변경" to { client.manage(TmuxCommand.renameSession(s.id, it)) } },
                        "삭제…" to { deletion = "세션 ${s.name}" to { client.manage(TmuxCommand.killSession(s.id)) } },
                    ))
                if (expanded[s.id] == true) remote.windows.filter { it.session == s.id }.forEach { w ->
                    val key = "${s.id}/${w.id}"
                    TreeRow(w.name, 1, session == s.id && currentWindow?.id == w.id, expanded[key] == true, { expanded[key] = expanded[key] != true },
                        { client.selectTarget(s.id, w.id) }, canBrowse, listOf(
                            "이름 변경" to { namePrompt = "창 이름 변경" to { client.manage(TmuxCommand.renameWindow(w.id, it)) } },
                            "삭제…" to { deletion = "창 ${w.name}" to { client.manage(TmuxCommand.killWindow(w.id)) } },
                        ))
                    if (expanded[key] == true) remote.panes.filter { it.window == w.id }.forEach { p ->
                        TreeRow("${p.id} ${p.command} ${p.width}×${p.height}", 2, session == s.id && pane == p.id, null, {},
                            { client.selectTarget(s.id, w.id, p.id) }, canBrowse, listOf(
                                "삭제…" to { deletion = "패널 ${p.id}" to { client.manage(TmuxCommand.killPane(p.id)) } },
                            ))
                    }
                }
            }
            HorizontalDivider()
            Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                TextButton({ pane?.let { client.manage(TmuxCommand.split(it, true)) } }, enabled = canManage && pane != null) { Text("tmux 패널 분할 ↔") }
                TextButton({ pane?.let { client.manage(TmuxCommand.split(it, false)) } }, enabled = canManage && pane != null) { Text("tmux 패널 분할 ↕") }
            }
            Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                listOf("←" to 'L', "→" to 'R', "↑" to 'U', "↓" to 'D').forEach { (label, direction) ->
                    TextButton({ pane?.let { client.manage(TmuxCommand.focusPane(it, direction)) } }, enabled = canManage && pane != null) { Text("포커스 $label") }
                }
            }
            Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                listOf("←" to 'L', "→" to 'R', "↑" to 'U', "↓" to 'D').forEach { (label, direction) ->
                    TextButton({ pane?.let { client.manage(TmuxCommand.resizePane(it, direction)) } }, enabled = canManage && pane != null) { Text("경계$label") }
                }
            }
        }
    }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
                Text(status.message, style = MaterialTheme.typography.labelSmall, modifier = Modifier.weight(1f, fill = false),
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
                if (status.phase == Phase.Shell && tmuxProblem != null) TmuxInfoButton { tmuxDetails = true }
            }
            if (status.phase == Phase.Disconnected || status.phase == Phase.Failed || status.phase == Phase.MissingSession) TextButton({ client.connect() }) { Text("재연결") }
            else TextButton({ client.disconnect() }) { Text("연결 종료") }
        }
        Box(Modifier.fillMaxWidth().height(4.dp)) {
            if (status.phase == Phase.Connecting || status.phase == Phase.Restoring || status.phase == Phase.Reconnecting) LinearProgressIndicator(Modifier.fillMaxWidth())
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("현재 세션: $sessionLabel", modifier = Modifier.weight(1f), style = MaterialTheme.typography.labelMedium,
                maxLines = 2, overflow = TextOverflow.Ellipsis)
            TextButton({ navigation = !navigation }) { Text(if (navigation) "tmux 접기" else "tmux 관리") }
        }
        if (navigation && !mobileLayout) navigationContent(Modifier)
        if (currentPane?.mode?.isNotBlank() == true) Text("tmux ${currentPane.mode} · 아래 터미널은 일반 패널 출력입니다. 로컬 스크롤·선택을 사용할 수 있습니다.", Modifier.padding(8.dp), style = MaterialTheme.typography.labelSmall)
        Box(Modifier.weight(1f).fillMaxWidth().background(Color(0xff11151b))) {
            PlatformTerminal(client, preferences, Modifier.fillMaxSize())
        }
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TerminalKey.toolbar.forEach { shortcut ->
                TextButton({ client.input(shortcut.bytes()) }, enabled = status.acceptsInput, contentPadding = PaddingValues(horizontal = 10.dp)) { Text(shortcut.label) }
            }
        }
    }
    if (mobileLayout && navigation) ModalBottomSheet(onDismissRequest = { navigation = false }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Text("tmux 세션 · 창 · 패널", Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleMedium)
        navigationContent(Modifier.weight(1f, fill = false))
        TextButton({ navigation = false }, modifier = Modifier.fillMaxWidth()) { Text("터미널로 돌아가기") }
    }
    trust?.let { request ->
        AlertDialog(onDismissRequest = { client.trustRequest.value = null; client.disconnect() }, title = { Text("처음 연결하는 서버") },
            text = { Column { Text(request.endpoint); Spacer(Modifier.height(8.dp)); SelectionContainer { Text(request.fingerprint) }; Spacer(Modifier.height(8.dp)); Text("서버 관리자에게 받은 SHA-256 지문과 일치하는지 확인하세요.") } },
            confirmButton = { TextButton({ client.trust() }) { Text("신뢰하고 연결") } }, dismissButton = { TextButton({ client.trustRequest.value = null; client.disconnect() }) { Text("취소") } })
    }
    namePrompt?.let { (title, action) -> NameDialog(title, { action(it); namePrompt = null }, { namePrompt = null }) }
    if (tmuxDetails && status.phase == Phase.Shell) tmuxProblem?.let {
        TmuxFailureDialog(client.host, it) { tmuxDetails = false }
    }
    deletion?.let { (target, action) -> ConfirmDialog("$target 삭제", "이 대상의 원격 프로세스가 종료됩니다. 삭제할까요?", { action(); deletion = null }, { deletion = null }) }
    notice?.let { message -> AlertDialog(onDismissRequest = { client.notice.value = null }, text = { Text(message) }, confirmButton = { TextButton({ client.notice.value = null }) { Text("닫기") } }) }
}

@Composable private fun TreeMenu(label: String, actions: List<Pair<String, () -> Unit>>, enabled: Boolean = true) {
    var menu by remember { mutableStateOf(false) }
    Box {
        IconButton({ menu = true }, enabled = enabled, modifier = Modifier.semantics { contentDescription = label }) { Text("…") }
        DropdownMenu(menu, { menu = false }) {
            actions.forEach { (text, action) -> DropdownMenuItem(text = { Text(text) }, onClick = { menu = false; action() }) }
        }
    }
}
@Composable private fun TreeRow(label: String, depth: Int, active: Boolean, expanded: Boolean?, toggle: () -> Unit,
    select: () -> Unit, enabled: Boolean, actions: List<Pair<String, () -> Unit>>) {
    Row(Modifier.fillMaxWidth().background(if (active) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent)
        .padding(start = (depth * 18).dp).semantics { selected = active }, verticalAlignment = Alignment.CenterVertically) {
        if (expanded != null) IconButton(toggle, modifier = Modifier.semantics { contentDescription = "$label ${if (expanded) "접기" else "펼치기"}" }) { Text(if (expanded) "▾" else "▸") }
        else Spacer(Modifier.width(48.dp))
        TextButton(select, enabled = enabled, shape = RectangleShape, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
            Text(label, Modifier.fillMaxWidth(), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        TreeMenu("$label 메뉴", actions, enabled)
    }
}

@Composable private fun HostEditor(model: AppModel, original: HostProfile?, mobileLayout: Boolean, dismiss: () -> Unit) {
    var alias by remember { mutableStateOf(original?.alias ?: "") }
    var hostname by remember { mutableStateOf(original?.hostname ?: "") }
    var port by remember { mutableStateOf((original?.port ?: 22).toString()) }
    var username by remember { mutableStateOf(original?.username ?: "") }
    var auth by remember { mutableStateOf(original?.auth ?: AuthKind.Password) }
    var password by remember { mutableStateOf("") }
    var privateKey by remember { mutableStateOf("") }
    var passphrase by remember { mutableStateOf("") }
    var showPassword by remember(auth) { mutableStateOf(false) }
    var favorite by remember { mutableStateOf(original?.favorite ?: false) }
    var generatedPublic by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    EditorDialog(mobileLayout, onDismissRequest = dismiss, title = { Text(if (original == null) "SSH 호스트 추가" else "호스트 편집") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(0.dp)) {
            OutlinedTextField(alias, { alias = it }, label = { Text("별칭") }, singleLine = true)
            OutlinedTextField(hostname, { hostname = it }, label = { Text("호스트 / IP") }, singleLine = true)
            Row {
                OutlinedTextField(port, { port = it }, label = { Text("포트") }, singleLine = true, modifier = Modifier.width(90.dp))
                Spacer(Modifier.width(8.dp))
                OutlinedTextField(username, { username = it }, label = { Text("사용자 이름") }, singleLine = true)
            }
            Row(verticalAlignment = Alignment.CenterVertically) { Checkbox(favorite, { favorite = it }); Text("즐겨찾기") }
            Row { FilterChip(auth == AuthKind.Password, { auth = AuthKind.Password }, label = { Text("비밀번호") }); Spacer(Modifier.width(8.dp)); FilterChip(auth == AuthKind.PrivateKey, { auth = AuthKind.PrivateKey }, label = { Text("개인키") }) }
            val passwordTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation()
            val showPasswordControl: @Composable () -> Unit = {
                Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).toggleable(showPassword, role = Role.Checkbox, onValueChange = { showPassword = it }), verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(showPassword, onCheckedChange = null, modifier = Modifier.padding(12.dp))
                    Text("비밀번호 표시")
                }
            }
            if (auth == AuthKind.Password) {
                OutlinedTextField(password, { password = it }, label = { Text(if (original == null) "비밀번호" else "새 비밀번호 (비우면 기존 유지)") }, visualTransformation = passwordTransformation, singleLine = true)
                showPasswordControl()
            }
            else {
                ImportKeyButton({ privateKey = it; generatedPublic = null }, { error = it })
                Text(if (privateKey.isEmpty()) "Ed25519 / RSA · 암호화 키 지원" else "개인키를 가져왔습니다.", style = MaterialTheme.typography.labelSmall)
                OutlinedTextField(passphrase, { passphrase = it }, label = { Text("키 암호") }, visualTransformation = passwordTransformation, singleLine = true)
                showPasswordControl()
                TextButton({ try { val generated = model.keys.ed25519(); privateKey = generated.credential.privateKey; passphrase = ""; generatedPublic = generated.publicKey } catch (_: Exception) { error = "키 생성 실패" } }) { Text("Ed25519 키 생성") }
                if (privateKey.isNotEmpty() || original?.auth == AuthKind.PrivateKey) {
                    Text("공개키를 서버의 authorized_keys에 등록하세요.")
                    TextButton({
                        try {
                            val credential = if (privateKey.isNotEmpty()) Credential(privateKey = privateKey, passphrase = passphrase)
                                else requireNotNull(model.credentials.read(original!!.credentialRef)).let { if (passphrase.isNotEmpty()) Credential(it.password, it.privateKey, passphrase) else it }
                            copyPublicKey(generatedPublic ?: model.keys.publicKey(credential))
                        } catch (_: Exception) { error = "공개키를 읽지 못했습니다. 개인키와 키 암호 또는 보호 저장소를 확인하세요." }
                    }) { Text("공개키 복사") }
                }
            }
            Text(model.storageDescription, style = MaterialTheme.typography.labelSmall)
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        }
    }, confirmButton = {
        TextButton({
            try {
                val id = original?.id ?: UUID.randomUUID().toString()
                val profile = HostProfile(id, alias.trim(), hostname.trim(), port.toInt(), username.trim(), auth, original?.credentialRef ?: id, favorite, original?.lastConnected ?: 0, original?.lastSession, original?.lastPane)
                val changed = (auth == AuthKind.Password && password.isNotEmpty()) || (auth == AuthKind.PrivateKey && (privateKey.isNotEmpty() || passphrase.isNotEmpty()))
                if (original == null || original.auth != auth) require(changed)
                if (changed) {
                    val previous = if (original == null) Credential() else requireNotNull(model.credentials.read(profile.credentialRef))
                    val credential = if (auth == AuthKind.Password) Credential(password, previous.privateKey, previous.passphrase)
                        else Credential(previous.password, privateKey.ifEmpty { previous.privateKey }, passphrase)
                    if (auth == AuthKind.PrivateKey) model.keys.publicKey(credential)
                    model.credentials.write(profile.credentialRef, credential)
                }
                model.update(model.local.value.copy(hosts = model.local.value.hosts.filter { it.id != id } + profile))
                dismiss()
            } catch (_: IllegalArgumentException) { error = "필수 입력과 포트(1–65535), 자격 증명을 확인하세요." }
            catch (_: Exception) { error = "보호 저장소에 저장하지 못했습니다. 저장을 중단했습니다." }
        }) { Text("저장") }
    }, dismissButton = { TextButton(dismiss) { Text("취소") } })
}

@Composable private fun SettingsDialog(model: AppModel, mobileLayout: Boolean, dismiss: () -> Unit) {
    val local by model.local.collectAsState()
    var forget by remember { mutableStateOf<String?>(null) }
    EditorDialog(mobileLayout, onDismissRequest = dismiss, title = { Text("설정") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("테마", style = MaterialTheme.typography.titleSmall)
            Row { listOf("system" to "시스템", "light" to "밝게", "dark" to "어둡게").forEach { (value, label) -> FilterChip(local.preferences.theme == value, { model.update(local.copy(preferences = local.preferences.copy(theme = value))) }, label = { Text(label) }) } }
            Text("터미널 글자 크기: ${local.preferences.fontSize}")
            Row {
                TextButton({ model.update(local.copy(preferences = local.preferences.copy(fontSize = (local.preferences.fontSize - 1).coerceAtLeast(10)))) }) { Text("작게") }
                TextButton({ model.update(local.copy(preferences = local.preferences.copy(fontSize = (local.preferences.fontSize + 1).coerceAtMost(28)))) }) { Text("크게") }
            }
            Text("스크롤백: ${local.preferences.scrollback}줄 · 터미널에서 스크롤·선택·복사")
            PlatformInputSettings(local.preferences) { preferences -> model.update(model.local.value.copy(preferences = preferences)) }
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

@Composable private fun EditorDialog(
    fullscreen: Boolean,
    onDismissRequest: () -> Unit,
    title: @Composable () -> Unit,
    text: @Composable () -> Unit,
    confirmButton: @Composable () -> Unit,
    dismissButton: @Composable () -> Unit = {},
) {
    if (!fullscreen) AlertDialog(onDismissRequest = onDismissRequest, title = title, text = text, confirmButton = confirmButton, dismissButton = dismissButton)
    else Dialog(onDismissRequest, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                ProvideTextStyle(MaterialTheme.typography.titleLarge) { title() }
                Box(Modifier.weight(1f).fillMaxWidth()) { text() }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) { dismissButton(); confirmButton() }
            }
        }
    }
}
