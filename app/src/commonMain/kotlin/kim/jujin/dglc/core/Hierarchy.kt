package kim.jujin.dglc.core

data class TreeLabel(val title: String, val detail: String) {
    val fullText: String get() = "$title\n$detail"
}
fun displayTreeText(value: String): String = value.map { c ->
    if (c.isISOControl() || c in '\u2028'..'\u202e' || c in '\u2066'..'\u2069') ' ' else c
}.joinToString("").trim()
private fun named(value: String, fallback: String) = displayTreeText(value).ifEmpty { fallback }
private fun RemoteState.liveTitle(name: String, fallback: String, window: RemoteWindow?): String {
    val base = named(name, fallback)
    val title = window?.let { w -> panes.firstOrNull { it.window == w.id && it.active } }?.title?.let(::displayTreeText).orEmpty()
    return if (title.isEmpty() || title == base) base else "$base · $title"
}
fun RemoteState.sessionLabel(session: RemoteSession): TreeLabel {
    val children = windows.filter { it.session == session.id }
    val count = panes.count { p -> children.any { it.id == p.window } }
    return TreeLabel(liveTitle(session.name, t("이름 없는 세션"), children.firstOrNull { it.active }), tr("창 {0}개 · 패널 {1}개", children.size, count))
}
fun RemoteState.windowLabel(window: RemoteWindow) = TreeLabel(liveTitle(window.name, t("이름 없는 창"), window),
    tr("창 {0} · 패널 {1}개", window.index, panes.count { it.window == window.id }))
fun RemotePane.treeLabel(): TreeLabel {
    val command = named(command, t("프로세스 미확인"))
    val mode = displayTreeText(mode).let { if (it.isEmpty()) "" else " · $it" }
    return TreeLabel(named(title, command), "$command · ${width}×${height}$mode")
}
enum class TreeKind { Session, Window, Pane }
data class HierarchyRow(val kind: TreeKind, val key: String, val session: String,
    val window: String?, val pane: String?, val label: TreeLabel, val children: List<HierarchyRow>) {
    fun selected(remote: RemoteState, session: String?, pane: String?) = this.session == session && when {
        this.pane != null -> this.pane == pane
        window != null -> remote.panes.any { it.id == pane && it.window == window }
        else -> true
    }
}
/** Display-only projection: never changes selection, expansion, or terminal input. */
fun RemoteState.hierarchyRows(showAll: Boolean): List<HierarchyRow> = sessions.map { s ->
    val windows = windows.filter { it.session == s.id }
    val children = windows.map { w ->
        HierarchyRow(TreeKind.Window, "${s.id}/${w.id}", s.id, w.id, null, windowLabel(w), emptyList())
    }
    if (!showAll && children.size == 1) {
        val w = children.single()
        HierarchyRow(TreeKind.Session, s.id, s.id, w.window, null,
            sessionLabel(s).copy(detail = "${windows.single().index}: ${named(windows.single().name, t("이름 없는 창"))} · " + tr("패널 {0}개", panes.count { it.window == w.window })), emptyList())
    } else HierarchyRow(TreeKind.Session, s.id, s.id, null, null, sessionLabel(s), children)
}

fun Phase.displayLabel(): String = when (this) {
    Phase.Disconnected -> t("연결 안 됨"); Phase.Connecting -> t("연결 중"); Phase.TrustRequired -> t("호스트키 확인"); Phase.Authenticating -> t("인증 중")
    Phase.Selecting -> t("세션 선택"); Phase.Restoring -> t("터미널 복원 중"); Phase.Ready -> t("연결됨"); Phase.Reconnecting -> t("재접속 중")
    Phase.MissingSession -> t("이전 세션 없음"); Phase.Failed -> t("연결 실패"); Phase.Shell -> t("연결됨 · 일반 SSH")
}
fun RemoteState.targetLabel(session: String?, pane: String?): String = buildList {
    sessions.firstOrNull { it.id == session }?.let { add(named(it.name, t("이름 없는 세션"))) }
    panes.firstOrNull { it.id == pane }?.let { p ->
        windows.firstOrNull { it.id == p.window && it.session == session }?.let { add(named(it.name, t("이름 없는 창"))) }
        add(p.treeLabel().title)
    }
}.joinToString(" · ")
