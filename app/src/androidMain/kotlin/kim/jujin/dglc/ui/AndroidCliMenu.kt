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

private class MenuKeys(private val previous: Window.Callback, val dismiss: () -> Unit) : Window.Callback by previous {
    fun consume(event: KeyEvent): Boolean {
        if (event.keyCode != KeyEvent.KEYCODE_ESCAPE && event.keyCode != KeyEvent.KEYCODE_BACK) return false
        if (event.action == KeyEvent.ACTION_UP && !event.isCanceled) dismiss()
        return true
    }
    override fun dispatchKeyEvent(event: KeyEvent) = consume(event) || previous.dispatchKeyEvent(event)
}

/** A hardware Escape can reach the keyboard before Window.Callback gets it. */
internal fun Context.consumeMenuKeyBeforeIme(event: KeyEvent) =
    (menuActivity()?.window?.callback as? MenuKeys)?.consume(event) == true

@Composable internal actual fun CliMenuDismissHandler(dismiss: () -> Unit) {
    val activity = LocalContext.current.menuActivity() ?: return
    val currentDismiss by rememberUpdatedState(dismiss)
    DisposableEffect(activity) {
        // OVERLAY runs before the IME's Back callback, including gesture navigation.
        val back = OnBackInvokedCallback { currentDismiss() }
        activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_OVERLAY, back)
        val window = activity.window
        val previous = window.callback
        val callback = MenuKeys(previous) { currentDismiss() }
        window.callback = callback
        onDispose {
            activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(back)
            if (window.callback === callback) window.callback = previous
        }
    }
}
