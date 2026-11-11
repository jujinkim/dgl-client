package kim.jujin.dglc.ui

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*

@Composable internal fun CloudSyncSettings(model: AppModel) {
    val sync = model.cloudSync ?: return
    val state by sync.status.collectAsState()
    DisposableEffect(sync) { onDispose { sync.cancelImport() } }
    val busy = state.busy || state.pendingImportCount != null
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(t("호스트 정보와 공통 설정을 공유합니다. 비밀번호·개인키 등 민감한 정보는 내보내기와 동기화에 포함되지 않습니다."))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.Export) }, enabled = !busy) { Text(t("내보내기…")) }
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.Import) }, enabled = !busy) { Text(t("가져오기…")) }
        }
        state.pendingImportCount?.let { count ->
            Text(tr("호스트 {0}개를 가져옵니다. 기존 목록에 병합하고 공통 설정은 파일 값으로 갱신합니다.", count))
            Text(t("파일에 없는 기존 호스트는 유지합니다. 연결 중인 동기화가 있으면 가져온 변경도 동기화됩니다."), style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                BorderedButton({ sync.confirmImport() }, enabled = !state.busy) { Text(t("가져오기")) }
                BorderedButton({ sync.cancelImport() }, enabled = !state.busy) { Text(t("취소")) }
            }
        }
        HorizontalDivider()
        Text(t("파일 동기화"), style = MaterialTheme.typography.titleSmall)
        Text(t("각 기기에서 같은 동기화 파일을 연결하세요. 실제 클라우드 전송은 선택한 파일 제공 앱이 처리합니다."))
        Text(t("첫 연결에서는 공유 파일의 공통 설정을 적용하고 이 기기의 호스트 목록을 병합합니다."), style = MaterialTheme.typography.bodySmall)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.OpenSync) }, enabled = !busy) { Text(t("동기화 파일 연결…")) }
            BorderedButton({ model.chooseSettingsFile(SettingsFileAction.CreateSync) }, enabled = !busy) { Text(t("새 동기화 파일…")) }
        }
        Text(t("호스트·테마·강조 색상 선택·글자 크기·스크롤백·앱 내 알림 설정을 공유합니다. 비밀번호·개인키·신뢰 키·매크로·OS 알림은 이 기기에 유지합니다."), style = MaterialTheme.typography.bodySmall)
        if (state.connected) {
            Text(state.location, style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                BorderedButton({ sync.sync() }, enabled = !busy) { Text(t("지금 파일 동기화")) }
                BorderedButton({ sync.disconnect() }, enabled = !busy) { Text(t("연결 해제")) }
            }
        }
        Text(t(state.message), style = MaterialTheme.typography.bodySmall)
        state.conflicts.forEach { conflict ->
            Text(t("동시 수정 · 유지할 값을 선택하세요"), style = MaterialTheme.typography.titleSmall)
            conflict.choices.forEach { choice ->
                OutlinedButton({ sync.sync(SyncResolution(conflict.key, choice.revision)) }, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text(choice.label) }
            }
        }
        Text(t("파일 제공 앱이 클라우드 전송을 처리합니다. 다른 기기에 반영되기까지 시간이 걸릴 수 있습니다."), style = MaterialTheme.typography.bodySmall)
    }
}
