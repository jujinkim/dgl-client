package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kotlinx.serialization.json.*
import kotlin.test.*

class HierarchyTest {
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
