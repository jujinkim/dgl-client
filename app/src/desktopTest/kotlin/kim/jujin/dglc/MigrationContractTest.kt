package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.storage.LocalRepository
import kotlinx.serialization.json.*
import java.nio.file.Files
import kotlin.test.*

/** The same checked-in expectations must be consumed by the Rust port. */
class MigrationContractTest {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
    private val fixture = javaClass.getResourceAsStream("/migration/contract-v1.json")!!.use {
        json.parseToJsonElement(it.reader(Charsets.UTF_8).readText()).jsonObject
    }.also { assertEquals(1, it.getValue("schema").jsonPrimitive.int) }
    private fun cases(key: String) = fixture.getValue(key).jsonArray.map { it.jsonObject }
    private fun JsonObject.text(key: String) = getValue(key).jsonPrimitive.content
    private fun JsonObject.number(key: String, default: Int) = get(key)?.jsonPrimitive?.int ?: default
    private fun JsonObject.rejected() = get("error")?.jsonPrimitive?.boolean == true
    private fun String.bytes() = chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun ByteArray.hex() = joinToString("") { "%02x".format(it.toInt() and 255) }
    private fun event(value: ControlEvent): JsonObject = buildJsonObject {
        when (value) {
            is ControlEvent.Response -> {
                put("type", "response"); put("timestamp", value.timestamp); put("number", value.number)
                put("flags", value.flags); put("failed", value.failed)
                putJsonArray("linesHex") { value.lines.forEach { add(it.hex()) } }
            }
            is ControlEvent.Output -> {
                put("type", "output"); put("pane", value.pane); put("bytesHex", value.bytes.hex())
            }
            is ControlEvent.Notification -> {
                put("type", "notification"); put("name", value.name); put("arguments", value.arguments)
            }
        }
    }

    @Test fun controlStreamFixturesAtEveryBoundary() {
        for (case in cases("protocol")) {
            val wire = case.text("wire").encodeToByteArray()
            val partitions = (0..wire.size).map { listOf(wire.copyOfRange(0, it), wire.copyOfRange(it, wire.size)) } +
                listOf(wire.map { byteArrayOf(it) })
            for (chunks in partitions) {
                val parser = ControlParser(case.number("maxLine", 1 shl 20), case.number("maxResponse", 4 shl 20))
                val result = mutableListOf<ControlEvent>()
                if (case.rejected()) {
                    var feedFailed = false
                    assertFailsWith<ProtocolException>(case.text("id")) {
                        try { chunks.forEach { result += parser.feed(it) } }
                        catch (error: ProtocolException) { feedFailed = true; throw error }
                        parser.finish()
                    }
                    if (feedFailed) assertFailsWith<IllegalStateException> { parser.feed(byteArrayOf()) }
                } else {
                    chunks.forEach { result += parser.feed(it) }; parser.finish()
                    assertEquals(case.getValue("events"), JsonArray(result.map(::event)), case.text("id"))
                }
            }
        }
    }

    @Test fun escapeFixtures() {
        for (case in cases("escapes")) {
            val decode = {
                val bytes = case.text("source").encodeToByteArray()
                if (case.text("mode") == "capture") decodeCapture(bytes) else decodeOctal(bytes)
            }
            if (case.rejected()) assertFailsWith<ProtocolException>(case.text("id")) { decode() }
            else assertEquals(case.text("bytesHex"), decode().hex(), case.text("id"))
        }
    }

    private fun command(c: JsonObject): TmuxWire {
        fun target() = c.text("target")
        fun name() = c.text("name")
        return when (c.text("op")) {
            "newSession" -> TmuxWire.newSession(name())
            "renameSession" -> TmuxWire.renameSession(target(), name())
            "killSession" -> TmuxWire.killSession(target())
            "switchSession" -> TmuxWire.switchSession(target())
            "newWindow" -> TmuxWire.newWindow(target(), name())
            "renameWindow" -> TmuxWire.renameWindow(target(), name())
            "selectWindow" -> TmuxWire.selectWindow(target())
            "killWindow" -> TmuxWire.killWindow(target())
            "selectPane" -> TmuxWire.selectPane(target())
            "killPane" -> TmuxWire.killPane(target())
            "capture" -> TmuxWire.capture(target())
            "captureMain" -> TmuxWire.captureMain(target())
            "pending" -> TmuxWire.pending(target())
            "pause" -> TmuxWire.pause(target())
            "resume" -> TmuxWire.resume(target())
            "split" -> TmuxWire.split(target(), c.getValue("horizontal").jsonPrimitive.boolean)
            "resizePane" -> TmuxWire.resizePane(target(), c.text("direction").single())
            "clientSize" -> TmuxWire.clientSize(Size(c.number("columns", 80), c.number("rows", 24)))
            "send" -> TmuxWire.send(target(), c.text("bytesHex").bytes())
            "flowControl" -> TmuxWire.flowControl
            else -> error("Unknown fixture operation")
        }
    }

