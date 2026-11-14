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
        TmuxProblemKind.NotFound -> t("tmux.problem_not_detected")
        TmuxProblemKind.ExecutionFailed -> t("tmux.problem_run_failed")
        TmuxProblemKind.UnsupportedVersion -> t("tmux.problem_unsupported")
        TmuxProblemKind.InvalidVersion -> t("tmux.problem_invalid_version")
    }
    val explanation get() = when (kind) {
        TmuxProblemKind.NotFound -> t("tmux.problem_not_detected_detail")
        TmuxProblemKind.ExecutionFailed -> t("tmux.problem_run_failed_detail")
        TmuxProblemKind.UnsupportedVersion -> t("tmux.problem_unsupported_detail")
        TmuxProblemKind.InvalidVersion -> t("tmux.problem_invalid_version_detail")
    }
}
