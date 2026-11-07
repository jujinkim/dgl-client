package kim.jujin.dglc.ui

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape

/**
 * Text-labelled buttons keep an outline so they read as buttons. Icon-only buttons, list rows,
 * tabs and menu items use their own containers. The padding matches TextButton so compact rows
 * keep their layout.
 */
@Composable internal fun BorderedButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true,
    shape: Shape = ButtonDefaults.outlinedShape, contentPadding: PaddingValues = ButtonDefaults.TextButtonContentPadding,
    content: @Composable RowScope.() -> Unit) =
    OutlinedButton(onClick, modifier, enabled, shape, contentPadding = contentPadding, content = content)