    @Test fun commandFixtures() {
        for (case in cases("commands")) {
            if (case.rejected()) assertFailsWith<IllegalArgumentException>(case.text("id")) { command(case) }
            else {
                val result = command(case)
                assertEquals(case.text("wire"), result.wire, case.text("id"))
                assertEquals(case.text("shellWire"), result.shellWire, case.text("id"))
                assertEquals("'/test path'\\''/tmux' -u " + result.shellWire.removePrefix("tmux "),
                    result.shellWithExecutable("/test path'/tmux"))
            }
        }
    }

    @Test fun inputFixtures() {
        val keys = fixture.getValue("keys").jsonObject
        assertEquals(TerminalKey.entries.map { it.name }.toSet(), keys.keys)
        keys.forEach { (name, expected) -> assertEquals(expected.jsonPrimitive.content, TerminalKey.valueOf(name).bytes().hex()) }
        for (case in cases("input")) {
            val steps = case.getValue("steps").jsonArray.map { row ->
                val step = row.jsonObject
                when (step.text("type")) {
                    "text" -> MacroStep.Text(step.text("text").repeat(step.number("repeat", 1)))
                    "key" -> MacroStep.Key(TerminalKey.valueOf(step.text("key")))
                    else -> error("Unknown macro fixture")
                }
            }
            if (case.rejected()) assertFailsWith<IllegalArgumentException>(case.text("id")) { TerminalInput.encode(steps) }
            else {
                val bytes = TerminalInput.encode(steps)
                if ("bytesHex" in case) assertEquals(case.text("bytesHex"), bytes.hex(), case.text("id"))
                else {
                    assertEquals(case.number("expectedSize", -1), bytes.size, case.text("id"))
                    val pattern = case.text("expectedPatternHex").bytes()
                    bytes.forEachIndexed { index, byte -> assertEquals(pattern[index % pattern.size], byte) }
                }
            }
        }
    }

    @Test fun settingsFixtures() {
        val directory = Files.createTempDirectory("dgl-contract-").toFile()
        try {
            for (case in cases("settings")) {
                val source = case.getValue("source").toString()
                val file = directory.resolve("settings.json").apply { writeText(source) }
                val repository = LocalRepository(directory)
                if (case.rejected()) {
                    assertFailsWith<IllegalArgumentException>(case.text("id")) { repository.read() }
                    assertEquals(source, file.readText(), "Invalid source must be preserved")
                } else {
                    val state = repository.read()
                    assertEquals(case.getValue("expected"), json.encodeToJsonElement(LocalState.serializer(), state), case.text("id"))
                    repository.save(state)
                    assertEquals(state, repository.read(), case.text("id"))
                }
            }
        } finally { directory.deleteRecursively() }
    }

    @Test fun stateFixtures() {
        for (case in cases("state")) {
            val decode = {
                val rows = case.getValue("rows").jsonArray.map { it.jsonPrimitive.content.encodeToByteArray() }
                JsonArray(when (case.text("kind")) {
                    "sessions" -> TmuxRows.sessions(rows).map { buildJsonObject {
                        put("id", it.id); put("name", it.name); put("attached", it.attached)
                    } }
                    "windows" -> TmuxRows.windows(rows).map { buildJsonObject {
                        put("id", it.id); put("session", it.session); put("name", it.name)
                        put("active", it.active); put("layout", it.layout); put("index", it.index); put("width", it.width); put("height", it.height); put("zoomed", it.zoomed); put("visible_layout", it.visibleLayout)
                    } }
                    "panes" -> TmuxRows.panes(rows).map { buildJsonObject {
                        put("id", it.id); put("window", it.window); put("width", it.width); put("height", it.height)
                        put("active", it.active); put("mode", it.mode); put("command", it.command); put("title", it.title); put("left", it.left); put("top", it.top)
                    } }
                    else -> error("Unknown state fixture")
                })
            }
            if (case.rejected()) assertFailsWith<ProtocolException>(case.text("id")) { decode() }
            else assertEquals(case.getValue("expected"), decode(), case.text("id"))
        }
    }
}
