package kim.jujin.dglc.core

/**
 * Minimal read-only OpenType/TrueType access for installed terminal fonts: the family
 * name of one face, and that face as a standalone font a WebView `FontFace` accepts.
 * [read] returns exactly `length` bytes at `offset`, or throws.
 */
class OpenTypeFont(private val size: Long, private val read: (offset: Long, length: Int) -> ByteArray) {
    constructor(bytes: ByteArray) : this(bytes.size.toLong(), { offset, length ->
        require(offset >= 0 && length >= 0 && offset + length <= bytes.size)
        bytes.copyOfRange(offset.toInt(), offset.toInt() + length)
    })

    private class Table(val tag: String, val checksum: Long, val offset: Long, val length: Int)

    private fun bytes(offset: Long, length: Int): ByteArray {
        require(offset >= 0 && length >= 0 && offset + length <= size) { "Font data out of range" }
        return read(offset, length).also { require(it.size == length) }
    }
    private fun u16(b: ByteArray, at: Int) = ((b[at].toInt() and 0xff) shl 8) or (b[at + 1].toInt() and 0xff)
    private fun u32(b: ByteArray, at: Int) = (u16(b, at).toLong() shl 16) or u16(b, at + 2).toLong()

    /** Offset of face [index]'s table directory; a plain sfnt has only face 0. */
    private fun faceOffset(index: Int): Long {
        val header = bytes(0, 12)
        if (header.decodeToString(0, 4) != "ttcf") {
            require(index == 0) { "Not a font collection" }
            return 0
        }
        val count = u32(header, 8)
        require(index >= 0 && index < count && count <= 4096) { "Font index out of range" }
        return u32(bytes(12 + 4L * index, 4), 0)
    }

    private fun tables(index: Int): Pair<ByteArray, List<Table>> {
        val offset = faceOffset(index)
        val header = bytes(offset, 12)
        val count = u16(header, 4)
        require(count in 1..512) { "Invalid table count" }
        val records = bytes(offset + 12, 16 * count)
        val tables = (0 until count).map { i ->
            val at = 16 * i
            val length = u32(records, at + 12)
            require(length <= Int.MAX_VALUE)
            Table(records.decodeToString(at, at + 4), u32(records, at + 4), u32(records, at + 8), length.toInt())
        }
        return header to tables
    }

    /** Typographic family (name ID 16) when present, otherwise the family (name ID 1). */
    fun familyName(index: Int = 0): String? {
        val table = tables(index).second.firstOrNull { it.tag == "name" } ?: return null
        require(table.length in 6..MAX_NAME_TABLE)
        val name = bytes(table.offset, table.length)
        val count = u16(name, 2)
        val strings = u16(name, 4)
        require(6 + 12 * count <= name.size)
        data class Record(val platform: Int, val encoding: Int, val language: Int, val id: Int, val length: Int, val offset: Int)
        val records = (0 until count).map { i ->
            val at = 6 + 12 * i
            Record(u16(name, at), u16(name, at + 2), u16(name, at + 4), u16(name, at + 6), u16(name, at + 8), u16(name, at + 10))
        }
        fun decode(r: Record): String? {
            val start = strings + r.offset
            if (r.length == 0 || start + r.length > name.size) return null
            val raw = name.copyOfRange(start, start + r.length)
            val text = if (r.platform == 0 || (r.platform == 3 && (r.encoding == 1 || r.encoding == 10))) {
                if (raw.size % 2 != 0) return null
                CharArray(raw.size / 2) { ((raw[2 * it].toInt() and 0xff) shl 8 or (raw[2 * it + 1].toInt() and 0xff)).toChar() }.concatToString()
            } else if (r.platform == 1 && r.encoding == 0) {
                // Mac Roman; family names in this encoding are ASCII in practice.
                if (raw.any { it < 0x20 || it > 0x7e }) return null
                raw.decodeToString()
            } else return null
            return text.trim().takeIf { it.isNotEmpty() && it.length <= 256 && it.none { c -> c.isISOControl() } }
        }
        for (id in listOf(16, 1)) {
            val candidates = records.filter { it.id == id }
            val ordered = candidates.filter { it.platform == 3 && it.language == 0x409 } +
                candidates.filter { it.platform == 3 } + candidates.filter { it.platform == 0 } +
                candidates.filter { it.platform == 1 && it.language == 0 }
            ordered.firstNotNullOfOrNull(::decode)?.let { return it }
        }
        return null
    }

    /** Face [index] as one standalone sfnt: collections are rebuilt with only its tables. */
    fun standalone(index: Int = 0): ByteArray {
        val (header, tables) = tables(index)
        if (bytes(0, 4).decodeToString() != "ttcf") {
            require(size <= MAX_FONT_BYTES) { "Font too large" }
            return bytes(0, size.toInt())
        }
        val directory = 12 + 16 * tables.size
        var total = directory.toLong()
        val placed = tables.map { table -> total.also { total += (table.length + 3) and 3.inv() } }
        require(total <= MAX_FONT_BYTES) { "Font too large" }
        val out = ByteArray(total.toInt())
        header.copyInto(out, 0, 0, 12)
        fun put32(at: Int, value: Long) { for (i in 0..3) out[at + i] = (value shr (24 - 8 * i)).toByte() }
        tables.forEachIndexed { i, table ->
            val at = 12 + 16 * i
            table.tag.encodeToByteArray().copyInto(out, at)
            put32(at + 4, table.checksum)
            put32(at + 8, placed[i])
            put32(at + 12, table.length.toLong())
            bytes(table.offset, table.length).copyInto(out, placed[i].toInt())
        }
        return out
    }

    companion object {
        const val MAX_FONT_BYTES = 64L * 1024 * 1024
        private const val MAX_NAME_TABLE = 1024 * 1024
        /** Same tolerance as the desktop: every advance within 1% of the first. */
        fun sameAdvance(advances: List<Float>): Boolean {
            val first = advances.firstOrNull()?.takeIf { it > 0f && it.isFinite() } ?: return false
            return advances.all { kotlin.math.abs(it - first) <= first * 0.01f }
        }
    }
}
