package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.*
import kim.jujin.rovessh.tmux.ControlClient
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.junit.Assume.assumeTrue
import java.io.File
import kotlin.test.*

class IntegrationTest {
    private fun host(): HostProfile {
        assumeTrue("Run scripts/integration.py to enable real SSH tests", System.getenv("DGL_TEST_PORT") != null)
        return HostProfile("integration", "Isolated test", "127.0.0.1", System.getenv("DGL_TEST_PORT").toInt(), System.getenv("DGL_TEST_USER"), AuthKind.PrivateKey)
    }
    private fun credential(variable: String = "DGL_TEST_KEY_FILE", passphrase: String = "") = Credential(privateKey = File(System.getenv(variable)).readText(), passphrase = passphrase)
    private suspend fun connected() = SshjTransport().also { it.connect(host(), credential(), System.getenv("DGL_TEST_FINGERPRINT")) }

    @Test fun plainSshShellAndHostileNamesAndErrorRecovery() = runBlocking<Unit> {
        val ssh = connected(); val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var client: ControlClient? = null
        try {
            val shell = ssh.openShell(Size())
            shell.resize(Size(90, 30))
            shell.write("printf 'PLAIN_%s\\n' SSH_OK\r".encodeToByteArray())
            var output = ""
            withTimeout(5000) { while (!output.contains("PLAIN_SSH_OK")) output += shell.read()!!.decodeToString() }
            shell.close()
            val name = "bootstrap-${System.nanoTime()};"
            ssh.execute(TmuxCommand.newSession(name).shellWire)
            val session = RemoteStateDecoder.sessions(ssh.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            client = ControlClient(ssh.openControl(session), scope, {})
            client.start()
            for (value in listOf("#(printf unintended)", "#{session_name}", "quote'| pipe ; dollar\$ value")) {
                client.command(TmuxCommand.renameSession(session, value)); client.refresh()
                assertEquals(value, client.state.value.sessions.single { it.id == session }.name)
            }
            assertFailsWith<RemoteCommandException> { client.snapshot("%9999999") }
            client.refresh()
            assertTrue(client.state.value.sessions.any { it.id == session })
            client.close(); client = null
            ssh.execute("tmux " + TmuxCommand.killSession(session).wire)
        } finally { client?.close(); ssh.close(); scope.cancel() }
    }

    @Test fun hostTrustMismatchAndKeyAuthentication() = runBlocking<Unit> {
        val host = host()
        val ssh = SshjTransport()
        try {
            val unknown = assertFailsWith<UnknownHostKey> { ssh.connect(host, credential(), null) }
            assertEquals(System.getenv("DGL_TEST_FINGERPRINT"), unknown.fingerprint)
            assertFailsWith<ChangedHostKey> { ssh.connect(host, credential(), "SHA256:synthetic-mismatch") }
            for ((path, pass) in listOf("DGL_TEST_KEY_FILE" to "", "DGL_TEST_RSA_FILE" to "", "DGL_TEST_ENCRYPTED_FILE" to System.getenv("DGL_TEST_PASSPHRASE"))) {
                ssh.connect(host, credential(path, pass), unknown.fingerprint)
                assertTrue(supportsTmux(ssh.execute("tmux -V").decodeToString()))
                ssh.close()
            }
            assertFailsWith<ClientFailure> { ssh.connect(host, credential("DGL_TEST_ENCRYPTED_FILE", "incorrect synthetic passphrase"), unknown.fingerprint) }
        } finally { ssh.close() }
    }

    @Test fun actualSshControlInputExternalChangesAndProcessPreservation() = runBlocking<Unit> {
        val ssh = connected()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val output = Channel<PaneOutput>(128)
        var client: ControlClient? = null
        try {
            val name = "dgl-test-${System.nanoTime()}"
            ssh.execute("tmux " + TmuxCommand.newSession(name).wire)
            val sessionBytes = ssh.execute("tmux " + TmuxCommand.sessions.wire)
            val sessions = RemoteStateDecoder.sessions(sessionBytes.decodeToString().trimEnd().lines().map { it.encodeToByteArray() })
            val session = sessions.single { it.name == name }.id
            client = ControlClient(ssh.openControl(session), scope, { output.send(it) })
            client.start()
            val initial = client.state.value.panes.single { p -> client.state.value.windows.any { it.id == p.window && it.session == session } }.id
            client.command(TmuxCommand.clientSize(Size(100, 30)))
            val snapshot = client.snapshot(initial)
            assertTrue(snapshot.isNotEmpty())
            client.input(initial, "printf 'DGL_UNICODE_한글_日本語\\n'\r".encodeToByteArray())
            val seen = java.io.ByteArrayOutputStream()
            withTimeout(5000) { while (!seen.toByteArray().decodeToString().contains("DGL_UNICODE_한글_日本語")) seen.write(output.receive().bytes) }
            val pidBefore = ssh.execute("tmux display-message -p -t '$initial' '#{pane_pid}'").decodeToString().trim()
            ssh.execute("tmux " + TmuxCommand.split(initial, true).wire)
            withTimeout(5000) { while (client.state.value.panes.size < 2) delay(20) }
            client.command(TmuxCommand.renameSession(session, "safe'; kill-server; #"))
            client.refresh()
            assertTrue(client.state.value.sessions.any { it.id == session && it.name == "safe'; kill-server; #" })
            client.close(); client = null; ssh.close()
            val next = connected()
            try {
                val pidAfter = next.execute("tmux display-message -p -t '$initial' '#{pane_pid}'").decodeToString().trim()
                assertEquals(pidBefore, pidAfter)
                val reattached = ControlClient(next.openControl(session), scope, {})
                reattached.start()
                assertTrue(reattached.snapshot(initial).decodeToString().contains("DGL_UNICODE_한글_日本語"))
                reattached.close()
                assertTrue(next.execute("tmux list-sessions").isNotEmpty())
                next.execute("tmux " + TmuxCommand.killSession(session).wire)
            } finally { next.close() }
        } finally { client?.close(); ssh.close(); scope.cancel() }
    }

    @Test fun simultaneousClientsSnapshotDuringOutputAndSessionDeletion() = runBlocking<Unit> {
        val ssh = connected(); val other = connected()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var first: ControlClient? = null; var second: ControlClient? = null
        try {
            val name = "dgl-stream-${System.nanoTime()}"
            ssh.execute("tmux " + TmuxCommand.newSession(name).wire)
            val session = RemoteStateDecoder.sessions(ssh.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            val firstStream = ssh.openControl(session)
            val failure = CompletableDeferred<Exception>()
            val renderer = HeadlessTerminal()
            first = ControlClient(firstStream, scope, { renderer.feed(it.bytes) }, onSnapshot = { _, bytes -> renderer.feed(bytes) }, onFailure = { failure.complete(it) })
            first.start()
            val pane = first.state.value.panes.first { p -> first.state.value.windows.any { it.session == session && it.id == p.window } }.id
            second = ControlClient(other.openControl(session), scope, {})
            second.start()
            first.input(pane, "i=0; while [ \$i -lt 200 ]; do printf 'SEQ_%04d\\n' \"\$i\"; i=\$((i+1)); sleep 0.002; done\r".encodeToByteArray())
            repeat(10) { assertTrue(first.snapshot(pane).isNotEmpty()); delay(10) }
            withTimeout(5000) { while (!renderer.text().contains("SEQ_0199")) delay(20) }
            val sequence = Regex("(?m)^SEQ_(\\d{4})").findAll(renderer.text()).map { it.groupValues[1].toInt() }.toList()
            assertTrue(sequence.size >= 30)
            assertEquals((sequence.first()..199).toList(), sequence, "Snapshot/live transition must not duplicate or omit output rows")
            val attached = ssh.execute("tmux display-message -p -t '$session' '#{session_attached}'").decodeToString().trim().toInt()
            assertEquals(2, attached)
            other.execute("tmux " + TmuxCommand.killSession(session).wire)
            withTimeout(5000) { failure.await() }
        } finally { first?.close(); second?.close(); ssh.close(); other.close(); scope.cancel() }
    }
}
