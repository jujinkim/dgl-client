package kim.jujin.dglc.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.KeyEvent
import android.view.Window
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext

private tailrec fun Context.menuActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.menuActivity()
    else -> null
}

@Composable internal actual fun CliMenuDismissHandler(dismiss: () -> Unit) {
    val activity = LocalContext.current.menuActivity() ?: return
    val currentDismiss by rememberUpdatedState(dismiss)
    DisposableEffect(activity) {
        // OVERLAY runs before the IME's Back callback, including gesture navigation.
        val back = OnBackInvokedCallback { currentDismiss() }
        activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_OVERLAY, back)
        val window = activity.window
        val previous = window.callback
        val callback = object : Window.Callback by previous {
            override fun dispatchKeyEvent(event: KeyEvent): Boolean {
                if (event.keyCode != KeyEvent.KEYCODE_ESCAPE && event.keyCode != KeyEvent.KEYCODE_BACK)
                    return previous.dispatchKeyEvent(event)
                // Keep both halves of the key out of the WebView, then remove the handler.
                if (event.action == KeyEvent.ACTION_UP && !event.isCanceled) currentDismiss()
                return true
            }
        }
        window.callback = callback
        onDispose {
            activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(back)
            if (window.callback === callback) window.callback = previous
        }
    }
}
