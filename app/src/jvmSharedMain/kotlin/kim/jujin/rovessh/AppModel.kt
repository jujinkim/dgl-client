package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.LocalRepository
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

typealias ConnectionFactory = (HostProfile, CredentialStore, StateFlow<LocalState>, ((LocalState) -> LocalState) -> Unit) -> ClientConnection
class AppModel(val repository: LocalRepository, val credentials: CredentialStore, val storageDescription: String,
    val keys: KeyTools,
    private val agentInbox: AgentInbox = NoAgentInbox,
    private val connectionFactory: ConnectionFactory,
) {
    private val agentScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    @Volatile var foreground = false
    @Volatile var hostEditing = false
    @Volatile var visibleConnection: ClientConnection? = null
    @Volatile var sendAgentNotification: (AgentNotice) -> Unit = {}
    val agentNotifications = MutableStateFlow<List<AgentNotice>>(emptyList())
    val agentNavigationError = MutableStateFlow<String?>(null)
    val agentNavigation = MutableStateFlow(0L)
    val backgroundConnectionError = MutableStateFlow<String?>(null)
    val local = MutableStateFlow(repository.read())
    val connections = MutableStateFlow<List<ClientConnection>>(emptyList())
    val selected = MutableStateFlow<ClientConnection?>(null)
    private val composers = java.util.IdentityHashMap<ClientConnection, TextComposer>()
    fun composer(connection: ClientConnection): TextComposer = composers.getOrPut(connection) { TextComposer(connection, agentScope) }
    var cloudSync: CloudSyncController? = null
    var chooseSyncFile: (Boolean) -> Unit = {}
    var languageChanged: (String) -> Unit = {}
    init {
        agentScope.launch {
            while (isActive) {
                val visible = visibleConnection?.takeIf { foreground && it.status.value.phase == Phase.Ready }
                val keys = visible?.let { client ->
                    val remote = client.remote.value
                    val window = remote.windows.firstOrNull { it.id == (client.status.value.window ?: remote.panes.firstOrNull { p -> p.id == client.pane.value }?.window) }
                    remote.panes.filter { it.window == window?.id && (!window.zoomed || it.active) }
                        .map { "${client.agentEndpoint}:${client.agents.value.server}:${it.id}" }.toSet()
                } ?: emptySet()
                val result = synchronized(agentInbox) {
                    if (!isActive) return@launch
                    agentInbox.tick(connections.value.map { it.agentEndpoint to if (it.status.value.phase in setOf(Phase.Ready, Phase.Restoring, Phase.Selecting)) it.agents.value else AgentSnapshot() }, keys, local.value.preferences.agentNotifications)
                }
                agentNotifications.value = result.recent
                result.deliver.forEach { runCatching { sendAgentNotification(it) } }
                delay(250)
            }
        }
    }
    fun openAgentNotice(id: String) {
        val notice = agentNotifications.value.firstOrNull { it.id == id }
        agentInbox.markRead(id)
        val client = notice?.let { n -> connections.value.firstOrNull { c ->
            c.agentEndpoint == n.endpoint && c.status.value.phase == Phase.Ready && c.agents.value.current(System.currentTimeMillis()) &&
                c.agents.value.server == n.event.server && c.agents.value.observations.any { it.run == n.event.run && it.pane == n.event.pane }
        } }
        if (notice == null || client == null) { agentNavigationError.value = t("대상 연결 또는 패널이 종료되어 이동할 수 없습니다."); return }
        selected.value = client
        navigate(client, notice.event.session, notice.event.window, notice.event.pane)
        agentNavigation.value += 1
    }
    fun openAgentObservation(client: ClientConnection, observation: AgentObservation) {
        val snapshot = client.agents.value
        if (client !in connections.value || client.status.value.phase != Phase.Ready ||
            !snapshot.current(System.currentTimeMillis()) || snapshot.observations.none {
                it.run == observation.run && it.pane == observation.pane && it.session == observation.session && it.window == observation.window
            }) {
            agentNavigationError.value = t("대상 연결 또는 패널이 종료되어 이동할 수 없습니다.")
            return
        }
        selected.value = client
        navigate(client, observation.session, observation.window, observation.pane)
        agentNavigation.value += 1
    }
    private fun navigate(client: ClientConnection, session: String, window: String, pane: String) {
        if (client.session.value != session || client.pane.value != pane ||
            (client.status.value.window != null && client.status.value.window != window)) client.selectTarget(session, window, pane)
    }
    @Synchronized fun update(value: LocalState) {
        repository.save(value)
        languageChanged(value.preferences.language)
        local.value = value
    }
    @Synchronized fun modify(transform: (LocalState) -> LocalState) { update(transform(local.value)) }
    fun open(host: HostProfile) {
        val connection = connectionFactory(host, credentials, local, ::modify)
        connections.value += connection; selected.value = connection; connection.connect()
    }
    private fun isSelectedHost(client: ClientConnection, hostId: String) =
        selected.value === client && connections.value.any { it === client } && client.host.id == hostId
    fun reconnectSelected(client: ClientConnection, hostId: String) {
        if (isSelectedHost(client, hostId) && !client.status.value.canRetryNow) client.connect()
    }
    fun disconnectSelected(client: ClientConnection, hostId: String) {
        if (isSelectedHost(client, hostId)) client.disconnect()
    }
    fun removeConnection(connection: ClientConnection) {
        composers.remove(connection)?.close()
        connection.close(); connections.value -= connection
        if (selected.value == connection) selected.value = connections.value.lastOrNull()
    }
    fun disconnectAll() { connections.value.forEach { it.disconnect() } }
    fun close() {
        composers.values.forEach { it.close() }; composers.clear(); cloudSync?.close(); agentScope.cancel()
        connections.value.forEach { it.close() }; connections.value = emptyList(); selected.value = null; visibleConnection = null
        chooseSyncFile = {}; sendAgentNotification = {}; foreground = false
        synchronized(agentInbox) { agentInbox.close() }
    }
}
