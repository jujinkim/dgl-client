package kim.jujin.dglc

import kim.jujin.dglc.core.*

fun nativeInputError(code: String): String = when (code) {
    "StaleGeneration" -> t("연결·세션·패널이 변경되어 입력을 취소했습니다.")
    "NotReady" -> t("화면 동기화 또는 연결 복구 중에는 입력할 수 없습니다. 입력을 다시 전송하지 않습니다.")
    "QueueFull" -> t("입력 대기열이 가득 찼습니다. 입력을 다시 전송하지 않습니다.")
    "Cancelled", "Closed" -> t("입력이 취소되었거나 연결이 종료되었습니다. 입력을 다시 전송하지 않습니다.")
    "InvalidArgument" -> t("입력은 1바이트 이상, 64 KiB 이하여야 합니다.")
    else -> t("입력을 전송하지 않았습니다. 대기열 또는 연결 상태를 확인하세요.")
}

/** One nonblocking notice per recovery cycle, even if the user dismisses it. */
class InputRejections {
    private var recovering = false
    private var shown = false
    fun phase(phase: Phase) {
        val next = phase == Phase.Restoring || phase == Phase.Reconnecting
        if (next != recovering) shown = false
        recovering = next
    }
    fun reject(code: String): String? {
        if (recovering && shown) return null
        shown = true
        return nativeInputError(code)
    }
}
