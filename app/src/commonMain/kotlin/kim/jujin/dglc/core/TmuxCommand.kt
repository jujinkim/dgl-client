package kim.jujin.dglc.core

import kotlinx.serialization.json.*

/** A tmux management request for the Rust core, which validates it again and builds the tmux command. */
class TmuxCommand private constructor(val operation: JsonObject) {
    companion object {
        private fun operation(op: String, target: String? = null, name: String? = null, horizontal: Boolean? = null, direction: Char? = null) = TmuxCommand(buildJsonObject {
            put("op", op)
            target?.let { put("target", it) }; name?.let { put("name", it) }
            horizontal?.let { put("horizontal", it) }; direction?.let { put("direction", it.toString()) }
        })

        private fun id(value: String, prefix: Char): String {
            require(value.matches(Regex("\\${prefix}[0-9]+"))) { "Invalid tmux target" }
            return value
        }
        private fun name(value: String): String {
            require(value.isNotBlank() && value.length <= 128 && value.none { it == ':' || it == '.' || it.code < 32 || it.code == 127 }) {
                t("이름은 1–128자이며 콜론·마침표를 포함할 수 없습니다.")
            }
            return value
        }
        private fun direction(value: Char): Char { require(value in "LRUD"); return value }
        fun newSession(name: String) = operation("newSession", name = name(name))
        fun renameSession(session: String, name: String) = operation("renameSession", id(session, '$'), name(name))
        fun killSession(session: String) = operation("killSession", id(session, '$'))
        fun switchSession(session: String) = operation("switchSession", id(session, '$'))
        fun newWindow(session: String, name: String) = operation("newWindow", id(session, '$'), name(name))
        fun renameWindow(window: String, name: String) = operation("renameWindow", id(window, '@'), name(name))
        fun selectWindow(window: String) = operation("selectWindow", id(window, '@'))
        fun killWindow(window: String) = operation("killWindow", id(window, '@'))
        fun split(pane: String, horizontal: Boolean) = operation("split", id(pane, '%'), horizontal = horizontal)
        fun selectPane(pane: String) = operation("selectPane", id(pane, '%'))
        fun renamePane(pane: String, name: String) = operation("renamePane", id(pane, '%'), name(name))
        fun killPane(pane: String) = operation("killPane", id(pane, '%'))
        fun focusPane(pane: String, direction: Char) = operation("focusPane", id(pane, '%'), direction = direction(direction))
        fun resizePane(pane: String, direction: Char) = operation("resizePane", id(pane, '%'), direction = direction(direction))
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
}
