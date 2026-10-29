package kim.jujin.dglc.core

/** Byte framing precedes UTF-8 decoding. A partial codepoint can span %output messages. */
sealed interface ControlEvent {
    data class Response(val timestamp: Long, val number: Long, val flags: Int, val lines: List<ByteArray>, val failed: Boolean) : ControlEvent
    data class Output(val pane: String, val bytes: ByteArray) : ControlEvent
    data class Notification(val name: String, val arguments: String) : ControlEvent
}

class ProtocolException(message: String) : Exception(message)

class ControlParser(private val maxLine: Int = 1 shl 20, private val maxResponse: Int = 4 shl 20) {
    private var line = ByteArray(minOf(4096, maxLine))
    private var used = 0
    private var frame: Triple<Long, Long, Int>? = null
    private val body = mutableListOf<ByteArray>()
    private var responseBytes = 0
    private var poisoned = false

    fun feed(bytes: ByteArray): List<ControlEvent> {
        check(!poisoned) { "Parser is closed after a protocol error" }
        val events = mutableListOf<ControlEvent>()
        try {
            for (b in bytes) {
                if (b == 10.toByte()) {
                    parseLine(line.copyOf(used), events)
                    used = 0
                } else {
                    if (used == maxLine) throw ProtocolException("Control line limit exceeded")
                    if (used == line.size) line = line.copyOf(minOf(maxLine, line.size * 2))
                    line[used++] = b
                }
            }
        } catch (e: Exception) { poisoned = true; throw e }
        return events
    }

    fun finish() {
        if (used != 0 || frame != null) throw ProtocolException("Truncated control stream")
    }

    private fun boundary(raw: ByteArray): Triple<Long, Long, Int>? {
        val fields = raw.decodeToString().split(' ')
        if (fields.size != 4) return null
        val t = fields[1].toLongOrNull() ?: return null
        val n = fields[2].toLongOrNull() ?: return null
        val f = fields[3].toIntOrNull() ?: return null
        if (t < 0 || n < 0 || f < 0) return null
        return Triple(t, n, f)
    }

    private fun parseLine(raw: ByteArray, out: MutableList<ControlEvent>) {
        val separator = raw.indexOf(32)
        val name = raw.copyOfRange(0, if (separator < 0) raw.size else separator).decodeToString()
        val current = frame
        if (current != null) {
            // Command output may itself start with "%output", "%begin" or "%end".
            // Only the matching guard terminates a response.
            if ((name == "%end" || name == "%error") && boundary(raw) == current) {
                out += ControlEvent.Response(current.first, current.second, current.third, body.toList(), name == "%error")
                body.clear(); frame = null; responseBytes = 0
            } else {
                responseBytes += raw.size + 1
                if (responseBytes > maxResponse) throw ProtocolException("Control response limit exceeded")
                body += raw
            }
            return
        }
        when (name) {
            "%begin" -> frame = boundary(raw) ?: throw ProtocolException("Invalid begin guard")
            "%end", "%error" -> throw ProtocolException("Unexpected response guard")
            "%output", "%extended-output" -> {
                if (separator < 0) throw ProtocolException("Missing output pane")
                val paneEnd = raw.indexOfFrom(32, separator + 1)
                if (paneEnd < 0) throw ProtocolException("Missing output body")
                val pane = raw.copyOfRange(separator + 1, paneEnd).decodeToString()
                if (!pane.matches(Regex("%[0-9]+"))) throw ProtocolException("Invalid output pane")
                val start = if (name == "%extended-output") {
                    var i = paneEnd + 1
                    while (i + 1 < raw.size && !(raw[i] == 58.toByte() && raw[i + 1] == 32.toByte())) i++
                    if (i + 1 >= raw.size) throw ProtocolException("Invalid extended output")
                    i + 2
                } else paneEnd + 1
                out += ControlEvent.Output(pane, decodeOctal(raw.copyOfRange(start, raw.size)))
            }
            else -> {
                if (!name.startsWith('%')) throw ProtocolException("Unexpected control message")
                out += ControlEvent.Notification(name, if (separator < 0) "" else raw.copyOfRange(separator + 1, raw.size).decodeToString())
            }
        }
    }
}

private fun ByteArray.indexOfFrom(value: Byte, start: Int): Int {
    for (i in start until size) if (this[i] == value) return i
    return -1
}

fun decodeOctal(bytes: ByteArray): ByteArray = decodeEscapes(bytes, false)
fun decodeCapture(bytes: ByteArray): ByteArray = decodeEscapes(bytes, true)
private fun decodeEscapes(bytes: ByteArray, capture: Boolean): ByteArray {
    val result = ByteArray(bytes.size)
    var i = 0; var count = 0
    while (i < bytes.size) {
        if (bytes[i] == 92.toByte()) {
            if (capture && i + 1 < bytes.size && bytes[i + 1] == 92.toByte()) {
                result[count++] = 92; i += 2; continue
            }
            if (i + 3 >= bytes.size) throw ProtocolException("Truncated octal escape")
            var value = 0
            for (j in 1..3) {
                val digit = bytes[i + j].toInt() - 48
                if (digit !in 0..7) throw ProtocolException("Invalid octal escape")
                value = value * 8 + digit
            }
            if (value > 255) throw ProtocolException("Octal escape overflow")
            result[count++] = value.toByte(); i += 4
        } else result[count++] = bytes[i++]
    }
    return result.copyOf(count)
}
