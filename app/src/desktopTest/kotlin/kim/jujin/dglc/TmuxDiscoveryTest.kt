package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ssh.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kim.jujin.dglc.storage.MemoryCredentialStore
import kotlin.test.*

class TmuxDiscoveryTest {
    private fun framed(path: String) = "login banner\n\u001eDGL_TMUX_PATH\u001f$path\u001e\nlogout banner".encodeToByteArray()

    @Test fun resolvesSearchPathsAndQuotesExecutable() = runBlocking {
        val calls = mutableListOf<String>()
        val direct = TmuxDiscovery.find { calls += it; framed("/usr/local/bin/tmux") }
        assertEquals("/usr/local/bin/tmux", direct.executable)
        assertEquals(listOf(TmuxProbeOutcome.Found), direct.attempts.map { it.outcome })
        assertEquals(listOf(TmuxDiscovery.direct), calls)
        calls.clear()
        val path = "/opt/tmux tools/it's-tmux"
        val login = TmuxDiscovery.find {
            calls += it
            if (it == TmuxDiscovery.direct) throw RemoteCommandException(127)
            framed(path)
        }
        assertEquals(path, login.executable)
        assertEquals(listOf(TmuxProbeOutcome.NotFound, TmuxProbeOutcome.Found), login.attempts.map { it.outcome })
        assertEquals(listOf(TmuxDiscovery.direct, TmuxDiscovery.login), calls)
        assertEquals("'/opt/tmux tools/it'\\''s-tmux' -u new-session -d -s 'safe\\;'",
            TmuxCommand.newSession("safe;").shellWithExecutable(path))
        calls.clear()
        val fallback = TmuxDiscovery.find {
            calls += it
            when (it) {
                TmuxDiscovery.direct -> throw RemoteCommandException(127)
                TmuxDiscovery.login -> throw RemoteCommandException(126)
                else -> framed("/opt/local/bin/tmux")
            }
        }
        assertEquals(listOf(TmuxDiscovery.direct, TmuxDiscovery.login, TmuxDiscovery.common), calls)
        assertEquals("/opt/local/bin/tmux", fallback.executable)
        assertEquals(listOf(TmuxProbeOutcome.NotFound, TmuxProbeOutcome.Failed, TmuxProbeOutcome.Found), fallback.attempts.map { it.outcome })
        assertEquals(TmuxDiscovery.commonPaths, fallback.attempts.last().paths)
    }

    @Test fun recordsFailuresAndPreservesTransportFailures() = runBlocking {
        for (output in listOf(byteArrayOf(), framed("tmux"), framed("/tmp/tmux\nother"), framed("/tmp/tmux") + framed("/tmp/other"))) {
            val failure = assertFailsWith<TmuxDiscoveryException> { TmuxDiscovery.find { output } }
            assertEquals(List(3) { TmuxProbeOutcome.InvalidResponse }, failure.diagnostics.attempts.map { it.outcome })
            assertNull(failure.diagnostics.executable)
        }
        var calls = 0
        val network = ClientFailure(Failure.Network, "synthetic network failure")
        assertSame(network, assertFailsWith<ClientFailure> { TmuxDiscovery.find { calls++; throw network } })
        assertEquals(1, calls)
        assertFailsWith<CancellationException> { TmuxDiscovery.find { throw CancellationException() } }
        val denied = assertFailsWith<TmuxDiscoveryException> { TmuxDiscovery.find { throw RemoteCommandException(126) } }
        assertEquals(List(3) { TmuxProbeOutcome.Failed }, denied.diagnostics.attempts.map { it.outcome })
        assertTrue(denied.diagnostics.attempts.all { it.exitStatus == 126 })
        val timeout = assertFailsWith<TmuxDiscoveryException> { TmuxDiscovery.find(timeoutMillis = 10) { delay(100); framed("/bin/tmux") } }
        assertEquals(List(3) { TmuxProbeOutcome.TimedOut }, timeout.diagnostics.attempts.map { it.outcome })
    }

    @Test fun shellFallbackExplainsFailureAndReconnectClearsDetails() = runBlocking {
        var mode = TmuxProblemKind.NotFound
        var supported = false
        val attempts = listOf(TmuxProbeAttempt("SSH 기본 PATH", TmuxProbeOutcome.NotFound))
        val transport = object : SshTransport {
            override val tmuxDiagnostics get() = TmuxDiagnostics(attempts, "/opt/homebrew/bin/tmux")
            override suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?) {}
            override suspend fun execute(command: String) = byteArrayOf()
            override suspend fun tmuxVersion(): String = if (supported) "tmux 3.7c" else when (mode) {
                TmuxProblemKind.NotFound -> throw TmuxDiscoveryException(TmuxDiagnostics(attempts))
                TmuxProblemKind.ExecutionFailed -> throw RemoteCommandException(126)
                TmuxProblemKind.UnsupportedVersion -> "tmux 3.2"
                TmuxProblemKind.InvalidVersion -> "unexpected output\nPRIVATE_DIAGNOSTIC"
            }
            override suspend fun openControl(sessionId: String): SshTransport.Stream = error("No selected session")
            override suspend fun openShell(size: Size) = object : SshTransport.Stream() {
                override suspend fun read(): ByteArray? = awaitCancellation()
                override suspend fun write(bytes: ByteArray) {}
                override suspend fun resize(size: Size) {}
                override fun close() {}
            }
            override fun close() {}
        }
        val host = HostProfile("diagnostic-test", "Test", "example.test", username = "test")
        val store = MemoryCredentialStore().apply { write(host.id, Credential("test")) }
        val local = MutableStateFlow(LocalState())
        val model = ClientModel(host, store, local, { local.update(it) }) { transport }
        try {
            for (kind in TmuxProblemKind.entries) {
                mode = kind
                model.connect()
                val problem = withTimeout(3000) { model.tmuxProblem.filterNotNull().first { it.kind == kind } }
                withTimeout(3000) { model.status.first { it.phase == Phase.Shell && it.message.contains(problem.label) } }
                assertEquals(attempts, problem.diagnostics.attempts)
                assertTrue(model.status.value.message.contains(problem.label))
                if (kind == TmuxProblemKind.UnsupportedVersion) assertEquals("tmux 3.2", problem.version)
                if (kind == TmuxProblemKind.ExecutionFailed) assertEquals(126, problem.exitStatus)
                assertFalse(problem.toString().contains("PRIVATE_DIAGNOSTIC"))
            }
            supported = true
            model.connect()
            withTimeout(3000) { model.status.first { it.phase == Phase.Selecting } }
            assertNull(model.tmuxProblem.value)
        } finally { model.close() }
    }
}
