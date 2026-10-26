package kim.jujin.dglc.core

import kotlinx.serialization.json.*

/** Only this encoder constructs commands. Arbitrary terminal input is encoded as hex bytes. */
class TmuxCommand private constructor(val wire: String, val shellWire: String = "tmux $wire", val nativeOperation: JsonObject? = null) {
    fun shellWithExecutable(executable: String): String {
        require(executable.startsWith('/')) { "Expected an absolute tmux path" }
        return quote(executable) + " -u " + shellWire.removePrefix("tmux ")
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
            require(value.isNotBlank() && value.length <= 128 && value.none { it == ':' || it == '.' }) { t("이름은 1–128자이며 콜론·마침표를 포함할 수 없습니다.") }
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
        fun renamePane(pane: String, name: String) = TmuxCommand("select-pane -t ${id(pane, '%')} -T ${name(name)}", nativeOperation = operation("renamePane", target = pane, name = name))
        fun killPane(pane: String) = TmuxCommand("kill-pane -t ${id(pane, '%')}", nativeOperation = operation("killPane", target = pane))
        fun focusPane(pane: String, direction: Char): TmuxCommand {
            require(direction in "LRUD")
            return TmuxCommand("select-pane -t ${id(pane, '%')} -$direction", nativeOperation = operation("focusPane", target = pane, direction = direction))
        }
        fun resizePane(pane: String, direction: Char) : TmuxCommand {
            require(direction in "LRUD")
            return TmuxCommand("resize-pane -t ${id(pane, '%')} -$direction 3", nativeOperation = operation("resizePane", target = pane, direction = direction))
        }
        fun clientSize(size: Size): TmuxCommand { require(size.columns in 2..500 && size.rows in 2..250); return TmuxCommand("refresh-client -C ${size.columns},${size.rows}") }
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
        val windows = TmuxCommand("list-windows -a -F '#{session_id}|#{window_id}|#{q:window_name}|#{window_active}|#{window_layout}|#{window_index}|#{window_width}|#{window_height}|#{window_zoomed_flag}|#{window_visible_layout}'")
        val panes = TmuxCommand("list-panes -a -F '#{pane_id}|#{window_id}|#{pane_width}|#{pane_height}|#{pane_active}|#{q:pane_mode}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title}|#{pane_left}|#{pane_top}'")
        val metadataSubscription = TmuxCommand("refresh-client -B 'dgl-metadata::#{S:#{session_id}|#{q:session_name}|#{W:#{window_id}|#{window_index}|#{q:window_name}|#{window_active}|#{P:#{pane_id}|#{pane_active}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title};}}}'")
        val flowControl = TmuxCommand("refresh-client -f 'pause-after=2'")
    }
}

object RemoteStateDecoder {
    /** Native Rust JSON contract, also exercised by shared hierarchy fixtures. */
    fun json(value: JsonObject): RemoteState {
        fun JsonObject.str(key: String) = getValue(key).jsonPrimitive.content
        fun rows(key: String) = value.getValue(key).jsonArray.map { it.jsonObject }
        return RemoteState(
            rows("sessions").map { RemoteSession(it.str("id"), it.str("name"), it.str("attached").toInt()) },
            rows("windows").map { RemoteWindow(it.str("id"), it.str("session"), it.str("name"), it.str("active").toBoolean(), it.str("layout"), it.str("index").toInt(), it["width"]?.jsonPrimitive?.int ?: 80, it["height"]?.jsonPrimitive?.int ?: 24, it["zoomed"]?.jsonPrimitive?.boolean ?: false, it["visible_layout"]?.jsonPrimitive?.content ?: "") },
            rows("panes").map { RemotePane(it.str("id"), it.str("window"), it.str("width").toInt(), it.str("height").toInt(), it.str("active").toBoolean(), it.str("mode"), it.str("command"), it.str("title"), it["left"]?.jsonPrimitive?.int ?: 0, it["top"]?.jsonPrimitive?.int ?: 0) },
        )
    }
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
    fun windows(rows: List<ByteArray>) = lines(rows, 10).map { RemoteWindow(target(it[1], '@'), target(it[0], '$'), unquote(it[2]), it[3] == "1", it[4], it[5].toInt(), it[6].toInt(), it[7].toInt(), it[8] == "1", it[9]) }
    fun panes(rows: List<ByteArray>) = lines(rows, 10).map { RemotePane(target(it[0], '%'), target(it[1], '@'), it[2].toInt(), it[3].toInt(), it[4] == "1", unquote(it[5]), unquote(it[6]), unquote(it[7]), it[8].toInt(), it[9].toInt()) }
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
