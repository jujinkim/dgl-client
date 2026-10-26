package kim.jujin.dglc.storage

import kim.jujin.dglc.*
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

/** Device-local geometry only. Never serialize runtime tabs, hosts, connections or drafts. */
class WorkspaceRepository(private val directory: File) {
    private val file = File(directory, "workspace-layout.json")
    private val json = Json { encodeDefaults = true }
    @Serializable private data class Layout(val version: Int = 1, val root: WorkspaceNode)
    @Synchronized fun read(): WorkspaceState = runCatching {
        require(file.length() in 1..16384)
        val layout = json.decodeFromString<Layout>(file.readText())
        require(layout.version == 1)
        fun validate(node: WorkspaceNode, depth: Int) {
            require(depth <= WorkspaceState.MAX_GROUPS && node.id.length in 1..80)
            if (node is WorkspaceNode.Split) {
                require(node.ratio.isFinite() && node.ratio > 0f && node.ratio < 1f)
                validate(node.first, depth + 1); validate(node.second, depth + 1)
            }
        }
        validate(layout.root, 0)
        require(layout.root.leaves().size <= WorkspaceState.MAX_GROUPS)
        require(layout.root.ids().distinct().size == layout.root.ids().size)
        WorkspaceState(layout.root)
    }.getOrElse { WorkspaceState() }
    @Synchronized fun save(root: WorkspaceNode) {
        directory.mkdirs()
        val temporary = File(directory, "workspace-layout.tmp")
        temporary.writeText(json.encodeToString(Layout.serializer(), Layout(root = root)))
        try { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING) }
        catch (_: java.nio.file.AtomicMoveNotSupportedException) { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING) }
    }
}
