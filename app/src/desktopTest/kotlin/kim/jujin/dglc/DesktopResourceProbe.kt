package kim.jujin.dglc

import com.jediterm.core.util.TermSize
import com.jediterm.terminal.*
import com.jediterm.terminal.emulator.JediEmulator
import com.jediterm.terminal.model.*
import com.sun.management.UnixOperatingSystemMXBean
import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.DesktopConnector
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.io.File
import java.lang.management.ManagementFactory
import java.lang.reflect.Proxy
import java.util.WeakHashMap
import kotlin.concurrent.thread

/** Standalone measured JVM; no Gradle daemon and no JUnit discovery. */
object DesktopResourceProbe {
    @JvmStatic fun main(args: Array<String>) {
        val configuration = Json.parseToJsonElement(File(args[0]).readText()).jsonObject.toMutableMap()
        configuration["appPid"] = JsonPrimitive(ProcessHandle.current().pid())
        try { runBlocking { ResourceScenario(JsonObject(configuration), File(args[1]), DesktopResourcePlatform()).run() } }
        catch (e: Throwable) { System.err.println("Resource probe failed: ${e.javaClass.simpleName}"); e.printStackTrace(); kotlin.system.exitProcess(1) }
        kotlin.system.exitProcess(0)
    }
}
private class DesktopResourcePlatform : ResourcePlatform {
    private var connector: DesktopConnector? = null
    private var buffer: TerminalTextBuffer? = null
    private var consumer: Thread? = null
    @Volatile private var slow = false
    @Volatile private var consumerFailure = false
    private val live = WeakHashMap<TerminalTextBuffer, Boolean>()
    override suspend fun attach(model: ClientModel) {
        val connection = DesktopConnector(model::input, model::resize)
        val style = StyleState()
        val text = TerminalTextBuffer(100, 30, style, 10000)
        val display = Proxy.newProxyInstance(TerminalDisplay::class.java.classLoader, arrayOf(TerminalDisplay::class.java)) { _, method, _ ->
            when (method.returnType) { Boolean::class.javaPrimitiveType -> false; Int::class.javaPrimitiveType -> 0; String::class.java -> ""; else -> null }
        } as TerminalDisplay
        val terminal = JediTerminal(display, text, style)
        connection.remoteResize = { terminal.resize(TermSize(it.columns, it.rows), RequestOrigin.Remote) }
        connector = connection; buffer = text; synchronized(live) { live[text] = true }
        consumer = thread(name = "resource-jediterm") {
            try {
                val emulator = JediEmulator(TtyBasedArrayDataStream(connection), terminal)
                while (connection.isConnected && emulator.hasNext()) {
                    while (slow && connection.isConnected) Thread.sleep(20)
                    emulator.next()
                }
            } catch (_: Exception) { if (connection.isConnected) consumerFailure = true }
        }
        model.bindSurface(connection.surface)
    }
    override suspend fun detach(model: ClientModel) {
        connector?.let { model.unbindSurface(it.surface); it.close() }
        consumer?.join(5000); check(consumer?.isAlive != true) { "consumer_not_reclaimed" }
        connector = null; buffer = null; consumer = null
    }
    override suspend fun recreate() {}
    override suspend fun slowConsumer(enabled: Boolean) {
        if (!enabled) {
            val pipe = DesktopConnector::class.java.getDeclaredField("pipe").apply { isAccessible = true }.get(connector) as java.io.PipedInputStream
            check(pipe.available() == 262144) { "backpressure_not_exercised" }
        }
        slow = enabled
    }
    override suspend fun metrics(): Map<String, Long> {
        check(!consumerFailure) { "consumer_failed" }
        val text = buffer
        val history = text?.historyLinesCount ?: 0
        check(history <= 10000) { "scrollback_unbounded" }
        val tail = text?.getScreenLines() ?: ""
        val progress = Regex("DGL:(\\d+)").findAll(tail).map { it.groupValues[1].toLong() }.maxOrNull() ?: 0L
        val pipe = connector?.let { c -> DesktopConnector::class.java.getDeclaredField("pipe").apply { isAccessible = true }.get(c) as java.io.PipedInputStream }
        val queued = pipe?.available() ?: 0
        check(queued <= 262144)
        return mapOf("fd" to (ManagementFactory.getOperatingSystemMXBean() as UnixOperatingSystemMXBean).openFileDescriptorCount,
            "threads" to ManagementFactory.getThreadMXBean().threadCount.toLong(),
            "gcCount" to ManagementFactory.getGarbageCollectorMXBeans().sumOf { it.collectionCount.coerceAtLeast(0) },
            "gcMillis" to ManagementFactory.getGarbageCollectorMXBeans().sumOf { it.collectionTime.coerceAtLeast(0) },
            "scrollback" to history.toLong(), "renderSequence" to progress, "queuedBytes" to queued.toLong(),
            "liveTerminals" to synchronized(live) { live.size.toLong() })
    }
}
