package kim.jujin.rovessh.ui

import kim.jujin.rovessh.core.t
import kim.jujin.rovessh.core.tr
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kim.jujin.rovessh.*

@Composable internal fun CloudSyncSettings(model: AppModel) {
    val sync = model.cloudSync ?: return
    val state by sync.status.collectAsState()
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(t("설정·호스트 동기화"), style = MaterialTheme.typography.titleSmall)
        Text(t("Google Drive의 같은 dgl-sync.json을 각 기기에서 연결하세요. PC에서는 Drive 앱의 동기화 폴더를 선택합니다."))
        Text(t("첫 연결에서는 공유 파일의 공통 설정을 적용하고 이 기기의 호스트 목록을 병합합니다."), style = MaterialTheme.typography.bodySmall)
        Row {
            TextButton({ model.chooseSyncFile(false) }, enabled = !state.busy) { Text(t("Google Drive 파일 연결")) }
            TextButton({ model.chooseSyncFile(true) }, enabled = !state.busy) { Text(t("새 파일")) }
        }
        Text(t("iCloud Drive: Android에서는 직접 파일 연결을 지원하지 않습니다. Mac·Windows에서 사용할 수 있습니다."), style = MaterialTheme.typography.bodySmall)
        Text(t("호스트·테마·강조 색상 선택·글자 크기·스크롤백·앱 내 알림 설정을 공유합니다. 비밀번호·개인키·신뢰 키·매크로·OS 알림은 이 기기에 유지합니다."), style = MaterialTheme.typography.bodySmall)
        if (state.connected) {
            Text(state.location, style = MaterialTheme.typography.bodySmall)
            Row {
                TextButton({ sync.sync() }, enabled = !state.busy) { Text(t("지금 파일 동기화")) }
                TextButton({ sync.disconnect() }, enabled = !state.busy) { Text(t("연결 해제")) }
            }
        }
        Text(state.message, style = MaterialTheme.typography.bodySmall)
        state.conflicts.forEach { conflict ->
            Text(t("동시 수정 · 유지할 값을 선택하세요"), style = MaterialTheme.typography.titleSmall)
            conflict.choices.forEach { choice ->
                OutlinedButton({ sync.sync(SyncResolution(conflict.key, choice.revision)) }, enabled = !state.busy, modifier = Modifier.fillMaxWidth()) { Text(choice.label) }
            }
        }
        Text(t("파일 제공 앱이 클라우드 전송을 처리합니다. 다른 기기에 반영되기까지 시간이 걸릴 수 있습니다."), style = MaterialTheme.typography.bodySmall)
    }
}
