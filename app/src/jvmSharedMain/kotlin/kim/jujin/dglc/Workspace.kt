package kim.jujin.dglc

import kotlinx.serialization.Serializable
import java.util.UUID

@Serializable enum class SplitAxis { Horizontal, Vertical }
@Serializable sealed class WorkspaceNode {
    abstract val id: String
    @Serializable data class Leaf(override val id: String = UUID.randomUUID().toString()) : WorkspaceNode()
    @Serializable data class Split(override val id: String = UUID.randomUUID().toString(), val axis: SplitAxis,
        val ratio: Float = .5f, val first: WorkspaceNode, val second: WorkspaceNode) : WorkspaceNode()
}

data class WorkspaceGroup(val tabs: List<String> = emptyList(), val selected: String? = null)
data class WorkspaceRect(val x: Float, val y: Float, val width: Float, val height: Float) {
    fun contains(px: Float, py: Float) = px >= x && py >= y && px < x + width && py < y + height
}
enum class DropEdge { Center, Left, Right, Top, Bottom }
data class WorkspaceDrop(val group: String, val edge: DropEdge = DropEdge.Center, val index: Int = Int.MAX_VALUE)

/** Pure, immutable state. A drag only mutates this state when a validated drop is committed. */
data class WorkspaceState(val root: WorkspaceNode = WorkspaceNode.Leaf(),
    val groups: Map<String, WorkspaceGroup> = root.leaves().associate { it.id to WorkspaceGroup() },
    val activeGroup: String = root.leaves().first().id) {
    val activeTab get() = groups[activeGroup]?.selected
    fun groupOf(tab: String) = groups.entries.firstOrNull { tab in it.value.tabs }?.key
    fun activate(group: String): WorkspaceState = if (group in groups) copy(activeGroup = group) else this
    fun select(tab: String): WorkspaceState {
        val group = groupOf(tab) ?: return this
        return copy(groups = groups + (group to groups.getValue(group).copy(selected = tab)), activeGroup = group)
    }
    fun add(tab: String): WorkspaceState {
        if (groupOf(tab) != null) return select(tab)
        val group = groups.getValue(activeGroup)
        return copy(groups = groups + (activeGroup to group.copy(tabs = group.tabs + tab, selected = tab)))
    }
    fun close(tab: String): WorkspaceState {
        val id = groupOf(tab) ?: return this
        val group = groups.getValue(id)
        val index = group.tabs.indexOf(tab)
        val tabs = group.tabs - tab
        return copy(groups = groups + (id to WorkspaceGroup(tabs,
            if (group.selected == tab) tabs.getOrNull(index.coerceAtMost(tabs.lastIndex)) else group.selected)))
    }
    fun move(tab: String, destination: String, index: Int = Int.MAX_VALUE): WorkspaceState {
        val source = groupOf(tab) ?: return this
        if (destination !in groups) return this
        val removed = close(tab)
        val group = removed.groups.getValue(destination)
        val tabs = group.tabs.toMutableList().apply { add(index.coerceIn(0, size), tab) }
        return removed.copy(groups = removed.groups + (destination to WorkspaceGroup(tabs, tab)), activeGroup = destination)
    }
    fun fits(width: Float, height: Float): Boolean = root.rectangles(width, height).values.all {
        it.width >= MIN_WIDTH - .01f && it.height >= MIN_HEIGHT - .01f
    }
    fun split(group: String, axis: SplitAxis, width: Float, height: Float, before: Boolean = false,
        newId: String = UUID.randomUUID().toString()): WorkspaceState {
        if (groups.size >= MAX_GROUPS || group !in groups || newId in root.ids()) return this
        val leaf = WorkspaceNode.Leaf(newId)
        val old = WorkspaceNode.Leaf(group)
        val branch = WorkspaceNode.Split(axis = axis, first = if (before) leaf else old, second = if (before) old else leaf)
        val candidate = copy(root = root.replace(group, branch), groups = groups + (newId to WorkspaceGroup()), activeGroup = newId)
        return candidate.takeIf { it.fits(width, height) } ?: this
    }
    fun resize(split: String, ratio: Float, width: Float, height: Float): WorkspaceState {
        if (!ratio.isFinite()) return this
        val node = root.find(split) as? WorkspaceNode.Split ?: return this
        val rect = root.nodeRectangles(width, height)[split] ?: return this
        val firstMin = node.first.minimum(); val secondMin = node.second.minimum()
        val extent = (if (node.axis == SplitAxis.Horizontal) rect.width else rect.height) - DIVIDER
        val low = (if (node.axis == SplitAxis.Horizontal) firstMin.first else firstMin.second) / extent
        val high = 1f - (if (node.axis == SplitAxis.Horizontal) secondMin.first else secondMin.second) / extent
        if (low > high || !low.isFinite() || !high.isFinite()) return this
        val candidate = copy(root = root.replace(split, node.copy(ratio = ratio.coerceIn(low, high))))
        return candidate.takeIf { it.fits(width, height) } ?: this
    }
    /** Remove a leaf and transfer its tabs to the closest leaf of its sibling subtree. */
    fun merge(group: String): WorkspaceState {
        val parent = root.parentOf(group) ?: return this
        val sibling = if (parent.first.id == group) parent.second else parent.first
        val destination = if (parent.first.id == group) sibling.leaves().first().id else sibling.leaves().last().id
        val source = groups.getValue(group); val target = groups.getValue(destination)
        return copy(root = root.replace(parent.id, sibling), groups = (groups - group) +
            (destination to WorkspaceGroup(target.tabs + source.tabs, if (activeGroup == group) source.selected ?: target.selected else target.selected)),
            activeGroup = if (activeGroup == group) destination else activeGroup)
    }
    fun drop(tab: String, target: WorkspaceDrop, width: Float, height: Float): WorkspaceState {
        if (groupOf(tab) == null || target.group !in groups) return this
        if (target.edge == DropEdge.Center) return move(tab, target.group, target.index)
        val axis = if (target.edge in listOf(DropEdge.Left, DropEdge.Right)) SplitAxis.Horizontal else SplitAxis.Vertical
        val next = split(target.group, axis, width, height, target.edge in listOf(DropEdge.Left, DropEdge.Top))
        return if (next == this) this else next.move(tab, next.activeGroup)
    }
    fun canDrop(tab: String, target: WorkspaceDrop, width: Float, height: Float): Boolean =
        groupOf(tab) != null && target.group in groups && (target.edge == DropEdge.Center || drop(tab, target, width, height) != this)

    companion object {
        const val MAX_GROUPS = 16
        const val MIN_WIDTH = 280f
        const val MIN_HEIGHT = 180f
        const val DIVIDER = 6f
    }
}

