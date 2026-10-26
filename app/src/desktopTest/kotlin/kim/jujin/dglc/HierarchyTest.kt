package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kotlinx.serialization.json.*
import kotlin.test.*

class HierarchyTest {
    @Test fun tabTitlesFollowActiveWindowAndPaneAndIgnoreOtherSessions() {
        val fixture = javaClass.getResourceAsStream("/migration/hierarchy-v2.json")!!.use {
            Json.parseToJsonElement(it.reader(Charsets.UTF_8).readText()).jsonArray
        }
        var remote = RemoteStateDecoder.json(fixture[3].jsonObject.getValue("remote").jsonObject)
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> w.copy(active = i != 1) },
            panes = remote.panes.mapIndexed { i, p -> p.copy(active = i != 1 && i != 3) })
        fun title() = remote.sessionTabTitle(remote.sessions[0])
        fun pane(index: Int, text: String) { remote = remote.copy(panes = remote.panes.mapIndexed { i, p -> if (i == index) p.copy(title = text) else p }) }
        assertEquals("개발 · 편집 · 작업", title())
        pane(2, "Hidden"); pane(4, "Other session")
        assertEquals("개발 · 편집 · 작업", title())
        remote = remote.copy(sessions = remote.sessions.mapIndexed { i, s -> if (i == 0) s.copy(name = " 프로젝트\n") else s },
            windows = remote.windows.mapIndexed { i, w -> if (i == 0) w.copy(name = "Editor") else w })
        pane(0, "Building 😀\u202e\n")
        assertEquals("프로젝트 · Editor · Building 😀", title())
        remote = remote.copy(panes = remote.panes.mapIndexed { i, p -> if (i < 2) p.copy(active = i == 1) else p })
        pane(1, "Review")
        assertEquals("프로젝트 · Editor · Review", title())
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> if (i < 2) w.copy(active = i == 1) else w })
        assertEquals("프로젝트 · 서버 · Hidden", title())
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> if (i == 1) w.copy(name = "프로젝트") else w })
        pane(2, " 프로젝트\u2069")
        assertEquals("프로젝트", title())
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> if (i == 1) w.copy(name = "\t\n") else w })
        pane(2, "")
        assertEquals("프로젝트", title())
        assertEquals("개발 · Other session", remote.sessionTabTitle(remote.sessions[1]))
        assertEquals("빈 세션", remote.sessionTabTitle(remote.sessions[2]))
        remote = remote.copy(windows = emptyList())
        assertEquals("프로젝트", title())
        assertEquals("\$1", remote.sessions[0].id)
        assertEquals("%2", remote.panes[1].id)
    }
    @Test fun liveTitlesFollowActivePanesWithoutChangingIdentity() {
        val fixture = javaClass.getResourceAsStream("/migration/hierarchy-v2.json")!!.use {
            Json.parseToJsonElement(it.reader(Charsets.UTF_8).readText()).jsonArray
        }
        var remote = RemoteStateDecoder.json(fixture[3].jsonObject.getValue("remote").jsonObject)
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> w.copy(active = i != 1) },
            panes = remote.panes.mapIndexed { i, p -> p.copy(active = i != 1 && i != 3) })
        fun keys() = remote.hierarchyRows(true).map { it.key to it.children.map { child -> child.key } }
        val originalKeys = keys()
        fun title() = remote.sessionLabel(remote.sessions[0]).title
        fun pane(index: Int, text: String) { remote = remote.copy(panes = remote.panes.mapIndexed { i, p -> if (i == index) p.copy(title = text) else p }) }
        assertEquals("개발 · 작업", title())
        pane(0, "Building 😀\u202e\n")
        assertEquals("개발 · Building 😀", title())
        assertEquals("편집 · Building 😀", remote.windowLabel(remote.windows[0]).title)
        remote = remote.copy(panes = remote.panes.mapIndexed { i, p -> if (i < 2) p.copy(active = i == 1) else p })
        pane(1, "Review")
        assertEquals("개발 · Review", title())
        remote = remote.copy(windows = remote.windows.mapIndexed { i, w -> if (i < 2) w.copy(active = i == 1) else w })
        pane(2, "Tests")
        assertEquals("개발 · Tests", title())
        assertEquals("편집 · Review", remote.windowLabel(remote.windows[0]).title)
        pane(4, "Other session")
        assertEquals("개발 · Other session", remote.sessionLabel(remote.sessions[1]).title)
        pane(2, "")
        assertEquals("개발", title())
        pane(2, "개발")
        assertEquals("개발", title())
        assertEquals("개발", remote.sessions[0].name)
        assertEquals("편집", remote.windows[0].name)
        assertEquals(originalKeys, keys())
    }
    @Test fun sharedHierarchyProjection() {
        val cases = javaClass.getResourceAsStream("/migration/hierarchy-v2.json")!!.use {
            Json.parseToJsonElement(it.reader(Charsets.UTF_8).readText()).jsonArray
        }
        fun flatten(rows: List<HierarchyRow>, depth: Int = 0): List<JsonElement> = rows.flatMap { r ->
            listOf(buildJsonArray {
                add(r.key); add(depth); add(r.label.title); add(r.label.detail)
                add(r.window?.let(::JsonPrimitive) ?: JsonNull); add(r.pane?.let(::JsonPrimitive) ?: JsonNull); add(r.children.isNotEmpty())
            }) + flatten(r.children, depth + 1)
        }
        cases.forEach { value ->
            val case = value.jsonObject
            val remote = RemoteStateDecoder.json(case.getValue("remote").jsonObject)
            listOf(false to "compact", true to "all").forEach { (all, key) ->
                assertEquals(case.getValue(key), JsonArray(flatten(remote.hierarchyRows(all))), "${case["name"]} / $all")
            }
            val target = remote.panes.first()
            val sid = remote.windows.first { it.id == target.window }.session
            val label = remote.targetLabel(sid, target.id)
            assertFalse(label.contains(target.id))
            assertFalse(label.contains(sid))
            assertEquals("연결됨", Phase.Ready.displayLabel())
        }
        assertEquals("select-pane -t '%12' -T '작업 ##S '\\'' 제목'", TmuxCommand.renamePane("%12", "작업 #S ' 제목").wire)
        assertFailsWith<IllegalArgumentException> { TmuxCommand.renamePane("@12", "작업") }
        assertEquals("A B C D E F G", displayTreeText("A\u0000B\u007fC\u0085D\u2028E\u202eF\u2069G"))
    }
    @Test fun panelStateSeparatesRunsOtherPanesAndOtherConnections() {
        fun event(id: String, pane: String, kind: String) = AgentEvent(id, 1, "server", id, "parent", "$1", "@1", pane, "Codex", kind, "Test", 1000)
        val question = event("question", "%1", "Question")
        val response = event("response", "%1", "Response")
        val error = event("error", "%2", "Error")
        val snapshot = AgentSnapshot(server = "server", valid = true, observed_at = 1000,
            observations = listOf(question, response, error).map { AgentObservation(it.session, it.window, it.pane, it.run, it.actor, it.agent, it) })
        val notices = listOf(AgentNotice("first", question), AgentNotice("first", error), AgentNotice("second", response))
        val first = snapshot.panelState("%1", "first", notices, 1001)
        assertEquals(2, first.observations.size)
        assertEquals(listOf(question), first.unread.map { it.event })
        assertTrue(first.label.contains("질문"))
        assertTrue(snapshot.panelState("%2", "first", notices, 1001).label.contains("오류"))
        assertEquals("상태 미확인", snapshot.panelState("%3", "first", notices, 1001).label)
        assertEquals("상태 미확인", snapshot.panelState("%1", "first", notices, 11000).label)
        assertTrue(snapshot.panelState("%1", "other", notices, 1001).unread.isEmpty())
    }

}
