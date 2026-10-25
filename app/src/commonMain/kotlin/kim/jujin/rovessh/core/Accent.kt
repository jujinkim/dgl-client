package kim.jujin.rovessh.core

import kotlinx.serialization.*
import kotlinx.serialization.descriptors.*
import kotlinx.serialization.encoding.*
import kotlinx.serialization.json.*
import kotlin.math.*

@Serializable(with = AccentSettingsSerializer::class)
data class AccentSettings(val mode: String = "system", val customColor: String? = null) {
    fun resolve(dark: Boolean, system: Int?): Int = when (mode) {
        "system" -> system?.takeIf { it in 0..0xffffff }
        "custom" -> parseAccentHex(customColor ?: "")
        else -> null
    } ?: if (dark) 0x75aaff else 0x2468c9
}
fun normalizeAccentHex(value: String): String? = value.trim().takeIf {
    it.length == 7 && it[0] == '#' && it.drop(1).all { c -> c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F' }
}?.uppercase()
fun parseAccentHex(value: String): Int? = normalizeAccentHex(value)?.drop(1)?.toIntOrNull(16)

object AccentSettingsSerializer : KSerializer<AccentSettings> {
    override val descriptor: SerialDescriptor = JsonElement.serializer().descriptor
    override fun deserialize(decoder: Decoder): AccentSettings {
        val element = (decoder as JsonDecoder).decodeJsonElement()
        return runCatching {
            val value = element.jsonObject
            val mode = value["mode"]?.jsonPrimitive?.also { require(it.isString) }?.content ?: "system"
            require(mode in listOf("system", "default", "custom"))
            val color = value["customColor"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.let {
                require(it.isString); requireNotNull(normalizeAccentHex(it.content))
            }
            AccentSettings(mode, color)
        }.getOrDefault(AccentSettings())
    }
    override fun serialize(encoder: Encoder, value: AccentSettings) {
        (encoder as JsonEncoder).encodeJsonElement(buildJsonObject {
            put("mode", value.mode)
            value.customColor?.let { put("customColor", normalizeAccentHex(it) ?: it) }
        })
    }
}
val accentPresets = listOf("파랑" to "#2468C9", "보라" to "#8056D9", "분홍" to "#D84F99", "빨강" to "#D63D45",
    "주황" to "#E57B25", "노랑" to "#E6BB24", "초록" to "#329654", "청록" to "#21999D")
fun blendAccent(color: Int, background: Int, amount: Double): Int = listOf(16, 8, 0).fold(0) { out, shift ->
    out or ((((color shr shift and 255) * amount + (background shr shift and 255) * (1 - amount)).roundToInt()) shl shift)
}
private fun luminance(color: Int): Double = listOf(16, 8, 0).zip(listOf(0.2126, 0.7152, 0.0722)).sumOf { (shift, weight) ->
    val c = (color shr shift and 255) / 255.0
    weight * if (c <= 0.04045) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)
}
fun accentContrast(a: Int, b: Int): Double {
    val x = luminance(a); val y = luminance(b)
    return (max(x, y) + 0.05) / (min(x, y) + 0.05)
}
fun onAccentColor(color: Int): Int = if (accentContrast(0, color) >= accentContrast(0xffffff, color)) 0 else 0xffffff
fun readableAccent(color: Int, backgrounds: List<Int>, ratio: Double = 4.5): Int {
    val target = onAccentColor(backgrounds.first())
    return (0..100).asSequence().map { blendAccent(target, color, it / 100.0) }
        .firstOrNull { c -> backgrounds.all { accentContrast(c, it) >= ratio } } ?: target
}
