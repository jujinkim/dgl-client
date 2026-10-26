package kim.jujin.dglc.core

enum class TmuxProbeOutcome { Found, NotFound, Failed, InvalidResponse, TimedOut }
data class TmuxProbeAttempt(
    val location: String,
    val outcome: TmuxProbeOutcome,
    val paths: List<String> = emptyList(),
    val exitStatus: Int? = null,
)
data class TmuxDiagnostics(val attempts: List<TmuxProbeAttempt> = emptyList(), val executable: String? = null)
enum class TmuxProblemKind { NotFound, ExecutionFailed, UnsupportedVersion, InvalidVersion }
data class TmuxProblem(
    val kind: TmuxProblemKind,
    val diagnostics: TmuxDiagnostics = TmuxDiagnostics(),
    val version: String? = null,
    val exitStatus: Int? = null,
) {
    val label get() = when (kind) {
        TmuxProblemKind.NotFound -> t("tmux 탐지 실패")
        TmuxProblemKind.ExecutionFailed -> t("tmux 실행 실패")
        TmuxProblemKind.UnsupportedVersion -> t("tmux 버전 미지원")
        TmuxProblemKind.InvalidVersion -> t("tmux 버전 확인 실패")
    }
    val explanation get() = when (kind) {
        TmuxProblemKind.NotFound -> t("이 호스트에서 실행할 수 있는 tmux를 찾지 못했습니다.")
        TmuxProblemKind.ExecutionFailed -> t("tmux 실행 파일을 찾았지만 버전 확인 명령을 실행하지 못했습니다.")
        TmuxProblemKind.UnsupportedVersion -> t("호스트의 tmux 버전이 앱에서 지원하는 범위(3.3–3.7)에 포함되지 않습니다.")
        TmuxProblemKind.InvalidVersion -> t("호스트의 응답에서 tmux 버전을 확인하지 못했습니다.")
    }
}
