package kim.jujin.dglc.core

import kotlinx.serialization.json.*

/** tmux command text for the reference engine and test setup. Arbitrary terminal input is encoded as hex bytes. */
class TmuxWire private constructor(val wire: String, val shellWire: String = "tmux $wire") {
    fun shellWithExecutable(executable: String): String {
        require(executable.startsWith('/')) { "Expected an absolute tmux path" }
        return quote(executable) + " -u " + shellWire.removePrefix("tmux ")
    }

    companion object {
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
            require(value.isNotBlank() && value.length <= 128 && value.none { it == ':' || it == '.' })
            return quote(value.replace("#", "##")) // Names undergo tmux format expansion once.
        }
        /** The reference engine's encoding of a product request. */
        fun of(command: TmuxCommand): TmuxWire {
            val op = command.operation
            fun text(key: String) = op.getValue(key).jsonPrimitive.content
            return when (text("op")) {
                "newSession" -> newSession(text("name"))
                "renameSession" -> renameSession(text("target"), text("name"))
                "killSession" -> killSession(text("target"))
                "switchSession" -> switchSession(text("target"))
                "newWindow" -> newWindow(text("target"), text("name"))
                "renameWindow" -> renameWindow(text("target"), text("name"))
                "selectWindow" -> selectWindow(text("target"))
                "killWindow" -> killWindow(text("target"))
                "split" -> split(text("target"), op.getValue("horizontal").jsonPrimitive.boolean)
                "selectPane" -> selectPane(text("target"))
                "renamePane" -> renamePane(text("target"), text("name"))
                "killPane" -> killPane(text("target"))
                "focusPane" -> focusPane(text("target"), text("direction").single())
                "resizePane" -> resizePane(text("target"), text("direction").single())
                else -> throw IllegalArgumentException("Unknown tmux operation")
            }
        }
        fun newSession(name: String): TmuxWire {
            val encoded = name(name)
            // tmux's argv parser treats a trailing semicolon as a command separator,
            // even after the remote shell removed its quotes. Escape that second layer.
            val argument = name.replace("#", "##").let { if (it.endsWith(';')) it.dropLast(1) + "\\;" else it }
            return TmuxWire("new-session -d -s $encoded", "tmux new-session -d -s ${quote(argument)}")
        }
        fun renameSession(session: String, name: String) = TmuxWire("rename-session -t ${id(session, '$')} ${name(name)}")
        fun killSession(session: String) = TmuxWire("kill-session -t ${id(session, '$')}")
        fun switchSession(session: String) = TmuxWire("switch-client -t ${id(session, '$')}")
        fun newWindow(session: String, name: String) = TmuxWire("new-window -t ${id(session, '$')} -n ${name(name)}")
        fun renameWindow(window: String, name: String) = TmuxWire("rename-window -t ${id(window, '@')} ${name(name)}")
        fun selectWindow(window: String) = TmuxWire("select-window -t ${id(window, '@')}")
        fun killWindow(window: String) = TmuxWire("kill-window -t ${id(window, '@')}")
        fun split(pane: String, horizontal: Boolean) = TmuxWire("split-window ${if (horizontal) "-h" else "-v"} -t ${id(pane, '%')}")
        fun selectPane(pane: String) = TmuxWire("select-pane -t ${id(pane, '%')}")
        fun renamePane(pane: String, name: String) = TmuxWire("select-pane -t ${id(pane, '%')} -T ${name(name)}")
        fun killPane(pane: String) = TmuxWire("kill-pane -t ${id(pane, '%')}")
        fun focusPane(pane: String, direction: Char): TmuxWire {
            require(direction in "LRUD")
            return TmuxWire("select-pane -t ${id(pane, '%')} -$direction")
        }
        fun resizePane(pane: String, direction: Char): TmuxWire {
            require(direction in "LRUD")
            return TmuxWire("resize-pane -t ${id(pane, '%')} -$direction 3")
        }
        fun clientSize(size: Size): TmuxWire { require(size.columns in 2..500 && size.rows in 2..250); return TmuxWire("refresh-client -C ${size.columns},${size.rows}") }
        fun send(pane: String, bytes: ByteArray): TmuxWire {
            require(bytes.size in 1..256)
            return TmuxWire("send-keys -H -t ${id(pane, '%')} " + bytes.joinToString(" ") { (it.toInt() and 255).toString(16).padStart(2, '0') })
        }
        fun capture(pane: String) = TmuxWire("capture-pane -p -e -C -N -t ${id(pane, '%')}")
        fun captureMain(pane: String) = TmuxWire("capture-pane -p -a -q -e -C -N -t ${id(pane, '%')}")
        fun pending(pane: String) = TmuxWire("capture-pane -p -P -C -t ${id(pane, '%')}")
        fun pause(pane: String) = TmuxWire("refresh-client -A ${quote("${validatedPane(pane)}:pause")}")
        fun resume(pane: String) = TmuxWire("refresh-client -A ${quote("${validatedPane(pane)}:continue")}")
        private fun validatedPane(pane: String): String { id(pane, '%'); return pane }
        fun modes(pane: String) = TmuxWire("display-message -p -t ${id(pane, '%')} " + quote("#{cursor_x}|#{cursor_y}|#{alternate_on}|#{cursor_flag}|#{keypad_cursor_flag}|#{keypad_flag}|#{mouse_any_flag}|#{mouse_button_flag}|#{mouse_standard_flag}|#{mouse_utf8_flag}|#{mouse_sgr_flag}|#{bracketed_paste}|#{scroll_region_upper}|#{scroll_region_lower}|#{insert_flag}|#{pane_width}|#{pane_height}"))
        val sessions = TmuxWire("list-sessions -F '#{session_id}|#{q:session_name}|#{session_attached}'")
        val windows = TmuxWire("list-windows -a -F '#{session_id}|#{window_id}|#{q:window_name}|#{window_active}|#{window_layout}|#{window_index}|#{window_width}|#{window_height}|#{window_zoomed_flag}|#{window_visible_layout}'")
        val panes = TmuxWire("list-panes -a -F '#{pane_id}|#{window_id}|#{pane_width}|#{pane_height}|#{pane_active}|#{q:pane_mode}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title}|#{pane_left}|#{pane_top}'")
        val metadataSubscription = TmuxWire("refresh-client -B 'dgl-metadata::#{S:#{session_id}|#{q:session_name}|#{W:#{window_id}|#{window_index}|#{q:window_name}|#{window_active}|#{P:#{pane_id}|#{pane_active}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title};}}}'")
        val flowControl = TmuxWire("refresh-client -f 'pause-after=2'")
    }
}

/** Rows printed by [TmuxWire.sessions], [TmuxWire.windows] and [TmuxWire.panes]. */
object TmuxRows {
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
