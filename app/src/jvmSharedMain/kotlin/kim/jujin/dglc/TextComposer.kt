package kim.jujin.dglc

import kim.jujin.dglc.core.t
import kim.jujin.dglc.core.tr
import androidx.compose.ui.text.input.TextFieldValue
import kim.jujin.dglc.core.TerminalInput
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

    fun open() {
        if (!sending.value) { target = client.captureInputTarget(); error.value = null }
        opened.value = true
    }
    fun dismiss() { opened.value = false }
    fun edit(next: TextFieldValue) { if (!sending.value) { value.value = next; error.value = null } }

    @Synchronized fun send(current: () -> Boolean, preparePaste: suspend (String, Boolean) -> ByteArray) {
        if (sending.value || !opened.value) return
        val text = value.value.text
        if (text.isEmpty()) { error.value = t("입력할 텍스트를 작성하세요."); return }
        if (text.encodeToByteArray().size > TerminalInput.MAX_BYTES) { error.value = t("입력은 UTF-8 64 KiB 이하여야 합니다."); return }
        val destination = target
        fun matches() = destination != null && current() && destination == client.captureInputTarget()
        if (!matches()) { error.value = t("연결·세션·패널이 변경되어 전송을 취소했습니다. 초안을 확인한 뒤 다시 여세요."); return }
        sending.value = true
        val enter = includeEnter.value
        scope.launch {
            try {
                val bytes = preparePaste(text, enter)
                require(bytes.isNotEmpty() && bytes.size <= TerminalInput.MAX_BYTES) { t("입력은 붙여넣기 제어 문자와 Enter를 포함하여 UTF-8 64 KiB 이하여야 합니다.") }
                if (!matches()) {
                    error.value = t("연결·세션·패널이 변경되어 전송을 취소했습니다. 초안은 유지됩니다.")
                } else {
                    val result = client.sendInput(destination!!, bytes)
                    error.value = result.error
                    if (result.error == null) {
                        value.value = TextFieldValue(); includeEnter.value = false; opened.value = false
                    }
                }
            } catch (_: TimeoutCancellationException) { error.value = t("터미널 입력 준비 시간이 초과되었습니다. 초안은 유지됩니다.") }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (failure: Exception) { error.value = failure.message ?: t("전송 결과를 확인하지 못했습니다. 초안은 유지되며 자동 재전송하지 않습니다.") }
            finally { sending.value = false }
        }
    }
    fun close() { job.cancel(); value.value = TextFieldValue(); includeEnter.value = false; opened.value = false; target = null }
}
