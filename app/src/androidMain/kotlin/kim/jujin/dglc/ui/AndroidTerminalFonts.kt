package kim.jujin.dglc.ui

import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontFamily
import android.graphics.fonts.FontStyle
import android.graphics.fonts.SystemFonts
import android.graphics.text.TextRunShaper
import kim.jujin.dglc.core.OpenTypeFont
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.io.File
import java.io.RandomAccessFile

/** One installed monospace family: the face closest to regular upright. */
class TerminalFontFile(val family: String, val file: File, val index: Int, val typeface: Typeface) {
    fun sameFace(other: TerminalFontFile?) = other != null && other.file == file && other.index == index
    /** The face as a standalone font for the terminal WebView. */
    fun standalone(): ByteArray = RandomAccessFile(file, "r").use { raf ->
        OpenTypeFont(raf.length()) { offset, length ->
            ByteArray(length).also { raf.seek(offset); raf.readFully(it) }
        }.standalone(index)
    }
}

/** Installed system fonts whose printable ASCII comes from the face itself with one advance. */
object AndroidTerminalFonts {
    private val mutex = Mutex()
    @Volatile private var cached: List<TerminalFontFile>? = null

    suspend fun installed(): List<TerminalFontFile> = cached ?: mutex.withLock {
        cached ?: withContext(Dispatchers.IO) { scan() }.also { cached = it }
    }
    suspend fun find(family: String): TerminalFontFile? =
        family.takeIf { it.isNotEmpty() }?.let { name -> installed().firstOrNull { it.family == name } }

    private fun scan(): List<TerminalFontFile> {
        val best = HashMap<String, Pair<Font, Int>>()
        for (font in SystemFonts.getAvailableFonts()) {
            val file = font.file ?: continue
            val family = try {
                RandomAccessFile(file, "r").use { raf ->
                    OpenTypeFont(raf.length()) { offset, length -> ByteArray(length).also { raf.seek(offset); raf.readFully(it) } }
                        .familyName(font.ttcIndex)
                }
            } catch (_: Exception) { null } ?: continue
            val score = kotlin.math.abs(font.style.weight - FontStyle.FONT_WEIGHT_NORMAL) +
                if (font.style.slant == FontStyle.FONT_SLANT_UPRIGHT) 0 else 1000
            if (best[family]?.let { score < it.second } != false) best[family] = font to score
        }
        return best.mapNotNull { (family, choice) ->
            val font = choice.first
            try {
                val typeface = Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
                if (monospace(font, typeface)) TerminalFontFile(family, font.file!!, font.ttcIndex, typeface) else null
            } catch (_: Exception) { null }
        }.sortedBy { it.family.lowercase() }
    }

    /** Shapes each character alone so coding ligatures cannot merge glyphs. */
    private fun monospace(font: Font, typeface: Typeface): Boolean {
        val paint = Paint().apply { this.typeface = typeface; textSize = 100f }
        val advances = ArrayList<Float>(94)
        for (code in 0x21..0x7e) {
            val text = code.toChar().toString()
            val glyphs = TextRunShaper.shapeTextRun(text, 0, 1, 0, 1, 0f, 0f, false, paint)
            if (glyphs.glyphCount() != 1) return false
            val used = glyphs.getFont(0)
            // A glyph from the system fallback means the face lacks the character.
            if (used.file != font.file || used.ttcIndex != font.ttcIndex) return false
            advances += glyphs.advance
        }
        return OpenTypeFont.sameAdvance(advances)
    }
}
