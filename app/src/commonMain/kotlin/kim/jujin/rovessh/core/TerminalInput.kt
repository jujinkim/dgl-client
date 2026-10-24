package kim.jujin.rovessh.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Stable serialized key names; labels and terminal sequences are shared by both UIs. */
@Serializable enum class TerminalKey(val label: String, private val sequence: String) {
    Escape("Esc", "\u001b"), Tab("Tab", "\t"),
    CtrlA("Ctrl+A", "\u0001"), CtrlB("Ctrl+B", "\u0002"), CtrlC("Ctrl+C", "\u0003"),
    CtrlD("Ctrl+D", "\u0004"), CtrlE("Ctrl+E", "\u0005"), CtrlF("Ctrl+F", "\u0006"),
    CtrlG("Ctrl+G", "\u0007"), CtrlH("Ctrl+H", "\u0008"), CtrlI("Ctrl+I", "\t"),
    CtrlJ("Ctrl+J", "\n"), CtrlK("Ctrl+K", "\u000b"), CtrlL("Ctrl+L", "\u000c"),
    CtrlM("Ctrl+M", "\r"), CtrlN("Ctrl+N", "\u000e"), CtrlO("Ctrl+O", "\u000f"),
    CtrlP("Ctrl+P", "\u0010"), CtrlQ("Ctrl+Q", "\u0011"), CtrlR("Ctrl+R", "\u0012"),
    CtrlS("Ctrl+S", "\u0013"), CtrlT("Ctrl+T", "\u0014"), CtrlU("Ctrl+U", "\u0015"),
    CtrlV("Ctrl+V", "\u0016"), CtrlW("Ctrl+W", "\u0017"), CtrlX("Ctrl+X", "\u0018"),
    CtrlY("Ctrl+Y", "\u0019"), CtrlZ("Ctrl+Z", "\u001a"),
    Up("↑", "\u001b[A"), Down("↓", "\u001b[B"), Left("←", "\u001b[D"), Right("→", "\u001b[C"),
    Home("Home", "\u001b[H"), End("End", "\u001b[F"), Enter("Enter", "\r"),
    Backspace("Backspace", "\u007f"), Delete("Delete", "\u001b[3~"),
    PageUp("PageUp", "\u001b[5~"), PageDown("PageDown", "\u001b[6~");

    fun bytes(): ByteArray = sequence.encodeToByteArray()

    companion object {
        val toolbar = listOf(Escape, Tab, CtrlC, CtrlD, CtrlL, Up, Down, Left, Right, Home, End)
    }
}

@Serializable sealed interface MacroStep {
    @Serializable @SerialName("text") data class Text(val text: String) : MacroStep
    @Serializable @SerialName("key") data class Key(val key: TerminalKey) : MacroStep
}

@Serializable enum class VolumeAction { Default, Shortcut, Macro }
@Serializable data class VolumeBinding(
    val action: VolumeAction = VolumeAction.Default,
    val steps: List<MacroStep> = emptyList(),
) {
    fun bytes(): ByteArray? = when (action) {
        VolumeAction.Default -> null
        VolumeAction.Shortcut -> {
            require(steps.size == 1 && steps.single() is MacroStep.Key) { "단축키를 선택하세요." }
            TerminalInput.encode(steps)
        }
        VolumeAction.Macro -> TerminalInput.encode(steps)
    }
}

object TerminalInput {
    const val MAX_BYTES = 64 * 1024

    /** xterm onBinary strings store one raw byte per character, not UTF-8 text. */
    fun decodeBinary(data: String): ByteArray {
        require(data.length <= MAX_BYTES && data.all { it.code <= 255 }) { "Invalid terminal binary input" }
        return ByteArray(data.length) { data[it].code.toByte() }
    }

    /** Text newlines mean Enter; typed backslash sequences remain literal. No implicit Enter. */
    fun encode(steps: List<MacroStep>): ByteArray {
        var size = 0
        val parts = steps.map { step ->
            val bytes = when (step) {
                is MacroStep.Text -> step.text.replace("\r\n", "\n").replace('\r', '\n').replace('\n', '\r').encodeToByteArray()
                is MacroStep.Key -> step.key.bytes()
            }
            require(bytes.size <= MAX_BYTES - size) { "매크로는 UTF-8 변환 후 64 KiB 이하여야 합니다." }
            size += bytes.size
            bytes
        }
        require(size > 0) { "실행할 텍스트나 키를 추가하세요." }
        return ByteArray(size).also { result ->
            var offset = 0
            parts.forEach { it.copyInto(result, offset); offset += it.size }
        }
    }
}
