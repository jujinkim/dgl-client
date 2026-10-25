package kim.jujin.rovessh.core

data class TreeLabel(val title: String, val detail: String) {
    val fullText: String get() = "$title\n$detail"
}

fun displayTreeText(value: String): String = value.map { c ->
    if (c.isISOControl() || c in '\u2028'..'\u202e' || c in '\u2066'..'\u2069') ' ' else c
}.joinToString("").trim()

fun RemoteState.sessionLabel(session: RemoteSession): TreeLabel {
    val children = windows.filter { it.session == session.id }
    val count = panes.count { p -> children.any { it.id == p.window } }
    return TreeLabel(displayTreeText(session.name), "창 ${children.size}개 · 패널 ${count}개")
}

fun RemoteState.windowLabel(window: RemoteWindow): TreeLabel {
    val children = panes.filter { it.window == window.id }
    val active = children.firstOrNull { it.active }
    val name = displayTreeText(window.name)
    val session = sessions.firstOrNull { it.id == window.session }?.let { displayTreeText(it.name) } ?: window.session
    val command = active?.let { displayTreeText(it.command) }.orEmpty()
    val process = if (command.isEmpty()) "" else " / $command"
    return TreeLabel(active?.let { displayTreeText(it.title) }.orEmpty().ifEmpty { name },
        "$session · ${window.index}: $name$process · 패널 ${children.size}개")
}

fun RemotePane.treeLabel(): TreeLabel {
    val command = displayTreeText(command)
    val mode = displayTreeText(mode).let { if (it.isEmpty()) "" else " · $it" }
    return TreeLabel(displayTreeText(title).ifEmpty { "$command $id".trim() }, "$id · $command · ${width}×${height}$mode")
}
