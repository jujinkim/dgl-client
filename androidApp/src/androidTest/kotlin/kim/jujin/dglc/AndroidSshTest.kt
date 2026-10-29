package kim.jujin.dglc

import androidx.test.platform.app.InstrumentationRegistry
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.SshjTransport
import kim.jujin.dglc.tmux.ControlClient
import kotlinx.coroutines.*
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File

class AndroidSshTest {
    @Test fun sshKeysControlAndReconnectionKeepRemoteProcess() = runBlocking<Unit> {
        assumeTrue("Use scripts/integration.py --command python3 scripts/android_integration.py", InstrumentationRegistry.getArguments().getString("dgl.fixture") == "true")
        val fixture = JSONObject(File(InstrumentationRegistry.getInstrumentation().targetContext.cacheDir, "integration.json").readText())
        val host = HostProfile("android-test", "Test", "127.0.0.1", fixture.getInt("port"), fixture.getString("user"), AuthKind.PrivateKey)
        val fingerprint = fixture.getString("fingerprint")
        val ssh = SshjTransport()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        var client: ControlClient? = null
        try {
            for (kind in listOf("rsa", "encrypted", "key")) {
                ssh.connect(host, Credential(privateKey = fixture.getString(kind), passphrase = if (kind == "encrypted") fixture.getString("passphrase") else ""), fingerprint)
                assertTrue(supportsTmux(ssh.execute("tmux -V").decodeToString()))
                if (kind != "key") ssh.close()
            }
            val name = "android-${System.nanoTime()}"
            ssh.execute("tmux " + TmuxWire.newSession(name).wire)
            val session = TmuxRows.sessions(ssh.execute("tmux " + TmuxWire.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            client = ControlClient(ssh.openControl(session), scope, {})
            client.start()
            val windowIds = client.state.value.windows.filter { it.session == session }.map { it.id }
            val pane = client.state.value.panes.first { it.window in windowIds }.id
            client.input(pane, "printf 'ANDROID_한글_日本語\\n'\r".encodeToByteArray())
            withTimeout(5000) { while (!client.snapshot(pane).decodeToString().contains("ANDROID_한글_日本語")) delay(50) }
            val pid = ssh.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString()
            client.close(); client = null; ssh.close()
            ssh.connect(host, Credential(privateKey = fixture.getString("key")), fingerprint)
            assertEquals(pid, ssh.execute("tmux display-message -p -t '$pane' '#{pane_pid}'").decodeToString())
            client = ControlClient(ssh.openControl(session), scope, {})
            client.start()
            assertTrue(client.snapshot(pane).decodeToString().contains("ANDROID_한글_日本語"))
            client.close(); client = null
            ssh.execute("tmux " + TmuxWire.killSession(session).wire)
        } catch (e: ClientFailure) { throw AssertionError("${e.kind}: ${e.diagnosticCode}") }
        finally { client?.close(); ssh.close(); scope.cancel() }
    }
}
