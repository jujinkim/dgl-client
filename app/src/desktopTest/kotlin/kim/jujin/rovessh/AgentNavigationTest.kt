package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.LocalRepository
import kotlinx.coroutines.flow.MutableStateFlow
import java.nio.file.Files
import kotlin.test.*

class AgentNavigationTest {
    private class Connection(id: String, hostId: String = id) : ClientConnection {
        override val host = HostProfile(hostId, id, "example.test", username = "test")
        override val agents = MutableStateFlow(AgentSnapshot())
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready))
        override val remote = MutableStateFlow(RemoteState())
        override val session = MutableStateFlow<String?>("\$1")
        override val pane = MutableStateFlow<String?>("%1")
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        val targets = mutableListOf<List<String?>>()
        override fun selectTarget(session: String, window: String?, pane: String?) { targets += listOf(session, window, pane) }
        var connects = 0
        var disconnects = 0
        override fun connect() { connects++ }
        override fun input(bytes: ByteArray) { error("navigation must not send input") }
        override fun trust() { error("navigation must not trust") }
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        override fun bindSurface(value: TerminalSurface?) {}
        override fun unbindSurface(expected: TerminalSurface) {}
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun disconnect() { disconnects++ }
        override fun close() {}
    }

    @Test fun detailsSelectTheConnectionAndRejectARecreatedPane() {
        val directory = Files.createTempDirectory("dgl-agent-navigation").toFile()
        val credentials = object : CredentialStore {
            override fun read(reference: String): Credential? = null
            override fun write(reference: String, credential: Credential) { error("unexpected credential write") }
            override fun delete(reference: String) { error("unexpected credential delete") }
        }
        val keys = object : KeyTools {
            override fun ed25519(): GeneratedKey = error("unexpected key generation")
            override fun publicKey(credential: Credential): String = error("unexpected key access")
        }
        val model = AppModel(LocalRepository(directory), credentials, "test", keys) { _, _, _, _ -> error("unexpected new connection") }
        try {
            val first = Connection("first", "same-host")
            val second = Connection("second", "same-host")
            model.connections.value = listOf(first, second)
            model.selected.value = second
            model.reconnectSelected(first, "same-host")
            model.disconnectSelected(first, "same-host")
            model.reconnectSelected(second, "wrong-host")
            assertEquals(0, first.connects + first.disconnects + second.connects)
            model.reconnectSelected(second, "same-host")
            assertEquals(1, second.connects)
            second.status.value = second.status.value.copy(canRetryNow = true)
            model.reconnectSelected(second, "same-host")
            assertEquals(1, second.connects)
            model.disconnectSelected(second, "same-host")
            assertEquals(1, second.disconnects)
            assertEquals(0, first.connects + first.disconnects)
            second.status.value = ConnectionStatus(Phase.Ready)
            second.connects = 0
            val observation = AgentObservation("\$2", "@2", "%2", "original-run", "parent", "Claude")
            second.agents.value = AgentSnapshot(server = "server", valid = true, observed_at = System.currentTimeMillis(), observations = listOf(observation))
            model.connections.value = listOf(first, second)
            model.selected.value = first
            model.openAgentObservation(second, observation)
            assertSame(second, model.selected.value)
            assertEquals<List<List<String?>>>(listOf(listOf("\$2", "@2", "%2")), second.targets)
            assertTrue(first.targets.isEmpty())
            assertEquals(0, first.connects + second.connects)
            second.agents.value = second.agents.value.copy(observations = listOf(observation.copy(run = "recreated-run")))
            model.openAgentObservation(second, observation)
            assertEquals(1, second.targets.size)
            assertTrue(model.agentNavigationError.value!!.contains("종료"))
        } finally {
            model.close()
            directory.deleteRecursively()
        }
    }
}
