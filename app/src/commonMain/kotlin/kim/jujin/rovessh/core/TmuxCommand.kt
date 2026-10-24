package kim.jujin.rovessh.core

import kotlinx.serialization.json.*

/** Only this encoder constructs commands. Arbitrary terminal input is encoded as hex bytes. */
class TmuxCommand private constructor(val wire: String, val shellWire: String = "tmux $wire", val nativeOperation: JsonObject? = null) {
    fun shellWithExecutable(executable: String): String {
        require(executable.startsWith('/')) { "Expected an absolute tmux path" }
        return quote(executable) + " " + shellWire.removePrefix("tmux ")
    }

    companion object {
        private fun operation(op: String, target: String? = null, name: String? = null, horizontal: Boolean? = null, direction: Char? = null) = buildJsonObject {
            put("op", op)
            target?.let { put("target", it) }; name?.let { put("name", it) }
            horizontal?.let { put("horizontal", it) }; direction?.let { put("direction", it.toString()) }
        }

        private fun id(value: String, prefix: Char): String {
            require(value.matches(Regex("\\${prefix}[0-9]+"))) { "Invalid tmux target" }
            return quote(value)
        }
        fun quote(value: String): String {
            require(value.length <= 1024 && value.none { it.code < 32 || it.code == 127 })
            // tmux's single quotes suppress lexical variable expansion.
            return "'" + value.replace("'", "'\\''") + "'"
        }
        private fun name(value: String): String {
            require(value.isNotBlank() && value.length <= 128 && value.none { it == ':' || it == '.' }) { "이름은 1–128자이며 콜론·마침표를 포함할 수 없습니다." }
            return quote(value.replace("#", "##")) // Names undergo tmux format expansion once.
        }
        fun newSession(name: String): TmuxCommand {
            val encoded = name(name)
            // tmux's argv parser treats a trailing semicolon as a command separator,
            // even after the remote shell removed its quotes. Escape that second layer.
            val argument = name.replace("#", "##").let { if (it.endsWith(';')) it.dropLast(1) + "\\;" else it }
            return TmuxCommand("new-session -d -s $encoded", "tmux new-session -d -s ${quote(argument)}", operation("newSession", name = name))
        }
        fun renameSession(session: String, name: String) = TmuxCommand("rename-session -t ${id(session, '$')} ${name(name)}", nativeOperation = operation("renameSession", target = session, name = name))
        fun killSession(session: String) = TmuxCommand("kill-session -t ${id(session, '$')}", nativeOperation = operation("killSession", target = session))
        fun switchSession(session: String) = TmuxCommand("switch-client -t ${id(session, '$')}", nativeOperation = operation("switchSession", target = session))
        fun newWindow(session: String, name: String) = TmuxCommand("new-window -t ${id(session, '$')} -n ${name(name)}", nativeOperation = operation("newWindow", target = session, name = name))
        fun renameWindow(window: String, name: String) = TmuxCommand("rename-window -t ${id(window, '@')} ${name(name)}", nativeOperation = operation("renameWindow", target = window, name = name))
        fun selectWindow(window: String) = TmuxCommand("select-window -t ${id(window, '@')}", nativeOperation = operation("selectWindow", target = window))
        fun killWindow(window: String) = TmuxCommand("kill-window -t ${id(window, '@')}", nativeOperation = operation("killWindow", target = window))
        fun split(pane: String, horizontal: Boolean) = TmuxCommand("split-window ${if (horizontal) "-h" else "-v"} -t ${id(pane, '%')}", nativeOperation = operation("split", target = pane, horizontal = horizontal))
        fun selectPane(pane: String) = TmuxCommand("select-pane -t ${id(pane, '%')}", nativeOperation = operation("selectPane", target = pane))
        fun killPane(pane: String) = TmuxCommand("kill-pane -t ${id(pane, '%')}", nativeOperation = operation("killPane", target = pane))
        fun resizePane(pane: String, direction: Char) : TmuxCommand {
            require(direction in "LRUD")
            return TmuxCommand("resize-pane -t ${id(pane, '%')} -$direction 3", nativeOperation = operation("resizePane", target = pane, direction = direction))
        }
        fun clientSize(size: Size) = TmuxCommand("refresh-client -C ${size.columns},${size.rows}")
        fun send(pane: String, bytes: ByteArray): TmuxCommand {
            require(bytes.size in 1..256)
            return TmuxCommand("send-keys -H -t ${id(pane, '%')} " + bytes.joinToString(" ") { (it.toInt() and 255).toString(16).padStart(2, '0') })
        }
        fun capture(pane: String) = TmuxCommand("capture-pane -p -e -C -N -t ${id(pane, '%')}")
        fun captureMain(pane: String) = TmuxCommand("capture-pane -p -a -q -e -C -N -t ${id(pane, '%')}")
        fun pending(pane: String) = TmuxCommand("capture-pane -p -P -C -t ${id(pane, '%')}")
        fun pause(pane: String) = TmuxCommand("refresh-client -A ${quote("${validatedPane(pane)}:pause")}")
        fun resume(pane: String) = TmuxCommand("refresh-client -A ${quote("${validatedPane(pane)}:continue")}")
        private fun validatedPane(pane: String): String { id(pane, '%'); return pane }
        fun modes(pane: String) = TmuxCommand("display-message -p -t ${id(pane, '%')} " + quote("#{cursor_x}|#{cursor_y}|#{alternate_on}|#{cursor_flag}|#{keypad_cursor_flag}|#{keypad_flag}|#{mouse_any_flag}|#{mouse_button_flag}|#{mouse_standard_flag}|#{mouse_utf8_flag}|#{mouse_sgr_flag}|#{bracketed_paste}|#{scroll_region_upper}|#{scroll_region_lower}|#{insert_flag}|#{pane_width}|#{pane_height}"))
        val sessions = TmuxCommand("list-sessions -F '#{session_id}|#{q:session_name}|#{session_attached}'")
        val windows = TmuxCommand("list-windows -a -F '#{session_id}|#{window_id}|#{q:window_name}|#{window_active}|#{window_layout}'")
        val panes = TmuxCommand("list-panes -a -F '#{pane_id}|#{window_id}|#{pane_width}|#{pane_height}|#{pane_active}|#{q:pane_mode}|#{q:pane_current_command}'")
        val flowControl = TmuxCommand("refresh-client -f 'pause-after=2'")
    }
}

