package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kotlinx.serialization.json.*
import kotlin.test.*

class HierarchyTest {
    @Test fun nativeContractAndHierarchyLabels() {
        val fixture = javaClass.getResourceAsStream("/migration/hierarchy-v1.json")!!.use {
            Json.parseToJsonElement(it.reader(Charsets.UTF_8).readText()).jsonObject
        }
        val remote = RemoteStateDecoder.json(fixture.getValue("remote").jsonObject)
        val expected = fixture.getValue("expected").jsonObject
        mapOf("sessions" to remote.sessions.map { remote.sessionLabel(it) },
            "windows" to remote.windows.map { remote.windowLabel(it) },
            "panes" to remote.panes.map { it.treeLabel() }).forEach { (kind, labels) ->
            labels.zip(expected.getValue(kind).jsonArray).forEach { (label, row) ->
                assertEquals(row.jsonArray[0].jsonPrimitive.content, label.title)
                assertEquals(row.jsonArray[1].jsonPrimitive.content, label.detail)
                assertEquals("${label.title}\n${label.detail}", label.fullText)
            }
        }
        val switched = remote.copy(panes = remote.panes.map { it.copy(active = it.id == "%1") })
        assertEquals("비활성 패널", switched.windowLabel(switched.windows[0]).title)
        val empty = switched.copy(panes = switched.panes.map { it.copy(title = "") })
        assertEquals("실제 창 이름", empty.windowLabel(empty.windows[0]).title)
        assertEquals("sh %1", empty.panes[0].treeLabel().title)
    }
}
