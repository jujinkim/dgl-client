package kim.jujin.dglc.ssh

import kim.jujin.dglc.core.*
import kotlinx.coroutines.withTimeoutOrNull

class TmuxDiscoveryException(val diagnostics: TmuxDiagnostics) : Exception("호스트에서 실행 가능한 tmux를 찾지 못했습니다.")

/** Resolve once per SSH connection; never run login startup output in a control stream. */
internal object TmuxDiscovery {
    private const val marker = "\u001eDGL_TMUX_PATH\u001f"
    private const val printPath = "printf '\\036DGL_TMUX_PATH\\037%s\\036' \"\$dgl_tmux_path\""
    private val probe = "dgl_tmux_path=\$(command -v tmux) || exit 127; " +
        "case \"\$dgl_tmux_path\" in /*) ;; *) exit 127 ;; esac; " +
        "[ -f \"\$dgl_tmux_path\" ] && [ -x \"\$dgl_tmux_path\" ] || exit 126; $printPath"
    internal val direct = "/bin/sh -c " + TmuxCommand.quote(probe)
    // The login shell loads the user's exported PATH. The inner sh ignores aliases
    // and functions, so the result is an executable path we can safely quote later.
    internal val login = "/bin/sh -c " + TmuxCommand.quote(
        "exec \"\${SHELL:-/bin/sh}\" -lc " + TmuxCommand.quote("exec $direct"))
    internal val commonPaths = listOf(
        "/opt/homebrew/bin/tmux", "/usr/local/bin/tmux", "/usr/bin/tmux", "/bin/tmux",
        "/opt/local/bin/tmux", "/home/linuxbrew/.linuxbrew/bin/tmux",
        "/run/current-system/sw/bin/tmux", "/snap/bin/tmux",
        "~/.local/bin/tmux", "~/bin/tmux", "~/.linuxbrew/bin/tmux", "~/.nix-profile/bin/tmux",
    )
    internal val common = commonCommand(commonPaths)

    internal fun commonCommand(paths: List<String>): String {
        val candidates = paths.joinToString(" ") {
            if (it.startsWith("~/")) "\"\$HOME/${it.removePrefix("~/")}\"" else TmuxCommand.quote(it)
        }
        return "/bin/sh -c " + TmuxCommand.quote(
            "for dgl_tmux_path in $candidates; do " +
                "if [ -f \"\$dgl_tmux_path\" ] && [ -x \"\$dgl_tmux_path\" ]; then $printPath; exit 0; fi; " +
                "done; exit 127")
    }

    suspend fun find(timeoutMillis: Long = 4_000, execute: suspend (String) -> ByteArray): TmuxDiagnostics {
        val attempts = mutableListOf<TmuxProbeAttempt>()
        val probes = listOf(
            Triple("SSH 기본 PATH", direct, emptyList()),
            Triple("로그인 셸 PATH", login, emptyList()),
            Triple("대표 설치 경로", common, commonPaths),
        )
        for ((location, command, paths) in probes) {
            val output = try { withTimeoutOrNull(timeoutMillis) { execute(command).decodeToString() } }
            catch (e: RemoteCommandException) {
                attempts += TmuxProbeAttempt(location, if (e.exitStatus == 127) TmuxProbeOutcome.NotFound else TmuxProbeOutcome.Failed, paths, e.exitStatus)
                continue
            }
            if (output == null) {
                attempts += TmuxProbeAttempt(location, TmuxProbeOutcome.TimedOut, paths)
                continue
            }
            val path = parsePath(output)
            if (path != null) return TmuxDiagnostics(attempts + TmuxProbeAttempt(location, TmuxProbeOutcome.Found, paths), path)
            attempts += TmuxProbeAttempt(location, TmuxProbeOutcome.InvalidResponse, paths)
        }
        throw TmuxDiscoveryException(TmuxDiagnostics(attempts.toList()))
    }

    private fun parsePath(output: String): String? {
        val start = output.indexOf(marker)
        if (start < 0 || output.indexOf(marker, start + marker.length) >= 0) return null
        val end = output.indexOf('\u001e', start + marker.length)
        if (end < 0) return null
        return output.substring(start + marker.length, end).takeIf { path ->
            path.startsWith('/') && path.length <= 1024 && path.none { it.code < 32 || it.code == 127 }
        }
    }
}
