package kim.jujin.dglc.ui

import android.content.*
import android.content.res.Configuration
import androidx.activity.ComponentActivity
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver

@Composable actual fun PlatformAccentColor(dark: Boolean): Int? {
    val context = LocalContext.current
    val configuration = LocalConfiguration.current
    var revision by remember { mutableIntStateOf(0) }
    DisposableEffect(context) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context?, intent: Intent?) { revision++ }
        }
        val callbacks = object : ComponentCallbacks {
            override fun onConfigurationChanged(config: Configuration) { revision++ }
            override fun onLowMemory() {}
        }
        val lifecycle = (context as? ComponentActivity)?.lifecycle
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_RESUME) revision++ }
        lifecycle?.addObserver(observer)
        context.registerComponentCallbacks(callbacks)
        context.registerReceiver(receiver, IntentFilter(Intent.ACTION_WALLPAPER_CHANGED), Context.RECEIVER_NOT_EXPORTED)
        onDispose {
            lifecycle?.removeObserver(observer)
            context.unregisterComponentCallbacks(callbacks)
            context.unregisterReceiver(receiver)
        }
    }
    return remember(context, configuration, dark, revision) {
        runCatching {
            (if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)).primary.toArgb() and 0xffffff
        }.getOrNull()
    }
}
