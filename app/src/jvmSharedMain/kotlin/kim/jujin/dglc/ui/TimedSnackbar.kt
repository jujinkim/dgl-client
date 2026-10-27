package kim.jujin.dglc.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import kim.jujin.dglc.SnackbarState
import kim.jujin.dglc.core.t

@Composable internal fun TimedSnackbar(state: SnackbarState, modifier: Modifier = Modifier) {
    val message by state.current.collectAsState()
    val displayed = message ?: return
    var remaining by remember(state, displayed.id) { mutableStateOf(state.remainingFraction(displayed.id)) }
    LaunchedEffect(state, displayed.id) {
        if (displayed.duration.millis != null) {
            do {
                remaining = state.tick(displayed.id)
                if (remaining != null && remaining!! > 0f) withFrameNanos { }
            } while (remaining != null && remaining!! > 0f)
        }
    }
    if (remaining != null && remaining!! <= 0f) return
    Column(modifier) {
        Snackbar(action = {
            TextButton({ state.dismiss(displayed.id) }) { Text(t("닫기")) }
        }) { Text(displayed.message) }
        remaining?.let { fraction ->
            val color = MaterialTheme.colorScheme.inverseOnSurface
            Box(Modifier.fillMaxWidth().height(3.dp)
                .background(MaterialTheme.colorScheme.inverseSurface).clearAndSetSemantics { }) {
                Box(Modifier.fillMaxSize().background(color.copy(alpha = 0.2f)))
                Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(color))
            }
        }
    }
}
