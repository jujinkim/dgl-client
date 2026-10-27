package kim.jujin.dglc.ui

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp

/** Decorative glyph; the owning control supplies its accessible name. */
@Composable internal fun UiGlyph(icon: ImageVector, modifier: Modifier = Modifier, tint: Color = LocalContentColor.current) {
    Icon(icon, null, modifier.size(20.dp), tint = tint)
}

internal fun agentIcon(kind: String?): ImageVector = when (kind) {
    "Question" -> UiIcons.question
    "Approval" -> UiIcons.approval
    "Response" -> UiIcons.response
    "Error" -> UiIcons.error
    "Activity" -> UiIcons.refresh
    "Ended" -> UiIcons.ended
    else -> UiIcons.unknown
}
