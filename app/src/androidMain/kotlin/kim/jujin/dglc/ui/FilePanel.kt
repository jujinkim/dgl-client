package kim.jujin.dglc.ui

import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*

@Composable internal actual fun PlatformFilePanel(files: FileTabs, key: FileTabKey, state: FileTabState, returnLabel: String) {
    val path = state.listing.path
    var uploadTo by rememberSaveable { mutableStateOf("") }
    var downloadFrom by rememberSaveable { mutableStateOf(arrayListOf<String>()) }
    val uploadFiles = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) files.copy(key, FileCopy(true, uris.map(Uri::toString), uploadTo))
    }
    val download = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let { files.copy(key, FileCopy(false, downloadFrom, it.toString())) }
    }
    val goBack = { files.select(null) }
    BackHandler(onBack = goBack)
    Column(Modifier.fillMaxSize().padding(vertical = 8.dp)) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            BorderedButton(goBack) { Text(returnLabel) }
            Column(Modifier.weight(1f).padding(start = 8.dp)) {
                Text(state.title, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (path.isNotEmpty()) Text(path, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.StartEllipsis)
            }
            TextButton({ files.close(key) }) { Text(t("common.close")) }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton({ files.browse(key, path.substringBeforeLast('/').ifEmpty { "/" }) }, enabled = path.isNotEmpty() && path != "/") { Text(t("files.up")) }
            TextButton({ files.browse(key, state.browsePath ?: path.takeIf(String::isNotEmpty)) }) { Text(t("common.refresh")) }
            TextButton({ uploadTo = path; uploadFiles.launch(arrayOf("*/*")) }, enabled = path.isNotEmpty()) { Text(t("files.upload")) }
            TextButton({ downloadFrom = ArrayList(state.selected.map { path.trimEnd('/') + "/" + it }); download.launch(null) },
                enabled = state.selected.isNotEmpty()) { Text(t("files.download")) }
        }
        if (state.loading) LinearProgressIndicator(Modifier.fillMaxWidth())
        Column(Modifier.padding(horizontal = 8.dp)) {
            state.listing.fallback?.let { Text(tr("files.fallback", it)) }
            if (state.loading) state.browsePath?.let { Text(tr("files.loading_path", it)) }
            state.error?.let { error ->
                Text(state.browsePath?.let { tr("files.path_error", it, error) } ?: error, color = MaterialTheme.colorScheme.error)
            }
            if (path.isNotEmpty() && state.listing.entries.isEmpty()) Text(t("files.empty"))
            state.fingerprint?.let { fp ->
                Text(fp)
                TextButton({ files.trust(key) }) { Text(t("workspace.trust_after_verify")) }
            }
        }
        CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides 36.dp) {
            LazyColumn(Modifier.fillMaxWidth().weight(1f)) {
                items(state.listing.entries, key = { it.name }) { entry ->
                    val openFolder = if (entry.directory) Modifier.clickable(role = Role.Button) {
                        files.browse(key, path.trimEnd('/') + "/" + entry.name)
                    } else Modifier
                    Row(Modifier.fillMaxWidth().then(openFolder).padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        if (entry.regular) {
                            Checkbox(entry.name in state.selected, { files.toggle(key, entry.name) },
                                modifier = Modifier.width(44.dp).semantics { contentDescription = entry.name })
                        } else Spacer(Modifier.width(44.dp))
                        Text(if (entry.directory) entry.name + "/" else entry.name,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (entry.directory) MaterialTheme.colorScheme.primary else LocalContentColor.current)
                    }
                }
            }
        }
        LazyColumn(Modifier.heightIn(max = 160.dp).fillMaxWidth(), contentPadding = PaddingValues(horizontal = 8.dp)) {
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
