package kim.jujin.dglc

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import kotlinx.serialization.json.Json
import uniffi.dgl_core.Documents
import java.io.IOException
import java.util.UUID

/** SAF document IDs can be opaque and cannot be joined as file paths. */
internal class AndroidDocuments(private val resolver: ContentResolver) : Documents {
    private fun document(id: String): Uri = Uri.parse(id).let { uri ->
        if (DocumentsContract.isTreeUri(uri) && uri.pathSegments.size == 2)
            DocumentsContract.buildDocumentUriUsingTree(uri, DocumentsContract.getTreeDocumentId(uri))
        else uri
    }
    private fun entry(uri: Uri): FileEntry {
        val columns = arrayOf(Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE, Document.COLUMN_SIZE, Document.COLUMN_FLAGS)
        return resolver.query(uri, columns, null, null, null)?.use { c ->
            if (!c.moveToFirst()) throw IOException("Missing document")
            val mime = c.getString(1)
            val directory = mime == Document.MIME_TYPE_DIR
            val regular = !directory && !mime.isNullOrBlank() && !mime.startsWith("inode/") &&
                (c.getInt(3) and Document.FLAG_VIRTUAL_DOCUMENT) == 0
            val size = if (c.isNull(2)) null else c.getLong(2).takeIf { it >= 0 }
            FileEntry(c.getString(0), directory, regular, size)
        } ?: throw IOException("Document unavailable")
    }
    override fun entryJson(id: String) = Json.encodeToString(FileEntry.serializer(), entry(document(id)))
    private fun entries(id: String): List<Pair<String, String>> {
        val parent = document(id)
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(parent, DocumentsContract.getDocumentId(parent))
        return resolver.query(children, arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
            buildList {
                while (c.moveToNext()) add(c.getString(1) to DocumentsContract.buildDocumentUriUsingTree(parent, c.getString(0)).toString())
            }
        } ?: throw IOException("Folder unavailable")
    }
    override fun find(parent: String, name: String) = entries(parent).firstOrNull { it.first == name }?.second
    override fun createFile(parent: String, name: String): String =
        DocumentsContract.createDocument(resolver, document(parent), "application/octet-stream", name)?.toString()
            ?: throw IOException("Cannot create $name")
    override fun openFd(id: String, write: Boolean): Int =
        resolver.openFileDescriptor(document(id), if (write) "w" else "r")?.use { it.detachFd() }
            ?: throw IOException("Cannot open document")
    private fun rename(uri: Uri, name: String): Uri =
        DocumentsContract.renameDocument(resolver, uri, name) ?: throw IOException("Cannot rename $name")
    override fun publish(temp: String, parent: String, name: String, existing: String?) {
        synchronized(this) {
            if (find(parent, name) != existing) throw IOException("Destination changed: $name")
            val backup = existing?.let { rename(document(it), ".dgl-${UUID.randomUUID()}.bak") }
            try {
                val published = rename(document(temp), name)
                if (entry(published).name != name) {
                    discard(published.toString())
                    throw IOException("Provider declined the name $name")
                }
            } catch (e: Exception) {
                backup?.let { rename(it, name) }
                throw e
            }
            backup?.let { discard(it.toString()) }
        }
    }
    override fun discard(temp: String) {
        if (!DocumentsContract.deleteDocument(resolver, document(temp))) throw IOException("Cannot remove temporary document")
    }
}
