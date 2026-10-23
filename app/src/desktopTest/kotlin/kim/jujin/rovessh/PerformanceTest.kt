package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.ssh.SshjTransport
import kim.jujin.rovessh.tmux.ControlClient
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.junit.Assume.assumeTrue
import java.io.File
import java.util.concurrent.atomic.AtomicLong
import kotlin.test.*

class PerformanceTest {
    @Test fun loopbackInputLatencyAndSustainedOutput() = runBlocking<Unit> {
        assumeTrue(System.getenv("DGL_TEST_PORT") != null)
        val host = HostProfile("performance", "Synthetic", "127.0.0.1", System.getenv("DGL_TEST_PORT").toInt(), System.getenv("DGL_TEST_USER"), AuthKind.PrivateKey)
        val transport = SshjTransport()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val output = Channel<ByteArray>(256)
        val received = AtomicLong()
        var client: ControlClient? = null
        val begin = System.nanoTime()
        try {
            transport.connect(host, Credential(privateKey = File(System.getenv("DGL_TEST_KEY_FILE")).readText()), System.getenv("DGL_TEST_FINGERPRINT"))
            val name = "performance-${System.nanoTime()}"
            transport.execute("tmux " + TmuxCommand.newSession(name).wire)
            val session = RemoteStateDecoder.sessions(transport.execute("tmux " + TmuxCommand.sessions.wire).decodeToString().trimEnd().lines().map { it.encodeToByteArray() }).single { it.name == name }.id
            client = ControlClient(transport.openControl(session), scope, { received.addAndGet(it.bytes.size.toLong()); output.send(it.bytes) })
            client.start()
            val windows = client.state.value.windows.filter { it.session == session }.map { it.id }
            val pane = client.state.value.panes.first { it.window in windows }.id
            client.snapshot(pane)
            val startupMs = (System.nanoTime() - begin) / 1e6
            client.input(pane, "stty -echo\r".encodeToByteArray()); delay(150)
            while (output.tryReceive().isSuccess) { }
            val latency = mutableListOf<Double>()
            repeat(20) { i ->
                val mark = "LATENCY_${i}_DONE"
                val start = System.nanoTime()
                client.input(pane, "printf 'LATENCY_%d_DONE\\n' $i\r".encodeToByteArray())
                var text = ""
                withTimeout(5000) { while (!text.contains(mark)) text += output.receive().decodeToString() }
                latency += (System.nanoTime() - start) / 1e6
            }
            val start = System.nanoTime(); val before = received.get()
            client.input(pane, "seq 1 20000; printf 'THROUGHPUT_%s\\n' DONE\r".encodeToByteArray())
            var tail = ""
            withTimeout(15000) { while (!tail.contains("THROUGHPUT_DONE")) tail = (tail + output.receive().decodeToString()).takeLast(128) }
            val seconds = (System.nanoTime() - start) / 1e9
            val runtime = Runtime.getRuntime()
            val used = runtime.totalMemory() - runtime.freeMemory()
            val ordered = latency.sorted()
            val version = System.getenv("DGL_TEST_TMUX_VERSION")?.takeIf { it.matches(Regex("[0-9a-z.]+")) } ?: "unknown"
            val report = """{
                "environment": "${System.getProperty("os.name")} ${System.getProperty("os.arch")}, loopback OpenSSH; control callback latency, not GUI/device latency",
                "tmux": "$version",
                "ssh_to_snapshot_ms": $startupMs,
                "input_roundtrip_p50_ms": ${ordered[10]},
                "input_roundtrip_p95_ms": ${ordered[18]},
                "sustained_output_bytes_per_second": ${(received.get() - before) / seconds},
                "observed_jvm_used_heap_bytes": $used,
                "samples": 20
            }
            """
            for (name in listOf("performance.json", "performance-$version.json")) File("../artifacts/$name").apply { parentFile.mkdirs(); writeText(report) }
            assertTrue(received.get() - before > 100000)
            client.close(); client = null
            transport.execute("tmux " + TmuxCommand.killSession(session).wire)
        } finally { client?.close(); transport.close(); scope.cancel() }
    }
}
