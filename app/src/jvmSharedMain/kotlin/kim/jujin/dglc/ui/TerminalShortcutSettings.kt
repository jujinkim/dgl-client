package kim.jujin.dglc.ui

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.TerminalKey

@OptIn(ExperimentalLayoutApi::class)
@Composable internal fun TerminalShortcutSettings(shortcuts: List<TerminalKey>, update: (List<TerminalKey>) -> Unit) {
    Column {
        Text(t("하단 단축키"), style = MaterialTheme.typography.titleSmall)
        Text(t("표시할 키를 선택하세요. 새로 켠 키는 맨 뒤에 추가됩니다."))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            (TerminalKey.defaultShortcuts + TerminalKey.entries).distinct().forEach { key ->
                val selected = key in shortcuts
                FilterChip(selected, {
                    update(if (selected) shortcuts.filterNot { it == key } else shortcuts + key)
                }, label = { Text(key.label) })
            }
        }
        TextButton({ update(TerminalKey.defaultShortcuts) }) { Text(t("단축키 기본값 복원")) }
    }
}
