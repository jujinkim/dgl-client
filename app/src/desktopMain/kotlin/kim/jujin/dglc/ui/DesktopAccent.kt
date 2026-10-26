package kim.jujin.dglc.ui

import androidx.compose.runtime.Composable

// JVM UI is retained for fixtures only; production desktop resolves native colors in Rust.
@Composable actual fun PlatformAccentColor(dark: Boolean): Int? = null
