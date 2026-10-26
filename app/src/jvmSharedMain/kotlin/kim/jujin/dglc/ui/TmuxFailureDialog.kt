package kim.jujin.dglc.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.core.*

@Composable internal fun TmuxInfoButton(onClick: () -> Unit) {
    val color = MaterialTheme.colorScheme.onSurfaceVariant
    IconButton(onClick, modifier = Modifier.semantics { contentDescription = t("tmux 실패 상세 정보") }) {
        Canvas(Modifier.size(20.dp)) {
            drawCircle(color, radius = 9.dp.toPx(), style = Stroke(1.5.dp.toPx()))
            drawCircle(color, radius = 1.dp.toPx(), center = Offset(center.x, center.y - 4.dp.toPx()))
            drawLine(color, Offset(center.x, center.y), Offset(center.x, center.y + 4.dp.toPx()), 1.8.dp.toPx(), StrokeCap.Round)
        }
    }
}

@Composable internal fun TmuxFailureDialog(host: HostProfile, problem: TmuxProblem, dismiss: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text(problem.label) }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(problem.explanation)
            Text(tr("호스트: {0}\n{1}@{2}:{3}", host.alias, host.username, host.hostname, host.port), style = MaterialTheme.typography.bodySmall)
            if (problem.kind == TmuxProblemKind.NotFound) Text(t("tmux가 설치되어 있어도 SSH 계정, 실행 권한 또는 PATH 설정에 따라 앱에서 찾지 못할 수 있습니다."))
            problem.diagnostics.executable?.let {
                Text(t("찾은 실행 파일"), style = MaterialTheme.typography.labelLarge)
                SelectionContainer { Text(it, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace) }
            }
            problem.version?.let { Text(tr("확인된 버전: {0}", it)) }
            problem.exitStatus?.let { Text(tr("실행 종료 코드: {0}", it), style = MaterialTheme.typography.bodySmall) }
            if (problem.diagnostics.attempts.isNotEmpty()) {
                Text(t("탐지 시도"), style = MaterialTheme.typography.titleSmall)
                problem.diagnostics.attempts.forEach { attempt ->
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        val outcome = when (attempt.outcome) {
                            TmuxProbeOutcome.Found -> t("실행 파일 발견")
                            TmuxProbeOutcome.NotFound -> t("실행 가능한 파일을 찾지 못함")
                            TmuxProbeOutcome.Failed -> t("확인 명령 실패") + (attempt.exitStatus?.let { tr(" (종료 코드 {0})", it) } ?: "")
                            TmuxProbeOutcome.InvalidResponse -> t("경로 응답을 확인하지 못함")
                            TmuxProbeOutcome.TimedOut -> t("응답 시간 초과")
                        }
                        Text("${attempt.location} · $outcome", style = MaterialTheme.typography.bodySmall)
                        if (attempt.paths.isNotEmpty()) SelectionContainer {
                            Text(attempt.paths.joinToString("\n"), style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }
            HorizontalDivider()
            Text(t("호스트에서 확인할 내용"), style = MaterialTheme.typography.titleSmall)
            Text(t("앱에 등록한 것과 같은 SSH 계정으로 아래 명령을 실행하세요."))
            SelectionContainer { Text("command -v tmux\ntmux -V", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall) }
            Text(when (problem.kind) {
                TmuxProblemKind.NotFound -> t("tmux가 다른 위치에 있다면 해당 디렉터리를 로그인 셸의 PATH에 추가하고 실행 권한을 확인하세요. 설치되어 있지 않다면 호스트에 tmux를 설치하세요.")
                TmuxProblemKind.ExecutionFailed -> t("표시된 실행 파일의 권한과 필요한 라이브러리, 호스트 터미널에서 출력되는 오류를 확인하세요.")
                TmuxProblemKind.UnsupportedVersion -> t("현재 앱은 tmux 3.3–3.7을 지원합니다. 사용 중인 tmux 버전과 실행 경로를 확인하세요.")
                TmuxProblemKind.InvalidVersion -> t("버전 명령의 출력과 tmux 래퍼 또는 셸 시작 설정을 확인하세요.")
            })
            Text(t("확인 후 호스트에 다시 연결하면 tmux를 다시 탐지합니다."))
            Text(t("현재는 일반 SSH 셸로 연결되어 있습니다. 이 상태의 작업은 연결을 종료하면 끝날 수 있습니다."), style = MaterialTheme.typography.bodySmall)
        }
    }, confirmButton = { TextButton(dismiss) { Text(t("닫기")) } })
}
