package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kotlinx.coroutines.flow.MutableStateFlow

/** Memory only, scoped to a connection/pane generation, independent of viewport and focus. */
class ShortcutInput(private val client: ClientConnection) {
    val modifiers = MutableStateFlow(TerminalModifiers())
    private var identity: List<Any?>? = null
    @Synchronized fun sync() {
        val status = client.status.value
        val next = listOf(client.generation, client.captureInputTarget(), client.session.value, client.pane.value)
        val sizeChange = status.phase == Phase.Restoring && status.resizing && !status.recovering
        if (identity != next || (!status.acceptsInput && !sizeChange)) modifiers.value = TerminalModifiers()
        identity = next
    }
    @Synchronized fun clear() { modifiers.value = TerminalModifiers() }
    @Synchronized fun toggle(key: TerminalShortcut) { sync(); if (client.status.value.acceptsInput) modifiers.value = modifiers.value.toggle(key) }
    @Synchronized fun encode(key: String, text: String?, physical: TerminalModifiers, appCursor: Boolean): ByteArray? {
        sync()
        if (!client.status.value.acceptsInput) return null
        val bytes = client.encodeKeyboard(key, text, modifiers.value.union(physical), appCursor)
        if (bytes == null) client.inputNotice.show(t("terminal.unencodable_shortcut"), SnackbarDuration.Short)
        // Clear at submission, even if the input queue rejects the request. Never retry it.
        else clear()
        return bytes
    }
}
