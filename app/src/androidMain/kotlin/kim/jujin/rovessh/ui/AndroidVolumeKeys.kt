package kim.jujin.rovessh.ui

import android.content.Context
import android.content.ContextWrapper
import android.view.KeyEvent
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import kim.jujin.rovessh.ClientConnection
import kim.jujin.rovessh.core.Preferences

interface VolumeKeyHost : LifecycleOwner { val volumeKeys: VolumeKeyDispatcher }

/** Activity-owned: never holds a target after its composition is disposed. Main thread only. */
class VolumeKeyDispatcher {
    private var owner: Any? = null
    private var execute: ((Boolean) -> Boolean)? = null
    private data class Press(val downTime: Long, val consumed: Boolean)
    private val presses = mutableMapOf<Pair<Int, Int>, Press>()
    var resumed = false
        set(value) { field = value; if (!value) presses.clear() }
    var focused = false
        set(value) { field = value; if (!value) presses.clear() }

    fun register(token: Any, action: (Boolean) -> Boolean) { owner = token; execute = action }
    fun unregister(token: Any) {
        if (owner === token) { owner = null; execute = null }
        // Keep the gesture decision until key-up; it carries no connection reference.
        // Switching screens while held must not send input or change the volume mid-press.
    }
    fun clear() { owner = null; execute = null; presses.clear() }

    fun dispatch(event: KeyEvent): Boolean {
        if (event.keyCode != KeyEvent.KEYCODE_VOLUME_UP && event.keyCode != KeyEvent.KEYCODE_VOLUME_DOWN) return false
        if (!resumed || !focused) return false
        val id = event.deviceId to event.keyCode
        val press = presses[id]?.takeIf { it.downTime == event.downTime }
        return when (event.action) {
            KeyEvent.ACTION_DOWN -> {
                if (press != null) return press.consumed
                // An orphan repeat (e.g. after rotation/focus loss) is never a new shortcut.
                if (event.repeatCount != 0 || event.isCanceled) return false
                val consumed = execute?.invoke(event.keyCode == KeyEvent.KEYCODE_VOLUME_UP) == true
                presses[id] = Press(event.downTime, consumed)
                consumed
            }
            KeyEvent.ACTION_UP -> { presses.remove(id); press?.consumed == true }
            else -> false
        }
    }
}

private tailrec fun Context.volumeKeyHost(): VolumeKeyHost? = when (this) {
    is VolumeKeyHost -> this
    is ContextWrapper -> baseContext.volumeKeyHost()
    else -> null
}

@Composable actual fun PlatformInputRegistration(client: ClientConnection, preferences: Preferences, allowed: () -> Boolean) {
    val host = LocalContext.current.volumeKeyHost() ?: return
    val currentAllowed by rememberUpdatedState(allowed)
    val currentPreferences by rememberUpdatedState(preferences)
    // Encode on settings changes, never on each key repeat. Invalid external settings fail closed.
    val up = remember(preferences.volumeUp) { runCatching { preferences.volumeUp.bytes() }.getOrNull() }
    val down = remember(preferences.volumeDown) { runCatching { preferences.volumeDown.bytes() }.getOrNull() }
    val currentUp by rememberUpdatedState(up)
    val currentDown by rememberUpdatedState(down)
    DisposableEffect(host, client) {
        val token = Any()
        val execute: (Boolean) -> Boolean = { volumeUp ->
            val bytes = if (volumeUp) currentUp else currentDown
            if (currentPreferences.volumeKeysEnabled && bytes != null && currentAllowed() && client.status.value.acceptsInput) {
                client.input(bytes)
                true
            } else false
        }
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> host.volumeKeys.register(token, execute)
                Lifecycle.Event.ON_PAUSE, Lifecycle.Event.ON_DESTROY -> host.volumeKeys.unregister(token)
                else -> Unit
            }
        }
        host.lifecycle.addObserver(observer)
        onDispose { host.lifecycle.removeObserver(observer); host.volumeKeys.unregister(token) }
    }
}