object RemoteStateDecoder {
    private fun splitRecord(value: String): List<String> {
        val fields = mutableListOf<String>(); val part = StringBuilder(); var escaped = false
        for (c in value) {
            if (c == '|' && !escaped) { fields += part.toString(); part.clear() }
            else { part.append(c); escaped = c == '\\' && !escaped }
        }
        fields += part.toString()
        return fields
    }
    private fun lines(rows: List<ByteArray>, count: Int) = rows.map { splitRecord(it.decodeToString()).also { fields ->
        if (fields.size != count) throw ProtocolException("Invalid state record")
    } }
    fun sessions(rows: List<ByteArray>) = lines(rows, 3).map { RemoteSession(target(it[0], '$'), unquote(it[1]), it[2].toInt()) }
    fun windows(rows: List<ByteArray>) = lines(rows, 5).map { RemoteWindow(target(it[1], '@'), target(it[0], '$'), unquote(it[2]), it[3] == "1", it[4]) }
    fun panes(rows: List<ByteArray>) = lines(rows, 7).map { RemotePane(target(it[0], '%'), target(it[1], '@'), it[2].toInt(), it[3].toInt(), it[4] == "1", unquote(it[5]), unquote(it[6])) }
    private fun target(value: String, prefix: Char): String {
        if (value.firstOrNull() != prefix || value.drop(1).isEmpty() || value.drop(1).any { !it.isDigit() }) throw ProtocolException("Invalid state id")
        return value
    }
    // tmux q: protects separators using backslash escaping. Keep rendering inert text.
    private fun unquote(value: String): String {
        val result = StringBuilder(); var escaped = false
        for (c in value) {
            if (escaped) { result.append(c); escaped = false }
            else if (c == '\\') escaped = true else result.append(c)
        }
        if (escaped) result.append('\\')
        return result.toString()
    }
}

fun supportsTmux(version: String): Boolean {
    val match = Regex("^tmux (\\d+)\\.(\\d+)[a-z]?$" ).matchEntire(version.trim()) ?: return false
    val major = match.groupValues[1].toInt(); val minor = match.groupValues[2].toInt()
    return major == 3 && minor in 3..7
}
