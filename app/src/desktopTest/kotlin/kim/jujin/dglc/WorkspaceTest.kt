package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.storage.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import java.nio.file.Files
import kotlin.test.*

class WorkspaceTest {
    @Test fun splitMoveMergeSelectionAndEmptyGroups() {
        var state = WorkspaceState(WorkspaceNode.Leaf("first")).add("a").add("b")
        state = state.split("first", SplitAxis.Horizontal, 1200f, 800f, newId = "second")
        assertNull(state.activeTab)
        state = state.add("c").move("a", "second", 0)
        assertEquals(listOf("a", "c"), state.groups.getValue("second").tabs)
        assertEquals("a", state.activeTab)
        state = state.select("b")
        assertEquals("first", state.activeGroup)
        state = state.close("b")
        assertTrue(state.groups.getValue("first").tabs.isEmpty())
        assertEquals(2, state.groups.size)
        state = state.activate("second").merge("second")
        assertEquals(listOf("a", "c"), state.groups.getValue("first").tabs)
        assertEquals("a", state.activeTab)
        assertEquals("first", state.activeGroup)
        assertEquals(state, state.merge("first"))
    }
    @Test fun minimumSpaceGroupLimitAndResizePreserveLayout() {
        var state = WorkspaceState(WorkspaceNode.Leaf("first"))
        assertEquals(state, state.split("first", SplitAxis.Horizontal, 565f, 800f))
        assertEquals(state, state.split("first", SplitAxis.Vertical, 800f, 365f))
        state = state.split("first", SplitAxis.Horizontal, 1000f, 800f, newId = "second")
        val id = state.root.id
        state = state.resize(id, .99f, 1000f, 800f)
        assertTrue(state.fits(1000f, 800f))
        assertTrue(state.root.rectangles(1000f, 800f).getValue("second").width >= 279.99f)
        val saved = state.root
        assertFalse(state.fits(500f, 100f))
        assertEquals(saved, state.root, "temporary projection cannot change saved ratios")
        state = WorkspaceState(WorkspaceNode.Leaf("g0"))
        repeat(15) { index -> state = state.split(state.activeGroup, SplitAxis.Horizontal, 40_000_000f, 800f, newId = "g${index + 1}") }
        assertEquals(16, state.groups.size)
        assertEquals(state, state.split(state.activeGroup, SplitAxis.Vertical, 40_000_000f, 800f))
    }
    @Test fun validatedDropsAndCancelledDragsDoNotMutateState() {
        val state = WorkspaceState(WorkspaceNode.Leaf("first")).add("a").add("b")
        val before = state.copy()
        assertFalse(state.canDrop("a", WorkspaceDrop("missing"), 1200f, 800f))
        assertFalse(state.canDrop("a", WorkspaceDrop("first", DropEdge.Left), 565f, 800f))
        assertTrue(state.canDrop("a", WorkspaceDrop("first", DropEdge.Top), 1200f, 800f))
        assertEquals(before, state, "preview and cancellation have no side effects")
        val moved = state.drop("a", WorkspaceDrop("first", DropEdge.Right), 1200f, 800f)
        assertEquals(2, moved.groups.size)
        assertEquals("a", moved.activeTab)
        assertEquals(listOf("b"), moved.groups.getValue("first").tabs)
        val reordered = state.drop("b", WorkspaceDrop("first", index = 0), 1200f, 800f)
        assertEquals(listOf("b", "a"), reordered.groups.getValue("first").tabs)
    }
    @Test fun geometryOnlyPersistenceAndMalformedFilesStartEmpty() {
        val directory = Files.createTempDirectory("dgl-workspace-layout").toFile()
        try {
            val repository = WorkspaceRepository(directory)
            val state = WorkspaceState(WorkspaceNode.Leaf("first")).add("secret-host-session-draft")
                .split("first", SplitAxis.Vertical, 1200f, 800f, newId = "second")
            repository.save(state.root)
            val file = java.io.File(directory, "workspace-layout.json")
            assertFalse(file.readText().contains("secret"))
            val restored = repository.read()
            assertEquals(state.root, restored.root)
            assertTrue(restored.groups.values.all { it.tabs.isEmpty() && it.selected == null })
            for (invalid in listOf("not json", "{}", file.readText().replace("0.5", "1.5"),
                file.readText().replace("second", "first"), "x".repeat(16385))) {
                file.writeText(invalid)
                assertEquals(1, repository.read().groups.size)
            }
        } finally { directory.deleteRecursively() }
    }
    @Test fun sessionsReserveBeforeConnectAndReuseAcrossGroups() = runBlocking {
        withModel { model, created ->
            val host = HostProfile("host", "alias", "example.test", username = "test")
            model.open(host)
            val source = created.single()
            source.session.value = "\$1"; source.status.value = ConnectionStatus(Phase.Ready)
            val font = model.terminalFont(source)
            font.beginGesture(); font.scale(1.5); font.endGesture()
            assertEquals(21, font.effective)
            model.changeWorkspace { it.split(it.activeGroup, SplitAxis.Horizontal, 1200f, 800f) }
            val secondGroup = model.workspace.value.activeGroup
            val second = model.openSession(source, "\$2") as Connection
            assertNull(model.terminalFont(second).overrideSize.value)
            assertSame(second, model.openSession(source, "\$2"))
            assertEquals(2, created.size, "a pending session must already have a reserved tab")
            assertEquals(secondGroup, model.workspace.value.activeGroup)
            second.session.value = "\$2"; second.pane.value = "%9"; second.status.value = ConnectionStatus(Phase.Ready, window = "@9")
            model.selectConnection(source)
            assertSame(second, model.openSession(source, "\$2"))
            assertEquals(secondGroup, model.workspace.value.activeGroup)
            assertTrue(second.targets.isEmpty(), "session reselection must retain its window and pane")
            assertEquals(1, second.connects)
            val tab = model.tabId(second)!!
            model.changeWorkspace { it.move(tab, it.root.leaves().first().id) }
            assertSame(second, model.tabClient(tab))
            assertSame(font, model.terminalFont(source), "tab movement and renderer lookup retain connection-owned font state")
            model.removeConnection(second)
            assertEquals(1, second.closes)
            assertNull(model.tabClient(tab))
            assertEquals(2, model.workspace.value.groups.size)
            val reopened = model.openSession(source, "\$2")!!
            assertNull(model.terminalFont(reopened).overrideSize.value)
        }
    }
    @Test fun staleConnectionGenerationsRejectDelayedTargetsAndModalGates() = runBlocking {
        withModel { model, created ->
            model.open(HostProfile("host", "alias", "example.test", username = "test"))
            val source = created.single()
            source.session.value = "\$1"; source.status.value = ConnectionStatus(Phase.Ready)
            assertNull(model.openSession(source, "\$2", expectedGeneration = source.generation - 1))
            assertEquals(1, created.size)
            val pending = model.openSession(source, "\$2", "@2", "%2") as Connection
            pending.generation++
            pending.session.value = "\$2"; pending.status.value = ConnectionStatus(Phase.Ready)
            delay(100)
            assertTrue(pending.targets.isEmpty())
            val shortcuts = model.shortcutInput(pending)
            shortcuts.toggle(TerminalShortcut.Ctrl)
            assertTrue(shortcuts.modifiers.value.control)
            val a = Any(); val b = Any()
            model.blockInput(a, true);
            assertEquals(TerminalModifiers(), shortcuts.modifiers.value)
            model.blockInput(a, false)
            shortcuts.toggle(TerminalShortcut.Alt)
            model.selectConnection(source)
            assertEquals(TerminalModifiers(), shortcuts.modifiers.value)
            model.selectConnection(pending)
            shortcuts.toggle(TerminalShortcut.Shift)
            model.foreground = true; model.foreground = false
            assertEquals(TerminalModifiers(), shortcuts.modifiers.value)
            model.blockInput(a, true); model.blockInput(b, true)
            assertTrue(model.inputBlocked(a))
            model.blockInput(b, false)
            assertFalse(model.inputBlocked(a)); assertTrue(model.inputBlocked())
            model.blockInput(a, false)
            assertFalse(model.inputBlocked())
        }
    }
    @Test fun notificationsReuseSessionTabsAndObserveEveryVisibleConnection() = runBlocking {
        val visible = MutableStateFlow<Set<String>>(emptySet())
        val inbox = object : AgentInbox {
            override fun tick(sources: List<Pair<String, AgentSnapshot>>, shown: Set<String>, settings: AgentNotificationSettings): AgentInboxResult {
                visible.value = shown; return AgentInboxResult()
            }
            override fun markRead(id: String) {}
        }
        withModel(inbox) { model, created ->
            model.workspaceEnabled = true
            model.open(HostProfile("host", "alias", "example.test", username = "test"))
            val first = created.single()
            first.session.value = "\$1"; first.status.value = ConnectionStatus(Phase.Ready, window = "@1")
            val second = model.openSession(first, "\$2") as Connection
            second.session.value = "\$2"; second.status.value = ConnectionStatus(Phase.Ready, window = "@2")
            for ((client, index) in listOf(first to 1, second to 2)) {
                client.pane.value = "%$index"
                client.remote.value = RemoteState(windows = listOf(RemoteWindow("@$index", "\$$index", "work", true, "")),
                    panes = listOf(RemotePane("%$index", "@$index", 80, 24, true, "", "sh")))
                client.agents.value = AgentSnapshot(server = "server", valid = true, observed_at = System.currentTimeMillis())
            }
            model.foreground = true; model.visibleConnections = setOf(first, second)
            withTimeout(2000) { while (visible.value.size != 2) delay(10) }
            assertTrue(visible.value.any { it.endsWith(":%1") } && visible.value.any { it.endsWith(":%2") })
            model.visibleConnections = setOf(second)
            withTimeout(2000) { while (visible.value.size != 1) delay(10) }
            assertTrue(visible.value.single().endsWith(":%2"))
            val observation = AgentObservation("\$2", "@2", "%2", "run", "actor", "Codex")
            first.agents.value = first.agents.value.copy(observations = listOf(observation))
            model.selectConnection(first)
            model.openAgentObservation(first, observation)
            assertSame(second, model.selected.value)
            assertEquals(2, created.size)
            assertTrue(first.targets.isEmpty())
        }
    }
    private suspend fun withModel(inbox: AgentInbox = NoAgentInbox, test: suspend (AppModel, MutableList<Connection>) -> Unit) {
        val directory = Files.createTempDirectory("dgl-workspace-sessions").toFile()
        val created = mutableListOf<Connection>()
        val credentials = object : CredentialStore {
            override fun read(reference: String): Credential? = null
            override fun write(reference: String, credential: Credential) {}
            override fun delete(reference: String) {}
        }
        val keys = object : KeyTools {
            override fun ed25519(): GeneratedKey = error("unused")
            override fun publicKey(credential: Credential): String = error("unused")
        }
        val model = AppModel(LocalRepository(directory), credentials, "test", keys, inbox) { host, _, _, _ -> Connection(host).also { created += it } }
        try { test(model, created) } finally { model.close(); directory.deleteRecursively() }
    }
    private class Connection(override val host: HostProfile) : ClientConnection {
        override val agents = MutableStateFlow(AgentSnapshot())
        override var generation = 0L
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus())
        override val remote = MutableStateFlow(RemoteState())
        override val session = MutableStateFlow<String?>(null)
        override val pane = MutableStateFlow<String?>(null)
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        val targets = mutableListOf<List<String?>>()
        var connects = 0; var closes = 0
        override fun connect() { generation++; connects++; status.value = ConnectionStatus(Phase.Connecting) }
        override fun selectTarget(session: String, window: String?, pane: String?) { targets += listOf(session, window, pane) }
        override fun trust() {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        override fun input(bytes: ByteArray) { error("navigation cannot send input") }
        override fun bindSurface(value: TerminalSurface?) {}
        override fun unbindSurface(expected: TerminalSurface) {}
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun disconnect() { generation++; status.value = ConnectionStatus() }
        override fun close() { closes++; disconnect() }
    }
}
