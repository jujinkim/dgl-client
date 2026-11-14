package kim.jujin.dglc

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.ui.text.input.TextFieldValue
import kim.jujin.dglc.core.TerminalInput
import kim.jujin.dglc.core.TerminalModifiers
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.serialization.json.*

/** Native target identity, deliberately independent of grid/IME changes. */
fun inputTargetIdentity(raw: String): String {
    val target = Json.parseToJsonElement(raw).jsonObject
    val generation = target.getValue("generation").jsonObject
    return JsonArray(listOf(target.getValue("connection"), target.getValue("pane_id"),
        generation.getValue("connection"), generation.getValue("pane"))).toString()
}

/**
 * One clipboard paste into the target captured when its menu opened. It uses the composer's
 * limits and target checks, the acknowledged input queue, and is never retried.
 * Returns the error to show, or null after the queue accepted the text.
 */
suspend fun pasteTerminalText(client: ClientConnection, destination: InputTarget?, current: () -> Boolean, text: String,
    preparePaste: suspend (String) -> ByteArray): String? {
    if (text.isEmpty()) return t("input.clipboard_empty")
    if (text.encodeToByteArray().size > TerminalInput.MAX_BYTES) return t("input.too_large")
    fun matches() = destination != null && current() && destination == client.captureInputTarget()
    val changed = t("input.paste_cancelled_target_changed")
    if (!matches()) return changed
    return try {
        val bytes = preparePaste(text)
        when {
            bytes.isEmpty() || bytes.size > TerminalInput.MAX_BYTES -> t("input.too_large_with_paste")
            !matches() -> changed
            else -> client.sendInput(destination!!, bytes).error
        }
    } catch (_: TimeoutCancellationException) { t("input.prepare_timeout") }
    catch (cancelled: CancellationException) { throw cancelled }
    catch (cause: Exception) { cause.message ?: t("input.send_unconfirmed") }
}

/** Connection-owned, memory only. Neither saved-state Bundles nor settings contain drafts. */
class TextComposer(private val client: ClientConnection, parent: CoroutineScope) {
    private val job = SupervisorJob(parent.coroutineContext[Job])
    private val scope = CoroutineScope(parent.coroutineContext + job)
    val value = MutableStateFlow(TextFieldValue())
    val includeEnter = MutableStateFlow(false)
    val opened = MutableStateFlow(false)
    val sending = MutableStateFlow(false)
    val error = MutableStateFlow<String?>(null)
    private var target: InputTarget? = null
    /** The session captured on open; the dialog shows it so the destination is never ambiguous. */
    val targetSession: String? get() = target?.session

    fun open() {
        if (!sending.value) { target = client.captureInputTarget(); error.value = null }
        opened.value = true
    }
    fun dismiss() { opened.value = false }
    fun edit(next: TextFieldValue) { if (!sending.value) { value.value = next; error.value = null } }

    @Synchronized fun send(current: () -> Boolean, preparePaste: suspend (String) -> ByteArray) {
        if (sending.value || !opened.value) return
        val text = value.value.text
        if (text.isEmpty()) { error.value = t("input.empty_text"); return }
        if (text.encodeToByteArray().size > TerminalInput.MAX_BYTES) { error.value = t("input.too_large"); return }
        val destination = target
        fun matches() = destination != null && current() && destination == client.captureInputTarget()
        if (!matches()) { error.value = t("input.send_cancelled_target_changed_draft_kept"); return }
        sending.value = true
        val enter = includeEnter.value
        scope.launch {
            var textSent = false
            fun failure(message: String) = if (textSent && enter) tr(
                "input.enter_failed_after_text", message
            ) else message
            try {
                val bytes = preparePaste(text)
                require(bytes.isNotEmpty() && bytes.size <= TerminalInput.MAX_BYTES) { t("input.too_large_with_paste") }
                if (!matches()) {
                    error.value = t("input.send_cancelled_target_changed")
                } else {
                    val result = client.sendInput(destination!!, bytes)
                    error.value = result.error
                    if (result.error != null) return@launch
                    textSent = true
                    if (enter) {
                        check(matches()) { t("input.cancelled_target_changed") }
                        val key = checkNotNull(client.encodeKeyboard("enter", null, TerminalModifiers(), false)) {
                            t("input.enter_unsupported")
                        }
                        val entered = client.sendInput(destination, key)
                        if (entered.error != null) {
                            error.value = failure(entered.error)
                            return@launch
                        }
                    }
                    value.value = TextFieldValue(); includeEnter.value = false; opened.value = false
                }
            } catch (_: TimeoutCancellationException) { error.value = failure(t("input.prepare_timeout_draft_kept")) }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (cause: Exception) { error.value = failure(cause.message ?: t("input.send_unconfirmed_draft_kept")) }
            finally { sending.value = false }
        }
    }
    fun close() { job.cancel(); value.value = TextFieldValue(); includeEnter.value = false; opened.value = false; target = null }
}
