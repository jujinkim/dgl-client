package kim.jujin.dglc.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

@Composable internal fun TmuxInfoButton(onClick: () -> Unit) {
    val color = MaterialTheme.colorScheme.onSurfaceVariant
    IconButton(onClick, modifier = Modifier.semantics { contentDescription = t("tmux.failure_details") }) {
        UiGlyph(UiIcons.info, tint = color)
    }
}

@Composable internal fun TmuxFailureDialog(host: HostProfile, problem: TmuxProblem, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(problem.label) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(problem.explanation)
            Text(tr("tmux.host_details", host.alias, host.username, host.hostname, host.port), style = MaterialTheme.typography.bodySmall)
            if (problem.kind == TmuxProblemKind.NotFound) Text(t("tmux.help_environment_note"))
            problem.diagnostics.executable?.let {
                Text(t("tmux.detected_executable"), style = MaterialTheme.typography.labelLarge)
                SelectionContainer { Text(it, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace) }
            }
            problem.version?.let { Text(tr("tmux.detected_version", it)) }
            problem.exitStatus?.let { Text(tr("tmux.exit_code", it), style = MaterialTheme.typography.bodySmall) }
            if (problem.diagnostics.attempts.isNotEmpty()) {
                Text(t("tmux.detection_attempts"), style = MaterialTheme.typography.titleSmall)
                problem.diagnostics.attempts.forEach { attempt ->
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        val outcome = when (attempt.outcome) {
                            TmuxProbeOutcome.Found -> t("tmux.probe_found")
                            TmuxProbeOutcome.NotFound -> t("tmux.probe_not_found")
                            TmuxProbeOutcome.Failed -> t("tmux.probe_command_failed") + (attempt.exitStatus?.let { tr("tmux.exit_code_suffix", it) } ?: "")
                            TmuxProbeOutcome.InvalidResponse -> t("tmux.probe_path_unreadable")
                            TmuxProbeOutcome.TimedOut -> t("tmux.probe_timed_out")
                        }
                        Text("${attempt.location} · $outcome", style = MaterialTheme.typography.bodySmall)
                        if (attempt.paths.isNotEmpty()) SelectionContainer {
                            Text(attempt.paths.joinToString("\n"), style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }
            HorizontalDivider()
            Text(t("tmux.help_title"), style = MaterialTheme.typography.titleSmall)
            Text(t("tmux.help_run_commands"))
            SelectionContainer { Text("command -v tmux\ntmux -V", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall) }
            Text(when (problem.kind) {
                TmuxProblemKind.NotFound -> t("tmux.help_add_to_path")
                TmuxProblemKind.ExecutionFailed -> t("tmux.help_run_failed")
                TmuxProblemKind.UnsupportedVersion -> t("tmux.help_unsupported_version")
                TmuxProblemKind.InvalidVersion -> t("tmux.help_invalid_version")
            })
            Text(t("tmux.help_reconnect"))
            Text(t("tmux.plain_shell_note"), style = MaterialTheme.typography.bodySmall)
        }
    }, confirmButton = { BorderedButton(dismiss) { Text(t("common.close")) } })
}
