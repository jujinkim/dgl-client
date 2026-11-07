package kim.jujin.dglc.core

import kotlin.test.*

class OpenTypeFontTest {
    private fun u16(v: Int) = byteArrayOf((v shr 8).toByte(), v.toByte())
    private fun u32(v: Long) = byteArrayOf((v shr 24).toByte(), (v shr 16).toByte(), (v shr 8).toByte(), v.toByte())
    private fun utf16(text: String) = text.flatMap { listOf((it.code shr 8).toByte(), it.code.toByte()) }.toByteArray()

    /** A `name` table: (platform, encoding, language, nameId, bytes). */
    private fun nameTable(vararg records: Array<Any>): ByteArray {
        val strings = records.map { it[4] as ByteArray }
        var offset = 0
        val head = u16(0) + u16(records.size) + u16(6 + 12 * records.size)
        val entries = records.mapIndexed { i, r ->
            (u16(r[0] as Int) + u16(r[1] as Int) + u16(r[2] as Int) + u16(r[3] as Int) + u16(strings[i].size) + u16(offset))
                .also { offset += strings[i].size }
        }
        return entries.fold(head) { a, b -> a + b } + strings.fold(ByteArray(0)) { a, b -> a + b }
    }

    /** One sfnt (or TTC face directory) whose table data starts at [base]. */
    private fun face(tables: List<Pair<String, ByteArray>>, base: Long, offsets: List<Long>? = null): ByteArray {
        var directory = byteArrayOf(0, 1, 0, 0) + u16(tables.size) + u16(0) + u16(0) + u16(0)
        var next = base + 12 + 16 * tables.size
        tables.forEachIndexed { i, (tag, data) ->
            val at = offsets?.get(i) ?: next.also { next += (data.size + 3) and 3.inv() }
            directory += tag.encodeToByteArray() + u32(0x1234L + i) + u32(at) + u32(data.size.toLong())
        }
        return directory
    }
    private fun padded(data: ByteArray) = data + ByteArray(((data.size + 3) and 3.inv()) - data.size)

    @Test fun familyPrefersTypographicWindowsEnglishThenFallsBack() {
        val name = nameTable(
            arrayOf(1, 0, 0, 1, "Mac Family".encodeToByteArray()),
            arrayOf(3, 1, 0x412, 1, utf16("한글 이름")),
            arrayOf(3, 1, 0x409, 1, utf16("Example Mono Light")),
            arrayOf(3, 1, 0x409, 16, utf16("Example Mono")),
        )
        val glyf = byteArrayOf(1, 2, 3)
        val font = face(listOf("glyf" to glyf, "name" to name), 0) + padded(glyf) + padded(name)
        assertEquals("Example Mono", OpenTypeFont(font).familyName())
        assertContentEquals(font, OpenTypeFont(font).standalone(), "a plain font is served unchanged")
        val legacy = nameTable(arrayOf(1, 0, 0, 1, "Mac Family".encodeToByteArray()), arrayOf(3, 1, 0x409, 2, utf16("Regular")))
        assertEquals("Mac Family", OpenTypeFont(face(listOf("name" to legacy), 0) + padded(legacy)).familyName())
        assertNull(OpenTypeFont(face(listOf("glyf" to glyf), 0) + padded(glyf)).familyName())
        assertFailsWith<IllegalArgumentException> { OpenTypeFont(font).familyName(1) }
    }

    @Test fun collectionFaceBecomesAStandaloneFontWithOnlyItsTables() {
        val shared = byteArrayOf(9, 8, 7, 6, 5)
        val first = nameTable(arrayOf(3, 1, 0x409, 1, utf16("Sans CJK")))
        val second = nameTable(arrayOf(3, 1, 0x409, 1, utf16("Mono CJK")))
        // ttcf header, two directory offsets, two 3-table directories, then table data.
        val dirs = 12 + 8L
        val dataStart = dirs + 2 * (12 + 16 * 2)
        val sharedAt = dataStart; val firstAt = sharedAt + padded(shared).size; val secondAt = firstAt + padded(first).size
        val header = "ttcf".encodeToByteArray() + u32(0x00010000) + u32(2) + u32(dirs) + u32(dirs + 12 + 16 * 2)
        val collection = header +
            face(listOf("glyf" to shared, "name" to first), dirs, listOf(sharedAt, firstAt)) +
            face(listOf("glyf" to shared, "name" to second), dirs + 12 + 16 * 2, listOf(sharedAt, secondAt)) +
            padded(shared) + padded(first) + padded(second)
        val font = OpenTypeFont(collection)
        assertEquals("Sans CJK", font.familyName(0))
        assertEquals("Mono CJK", font.familyName(1))
        assertFailsWith<IllegalArgumentException> { font.familyName(2) }
        val standalone = font.standalone(1)
        assertEquals(0x00010000, standalone.copyOfRange(0, 4).fold(0) { a, b -> (a shl 8) or (b.toInt() and 0xff) })
        val extracted = OpenTypeFont(standalone)
        assertEquals("Mono CJK", extracted.familyName())
        assertContentEquals(standalone, extracted.standalone(), "the extracted face is a plain font")
        assertTrue(standalone.size < collection.size)
        assertEquals(0, standalone.size % 4)
    }

    @Test fun truncatedOrHostileTablesAreRejected() {
        val name = nameTable(arrayOf(3, 1, 0x409, 1, utf16("Cut")))
        val font = face(listOf("name" to name), 0) + padded(name)
        assertFailsWith<IllegalArgumentException> { OpenTypeFont(font.copyOf(font.size - 8)).familyName() }
        assertFailsWith<IllegalArgumentException> { OpenTypeFont(byteArrayOf(0, 1, 0, 0, 0, 0)).familyName() }
        val control = nameTable(arrayOf(3, 1, 0x409, 1, utf16("a\nb")))
        assertNull(OpenTypeFont(face(listOf("name" to control), 0) + padded(control)).familyName())
    }

    @Test fun advancesMustMatchWithinOnePercent() {
        assertTrue(OpenTypeFont.sameAdvance(listOf(600f, 600f, 604f)))
        assertFalse(OpenTypeFont.sameAdvance(listOf(600f, 620f)))
        assertFalse(OpenTypeFont.sameAdvance(listOf(0f, 0f)))
        assertFalse(OpenTypeFont.sameAdvance(emptyList()))
        assertFalse(OpenTypeFont.sameAdvance(listOf(Float.NaN)))
    }
}