fun WorkspaceNode.leaves(): List<WorkspaceNode.Leaf> = when (this) {
    is WorkspaceNode.Leaf -> listOf(this)
    is WorkspaceNode.Split -> first.leaves() + second.leaves()
}
fun WorkspaceNode.ids(): List<String> = when (this) {
    is WorkspaceNode.Leaf -> listOf(id)
    is WorkspaceNode.Split -> listOf(id) + first.ids() + second.ids()
}
private fun WorkspaceNode.find(id: String): WorkspaceNode? = if (this.id == id) this else when (this) {
    is WorkspaceNode.Leaf -> null
    is WorkspaceNode.Split -> first.find(id) ?: second.find(id)
}
private fun WorkspaceNode.parentOf(id: String): WorkspaceNode.Split? = when (this) {
    is WorkspaceNode.Leaf -> null
    is WorkspaceNode.Split -> if (first.id == id || second.id == id) this else first.parentOf(id) ?: second.parentOf(id)
}
private fun WorkspaceNode.replace(id: String, replacement: WorkspaceNode): WorkspaceNode = if (this.id == id) replacement else when (this) {
    is WorkspaceNode.Leaf -> this
    is WorkspaceNode.Split -> copy(first = first.replace(id, replacement), second = second.replace(id, replacement))
}
private fun WorkspaceNode.minimum(): Pair<Float, Float> = when (this) {
    is WorkspaceNode.Leaf -> WorkspaceState.MIN_WIDTH to WorkspaceState.MIN_HEIGHT
    is WorkspaceNode.Split -> {
        val a = first.minimum(); val b = second.minimum()
        if (axis == SplitAxis.Horizontal) a.first + b.first + WorkspaceState.DIVIDER to maxOf(a.second, b.second)
        else maxOf(a.first, b.first) to a.second + b.second + WorkspaceState.DIVIDER
    }
}
fun WorkspaceNode.nodeRectangles(width: Float, height: Float): Map<String, WorkspaceRect> {
    val result = linkedMapOf<String, WorkspaceRect>()
    fun visit(node: WorkspaceNode, rect: WorkspaceRect) {
        result[node.id] = rect
        if (node is WorkspaceNode.Split) {
            if (node.axis == SplitAxis.Horizontal) {
                val first = (rect.width - WorkspaceState.DIVIDER).coerceAtLeast(0f) * node.ratio
                visit(node.first, rect.copy(width = first))
                visit(node.second, rect.copy(x = rect.x + first + WorkspaceState.DIVIDER, width = (rect.width - first - WorkspaceState.DIVIDER).coerceAtLeast(0f)))
            } else {
                val first = (rect.height - WorkspaceState.DIVIDER).coerceAtLeast(0f) * node.ratio
                visit(node.first, rect.copy(height = first))
                visit(node.second, rect.copy(y = rect.y + first + WorkspaceState.DIVIDER, height = (rect.height - first - WorkspaceState.DIVIDER).coerceAtLeast(0f)))
            }
        }
    }
    visit(this, WorkspaceRect(0f, 0f, width, height))
    return result
}
fun WorkspaceNode.rectangles(width: Float, height: Float) = nodeRectangles(width, height).filterKeys { id -> leaves().any { it.id == id } }
