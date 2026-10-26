package kim.jujin.dglc.storage

import kim.jujin.dglc.core.*
import kotlinx.serialization.json.Json
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

class LocalRepository(private val directory: File) {
    private val file = File(directory, "settings.json")
    private val json = Json { ignoreUnknownKeys = true; prettyPrint = true }
    @Synchronized fun read(): LocalState {
        if (!file.exists()) return LocalState()
        check(file.length() <= MAX_FILE_BYTES) { "Settings file exceeds limit" }
        return json.decodeFromString<LocalState>(file.readText())
    }
    @Synchronized fun save(value: LocalState) {
        directory.mkdirs()
        val temporary = File(directory, "settings.tmp")
        val bytes = json.encodeToString(LocalState.serializer(), value).encodeToByteArray()
        require(bytes.size <= MAX_FILE_BYTES) { t("설정 파일이 너무 큽니다.") }
        temporary.writeBytes(bytes)
        try { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING) }
        catch (_: java.nio.file.AtomicMoveNotSupportedException) { Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING) }
    }
    // Two 64 KiB macros may contain one serialized key step per byte, with JSON indentation.
    private companion object { const val MAX_FILE_BYTES = 16 * 1024 * 1024 }
}
