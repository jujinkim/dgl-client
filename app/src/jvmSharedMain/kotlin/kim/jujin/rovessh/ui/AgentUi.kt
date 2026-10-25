package kim.jujin.rovessh.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kim.jujin.rovessh.*
import kim.jujin.rovessh.core.*
import kotlinx.serialization.json.*

@Composable internal fun AgentBadge(client: ClientConnection, pane: String? = null, notices: List<AgentNotice> = emptyList(), compact: Boolean = false, onDetails: (() -> Unit)? = null) {
    val snapshot by client.agents.collectAsState()
    val (observations, unread, label) = snapshot.panelState(pane, client.agentEndpoint, notices, System.currentTimeMillis())
    val description = observations.map { agentName(it.agent) }.distinct().joinToString().ifEmpty { "터미널" } + " · " + label
    if (compact) {
        val icon = when { label.contains("질문") -> "?"; label.contains("승인") -> "◇"; label.contains("오류") -> "!"; label.contains("응답") -> "○"; label.contains("작업 사건") -> "↻"; else -> "—" }
        IconButton({ onDetails?.invoke() }, enabled = onDetails != null,
            modifier = Modifier.size(48.dp).semantics { contentDescription = "창 에이전트 상태 · $description" }) {
            Text(icon + if (unread.isEmpty()) "" else unread.size.toString(), maxLines = 1)
        }
    } else Row(Modifier.semantics { contentDescription = description }, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        observations.map { it.agent }.distinct().forEach { agent -> Icon(LauncherIcons.agent(agent), agentName(agent), Modifier.size(16.dp)) }
        Text(label, style = MaterialTheme.typography.labelSmall, maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
    }
}

@Composable internal fun AgentDetails(client: ClientConnection, model: AppModel, pane: String? = null, dismiss: () -> Unit) {
    val snapshot by client.agents.collectAsState()
    val setup by client.agentSetup.collectAsState()
    val notices by model.agentNotifications.collectAsState()
    var inspected by remember { mutableStateOf(false) }
    val preview = setup?.let { runCatching { Json.parseToJsonElement(it).jsonObject }.getOrNull() }
    val token = preview?.get("token")?.jsonPrimitive?.content
    AlertDialog(onDismissRequest = dismiss, title = { Text("에이전트 상태 연동 · ${client.host.alias}") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("마지막으로 관측된 사건입니다. 현재 입력·승인 대기나 작업 성공을 뜻하지 않습니다.")
            AgentBadge(client, pane, notices)
            val remote by client.remote.collectAsState()
            snapshot.panelState(pane, client.agentEndpoint, notices, System.currentTimeMillis()).observations.forEach { o ->
                Text("${agentName(o.agent)} · ${remote.targetLabel(o.session, o.pane)}" + if (o.actor != "parent") " · 서브에이전트" else "")
                Text(o.last?.let { "${agentMessage(it.kind)} · ${it.event}\n${java.util.Date(it.at)}" } ?: "미연동 · 상태 미확인")
                Text("진단 정보 · 세션 ID: ${o.session} · 창 ID: ${o.window} · 패널 ID: ${o.pane}", style = MaterialTheme.typography.labelSmall)
                Text("수신 확인 사건: ${o.supported.joinToString { agentMessage(it) }.ifEmpty { "미확인" }}", style = MaterialTheme.typography.labelSmall)
                TextButton({
                    model.openAgentObservation(client, o)
                    dismiss()
                }) { Text("패널로 이동") }
            }
            if (pane == null) {
            Text("선택 설치: Python 3 도우미와 사용자 훅을 추가합니다. 기존 훅을 보존하고 설정 백업을 만듭니다. 대화·명령 본문은 저장하지 않습니다.")
            TextButton({ inspected = true; client.configureAgents("inspect") }) { Text("설치 전 검사 · 변경 내용 보기") }
            if (inspected && preview == null) Text("검사 중…")
            preview?.get("error")?.jsonPrimitive?.content?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            if (token != null) {
                Text("Python ${preview["python"]?.jsonPrimitive?.content} · ${preview["tmux"]?.jsonPrimitive?.content}")
                Text("도우미: ${preview["helper"]?.jsonPrimitive?.content}\n관측 저장: ${preview["database"]?.jsonPrimitive?.content}")
                preview["configs"]?.jsonArray?.forEach { item ->
                    val c = item.jsonObject
                    Text("${c["agent"]?.jsonPrimitive?.content}: ${c["version"]?.jsonPrimitive?.content}\n${c["path"]?.jsonPrimitive?.content}")
                    Text(if (c["installed"]?.jsonPrimitive?.boolean == true) "설치됨 · 활성화 확인 필요 (실제 사건은 위에서 확인)" else "설치되지 않음")
                    Text(if (c["supported"]?.jsonPrimitive?.boolean == true) "추가 훅: ${c["events"]?.jsonArray?.joinToString { it.jsonPrimitive.content }}" else "지원 확인되지 않은 CLI 버전 · 설치 제외")
                    if (c["experimental"]?.jsonPrimitive?.boolean == true) Text("실험 지원 · 실제 CLI 수신 미검증")
                    c["error"]?.jsonPrimitive?.content?.takeIf { it.isNotEmpty() }?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    Text("수집 가능 사건: ${c["capabilities"]?.jsonArray?.joinToString { it.jsonPrimitive.content }}")
                    c["limitations"]?.jsonPrimitive?.content?.takeIf { it.isNotEmpty() }?.let { Text(it) }
                }
                Text("Codex는 /hooks에서 직접 검토·신뢰해야 합니다. 새 실행 또는 다음 실제 이벤트 수신 후 확인됩니다. 기존 CLI는 종료하지 않습니다.")
                TextButton({ client.configureAgents("install", token) }) { Text("연동 설치") }
                TextButton({ client.configureAgents("remove", token) }) { Text("dgl 훅만 해제") }
            }
            }
        }
    }, confirmButton = { TextButton(dismiss) { Text("닫기") } })
}

