package kim.jujin.rovessh

import android.app.Application
import android.content.Intent
import kim.jujin.rovessh.core.t
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*

/** Process-owned engine: Activity recreation and notification intents reuse SSH. */
class DglApplication : Application() {
    private var runtime: AndroidConnectionRuntime? = null
    fun connections(): AndroidConnectionRuntime = runtime ?: AndroidConnectionRuntime(this).also { runtime = it }
    fun currentConnections(): AndroidConnectionRuntime? = runtime
    fun removeTask(expected: AndroidConnectionRuntime? = runtime) {
        if (runtime !== expected) return
        runtime = null; expected?.removeTask()
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class AndroidConnectionRuntime(private val application: DglApplication) {
    val app = createEngine(application)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val visible = MutableStateFlow(false)
    private var wakeWanted = false
    private var service: ConnectionService? = null
    private val lifetime = ConnectionLifetime(
        startService = {
            try {
                application.startForegroundService(Intent(application, ConnectionService::class.java))
                app.backgroundConnectionError.value = null
                true
            } catch (_: RuntimeException) { maintenanceFailed(); false }
        },
        stopService = {
            service?.stopMaintaining()
            application.stopService(Intent(application, ConnectionService::class.java))
            app.backgroundConnectionError.value = null
        },
        wake = { wanted -> wakeWanted = wanted; service?.setAwake(wanted) },
        disconnect = app::disconnectAll,
        dispose = { app.close(); scope.cancel() },
    )
    init {
        val notifications = AgentNotifications(application)
        val configure = app.languageChanged
        app.languageChanged = { preference ->
            val previous = kim.jujin.rovessh.core.I18n.language
            configure(preference)
            if (previous != kim.jujin.rovessh.core.I18n.language) {
                notifications.refreshLanguage()
                try { service?.refreshNotification() } catch (_: RuntimeException) { serviceFailed(); service?.stopMaintaining() }
            }
        }
        app.sendAgentNotification = notifications::show
        scope.launch {
            val phases = app.connections.flatMapLatest { clients ->
                if (clients.isEmpty()) flowOf(emptyList()) else combine(clients.map { it.status }) { states -> states.map { it.phase } }
            }
            combine(app.local, visible, phases) { local, foreground, states -> Triple(local.preferences.keepConnectionsInBackground, foreground, states) }
                .collect { (keep, foreground, states) -> lifetime.update(keep, foreground, states) }
        }
    }
    fun setVisible(value: Boolean) { visible.value = value }
    fun attach(service: ConnectionService): Boolean {
        if (!lifetime.serviceRequested) return false
        this.service = service; service.setAwake(wakeWanted)
        return true
    }
    fun detach(service: ConnectionService, expected: Boolean) {
        if (this.service !== service) return
        this.service = null
        if (!expected && lifetime.serviceRequested) { lifetime.serviceFailed(); maintenanceFailed() }
    }
    fun maintenanceFailed() {
        app.backgroundConnectionError.value = t("백그라운드 연결 유지를 시작하지 못했습니다. 앱을 열어 두고 배터리 설정을 확인하세요.")
    }
    fun serviceFailed() { lifetime.serviceFailed(); maintenanceFailed() }
    fun removeTask() { lifetime.taskRemoved() }
}
