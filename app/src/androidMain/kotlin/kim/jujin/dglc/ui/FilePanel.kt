package kim.jujin.dglc.ui

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*

@Composable internal actual fun PlatformFilePanel(files: FileTabs, key: FileTabKey, state: FileTabState) {
    val path = state.listing.path
    var uploadTo by rememberSaveable { mutableStateOf("") }
    var downloadFrom by rememberSaveable { mutableStateOf(arrayListOf<String>()) }
    val uploadFiles = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) files.copy(key, FileCopy(true, uris.map(Uri::toString), uploadTo))
    }
    val uploadFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let { files.copy(key, FileCopy(true, listOf(it.toString()), uploadTo)) }
    }
    val download = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let { files.copy(key, FileCopy(false, downloadFrom, it.toString())) }
    }
    BackHandler { files.select(null) }
    Column(Modifier.fillMaxSize().padding(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(path.ifEmpty { state.title }, Modifier.weight(1f))
            TextButton({ files.close(key) }) { Text(t("common.close")) }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
            TextButton({ files.browse(key, path.substringBeforeLast('/').ifEmpty { "/" }) }, enabled = path.isNotEmpty() && !state.loading) { Text(t("files.up")) }
            TextButton({ files.browse(key, path.takeIf(String::isNotEmpty)) }, enabled = !state.loading) { Text(t("common.refresh")) }
            TextButton({ uploadTo = path; uploadFiles.launch(arrayOf("*/*")) }, enabled = path.isNotEmpty()) { Text(t("files.upload")) }
            TextButton({ uploadTo = path; uploadFolder.launch(null) }, enabled = path.isNotEmpty()) { Text(t("files.upload_folder")) }
            TextButton({ downloadFrom = ArrayList(state.selected.map { path.trimEnd('/') + "/" + it }); download.launch(null) },
                enabled = state.selected.isNotEmpty()) { Text(t("files.download")) }
        }
        if (state.loading) LinearProgressIndicator(Modifier.fillMaxWidth())
        state.listing.fallback?.let { Text(tr("files.fallback", it)) }
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        state.fingerprint?.let { fp ->
            Text(fp)
            TextButton({ files.trust(key) }) { Text(t("workspace.trust_after_verify")) }
        }
        LazyColumn(Modifier.weight(1f)) {
            items(state.listing.entries, key = { it.name }) { entry ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(entry.name in state.selected, { files.toggle(key, entry.name) }, enabled = entry.directory || entry.regular)
                    if (entry.directory) TextButton({ files.browse(key, path.trimEnd('/') + "/" + entry.name) }) { Text(entry.name + "/") }
                    else Text(entry.name)
                }
            }
        }
        LazyColumn(Modifier.heightIn(max = 160.dp)) {
            items(state.transfers.entries.toList().asReversed(), key = { it.key }) { (id, p) ->
                Column {
                    Text(p.error ?: if (p.done) tr("files.completed", p.completed.toString()) else "${p.item} · ${p.bytes} / ${p.size ?: "…"} B")
                    Row {
                        p.conflict?.let { path -> TextButton({ files.overwrite(key, id, path) }) { Text(tr("files.overwrite", path)) } }
                        if (!p.done) TextButton({ files.cancel(key, id) }) { Text(t("common.cancel")) }
                        if (p.error != null) TextButton({ files.retry(key, id) }) { Text(t("files.retry")) }
                    }
                }
            }
        }
    }
}