@Composable internal fun AgentNotifications(model: AppModel, dismiss: () -> Unit) {
    val notices by model.agentNotifications.collectAsState()
    AlertDialog(onDismissRequest = dismiss, title = { Text("최근 알림 · ${notices.count { !it.read }} 읽지 않음") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("이번 앱 실행 중 최대 100건. 읽음은 질문 응답·승인 처리를 뜻하지 않습니다.")
            if (notices.isEmpty()) Text("수신한 알림이 없습니다.")
            notices.forEach { n ->
                Text("${if (n.read) "" else "● "}${agentName(n.event.agent)} · ${n.endpoint}\n진단 정보 · 세션 ID: ${n.event.session} · 창 ID: ${n.event.window} · 패널 ID: ${n.event.pane}" + if (n.event.actor != "parent") " · 서브에이전트" else "")
                Text(agentMessage(n.event.kind))
                Text("${n.event.event} · ${java.util.Date(n.event.at)}", style = MaterialTheme.typography.labelSmall)
                TextButton({ model.openAgentNotice(n.id); dismiss() }) { Text("패널로 이동 · 읽음") }
            }
        }
    }, confirmButton = { TextButton(dismiss) { Text("닫기") } })
}

@Composable internal fun AgentNotificationSettingsUi(model: AppModel) {
    val local by model.local.collectAsState()
    val settings = local.preferences.agentNotifications
    Text("에이전트 알림", style = MaterialTheme.typography.titleSmall)
    listOf("질문·승인 요청" to settings.questions, "응답 종료 신호" to settings.responses, "명시적 오류" to settings.errors, "OS 알림" to settings.os).forEachIndexed { i, (label, enabled) ->
        Row { Checkbox(enabled, { value ->
            val next = when (i) { 0 -> settings.copy(questions = value); 1 -> settings.copy(responses = value); 2 -> settings.copy(errors = value); else -> settings.copy(os = value) }
            model.modify { it.copy(preferences = it.preferences.copy(agentNotifications = next)) }
        }); Text(label, Modifier.padding(top = 12.dp)) }
    }
    Text("OS 권한·집중 모드와 관계없이 앱 내 알림은 유지됩니다. 앱과 SSH 연결이 유지되는 동안 수신합니다.", style = MaterialTheme.typography.labelSmall)
}
