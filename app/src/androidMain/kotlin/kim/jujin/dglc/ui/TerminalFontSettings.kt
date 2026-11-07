package kim.jujin.dglc.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

@Composable actual fun PlatformTerminalFontSettings(family: String, select: (String) -> Unit) {
    var choosing by remember { mutableStateOf(false) }
    var fonts by remember { mutableStateOf<List<TerminalFontFile>?>(null) }
    LaunchedEffect(Unit) { fonts = try { AndroidTerminalFonts.installed() } catch (_: Exception) { emptyList() } }
    val installed = fonts?.firstOrNull { it.family == family }
    val missing = family.isNotEmpty() && fonts != null && installed == null
    val default = tr("기본 ({0})", "monospace")
    val label = when {
        family.isEmpty() -> default
        missing -> tr("{0} 없음 · 기본 글꼴 사용", family)
        else -> family
    }
    TextButton({ choosing = true }, Modifier.fillMaxWidth()) {
        Text(tr("터미널 글꼴: {0}", label), Modifier.weight(1f)); UiGlyph(UiIcons.chevronRight)
    }
    if (!choosing) return
    val choose = { value: String -> select(value); choosing = false }
    AlertDialog(onDismissRequest = { choosing = false }, title = { Text(t("터미널 글꼴")) }, text = {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(t("이 기기에 설치된 고정폭 글꼴만 표시합니다. 동기화하지 않습니다."), style = MaterialTheme.typography.bodySmall)
            val list = fonts
            if (list == null) Text(t("글꼴 목록을 불러오는 중…"))
            else LazyColumn(Modifier.heightIn(max = 360.dp)) {
                item { FontChoice(default, family.isEmpty() || missing, FontFamily.Monospace) { choose("") } }
                items(list, key = { it.family }) { font ->
                    FontChoice(font.family, font.family == family, remember(font) { FontFamily(font.typeface) }) { choose(font.family) }
                }
                if (list.isEmpty()) item { Text(t("다른 고정폭 글꼴을 찾지 못했습니다."), style = MaterialTheme.typography.bodySmall) }
            }
        }
    }, confirmButton = { TextButton({ choosing = false }) { Text(t("닫기")) } })
}

/** Each family previews itself, so the choice shows the glyphs it will paint. */
@Composable private fun FontChoice(name: String, selected: Boolean, preview: FontFamily, choose: () -> Unit) {
    Row(Modifier.fillMaxWidth().selectable(selected, role = Role.RadioButton, onClick = choose).padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        RadioButton(selected, onClick = null)
        Column {
            Text(name)
            Text("0O 1lI {}[] ~/src", fontFamily = preview, style = MaterialTheme.typography.bodySmall)
        }
    }
}
