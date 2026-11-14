package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.storage.LocalRepository
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first

typealias ConnectionFactory = (HostProfile, CredentialStore, StateFlow<LocalState>, ((LocalState) -> LocalState) -> Unit) -> ClientConnection
class AppModel(val repository: LocalRepository, val credentials: CredentialStore, val storageDescription: String,
    val keys: KeyTools,
    private val agentInbox: AgentInbox = NoAgentInbox,
    private val connectionFactory: ConnectionFactory,
) {
    private val agentScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    @Volatile var foreground = false
        set(value) { synchronized(this) {
            field = value
            if (!value) clearShortcutModifiers()
            updateTitleVisibility()
            claimFocusedSizes()
        } }
    @Volatile var hostEditing = false
    @Volatile var visibleConnection: ClientConnection? = null
    @Volatile var visibleConnections: Set<ClientConnection> = emptySet()
        set(value) { synchronized(this) { field = value; updateTitleVisibility(); claimFocusedSizes() } }
    private fun updateTitleVisibility() {
        connections.value.forEach { it.setTitleUpdatesEnabled(foreground && it in visibleConnections) }
    }
    private val focusClaims = TerminalFocusClaims<ClientConnection>()
    /**
     * The tmux window follows the screen focused last: an app activation or a move to another
     * tab or group claims every shown terminal once, the selected one last. Called under this
     * model's lock whenever foreground, selection or the shown terminals change. Only the
     * layout's shown set counts: a narrow screen's last input connection may already be hidden.
     * Each connection's claim runs on its own native actor, so a shown terminal on the selected
     * one's server window does not claim: claim order alone would not let the selected one win.
     */
    private fun claimFocusedSizes() {
        val open = connections.value
        val shown = visibleConnections.filter { client -> open.any { it === client } }
        focusClaims.update(foreground, selected.value, shown, ::sameServerWindow).forEach(::claimFocusedSize)
    }
    private fun sameServerWindow(a: ClientConnection, b: ClientConnection) =
        TerminalFocusClaims.sameServerWindow(a.host, a.status.value.window, b.host, b.status.value.window)
    // Opening the app often reconnects: a focus claim refused until then is sent at the
    // connection's next Ready state, only if it is still shown, focused and in the foreground,
    // and not on the focused terminal's server window, which may be known only by now.
    private fun claimFocusedSize(client: ClientConnection) {
        client.claimSize(force = true) {
            synchronized(this@AppModel) {
                if (focusClaims.stillClaims(client, ::sameServerWindow) && connections.value.any { it === client }) claimFocusedSize(client)
            }
        }
    }
    @Volatile var workspaceEnabled = false
    @Volatile var sendAgentNotification: (AgentNotice) -> Unit = {}
    val agentNotifications = MutableStateFlow<List<AgentNotice>>(emptyList())
    val agentNavigationError = MutableStateFlow<String?>(null)
    val agentNavigation = MutableStateFlow(0L)
    val backgroundConnectionError = MutableStateFlow<String?>(null)
    val local = MutableStateFlow(repository.read())
    val connections = MutableStateFlow<List<ClientConnection>>(emptyList())
    val selected = MutableStateFlow<ClientConnection?>(null)
    val workspace = MutableStateFlow(repository.workspace.read())
    val inputBlocks = MutableStateFlow<Set<Any>>(emptySet())
    /** Hosts whose session tree the wide sidebar shows under their row. */
    val expandedHosts = MutableStateFlow<Set<String>>(emptySet())
    /** Tab-less session listings of expanded hosts that no tab lists, by host ID. */
    val hostProbes = MutableStateFlow<Map<String, HostProbe>>(emptyMap())
    internal var clock: () -> Long = System::currentTimeMillis
    private var probeExpiry: Job? = null
    private val tabClients = linkedMapOf<String, ClientConnection>()
    private val reservations = mutableMapOf<Pair<String, String>, ClientConnection>()
    /** UI state owned by one connection, created lazily and guarded by this model's lock. */
    private class ConnectionResources {
        var sessionJob: Job? = null
        var shortcutInput: ShortcutInput? = null
        var titles: SessionTitleCache? = null
        var composer: TextComposer? = null
        var font: TerminalFontSize? = null
        val empty get() = sessionJob == null && shortcutInput == null && titles == null && composer == null && font == null
    }
    private val resources = java.util.IdentityHashMap<ClientConnection, ConnectionResources>()
    private fun resources(client: ClientConnection) = resources.getOrPut(client) { ConnectionResources() }
    @Synchronized fun connectionTitles(client: ClientConnection) = resources(client).run { titles ?: SessionTitleCache().also { titles = it } }
    @Synchronized fun shortcutInput(client: ClientConnection) = resources(client).run { shortcutInput ?: ShortcutInput(client).also { shortcutInput = it } }
    @Synchronized fun clearShortcutModifiers() { resources.values.forEach { it.shortcutInput?.clear() } }
    @Synchronized fun blockInput(owner: Any, blocked: Boolean) {
        if (blocked) clearShortcutModifiers()
        inputBlocks.value = if (blocked) inputBlocks.value + owner else inputBlocks.value - owner
    }
    fun inputBlocked(except: Any? = null) = inputBlocks.value.any { it !== except }
    @Synchronized fun tabClient(id: String?) = tabClients[id]
    @Synchronized fun tabId(client: ClientConnection) = tabClients.entries.firstOrNull { it.value === client }?.key
    @Synchronized fun reconcileWorkspace() {
        resources.entries.removeAll { (client, owned) ->
            if (connections.value.none { it === client }) owned.titles = null
            owned.empty
        }
        tabClients.filterValues { it !in connections.value }.keys.toList().forEach { id ->
            workspace.value = workspace.value.close(id); tabClients.remove(id)
        }
        connections.value.forEach { client -> if (tabId(client) == null) {
            val id = java.util.UUID.randomUUID().toString()
            tabClients[id] = client; workspace.value = workspace.value.add(id)
        } }
        selected.value?.let { client -> tabId(client)?.let { workspace.value = workspace.value.select(it) } }
    }
    @Synchronized fun changeWorkspace(transform: (WorkspaceState) -> WorkspaceState) {
        val previous = workspace.value
        val next = transform(previous)
        if (previous.activeTab != next.activeTab || previous.activeGroup != next.activeGroup) clearShortcutModifiers()
        workspace.value = next
        selected.value = tabClient(next.activeTab)
        claimFocusedSizes()
        if (previous.root != next.root) runCatching { repository.workspace.save(next.root) }.onFailure {
            backgroundConnectionError.value = t("app.workspace_layout_save_failed")
        }
    }
    @Synchronized fun selectConnection(client: ClientConnection) {
        if (client !in connections.value) return
        if (selected.value !== client) clearShortcutModifiers()
        selected.value = client
        tabId(client)?.let { workspace.value = workspace.value.select(it) }
        claimFocusedSizes()
    }
    @Synchronized fun composer(connection: ClientConnection): TextComposer =
        resources(connection).run { composer ?: TextComposer(connection, agentScope).also { composer = it } }
    @Synchronized fun terminalFont(connection: ClientConnection): TerminalFontSize =
        resources(connection).run { font ?: TerminalFontSize(local.value.preferences.fontSize).also { font = it } }
    var cloudSync: CloudSyncController? = null
    var chooseSettingsFile: (SettingsFileAction) -> Unit = {}
    var languageChanged: (String) -> Unit = {}
    init {
        agentScope.launch {
            while (isActive) {
                val visible = (visibleConnections + listOfNotNull(visibleConnection)).filter { foreground && it.status.value.phase == Phase.Ready }
                val keys = visible.flatMap { client ->
                    val remote = client.remote.value
                    val window = remote.windows.firstOrNull { it.id == (client.status.value.window ?: remote.panes.firstOrNull { p -> p.id == client.pane.value }?.window) }
                    remote.panes.filter { it.window == window?.id && (!window.zoomed || it.active) }
                        .map { "${client.agentEndpoint}:${client.agents.value.server}:${it.id}" }
                }.toSet()
                val result = synchronized(agentInbox) {
                    if (!isActive) return@launch
                    agentInbox.tick(connections.value.map { it.agentEndpoint to it.observedAgents }, keys, local.value.preferences.agentNotifications)
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
        if (notice == null || client == null) { agentNavigationError.value = t("agent.target_closed_cannot_navigate"); return }
        navigateNotice(client, notice.event.session, notice.event.window, notice.event.pane)
        agentNavigation.value += 1
    }
    fun openAgentObservation(client: ClientConnection, observation: AgentObservation) {
        val snapshot = client.agents.value
        if (client !in connections.value || client.status.value.phase != Phase.Ready ||
            !snapshot.current(System.currentTimeMillis()) || snapshot.observations.none {
                it.run == observation.run && it.pane == observation.pane && it.session == observation.session && it.window == observation.window
            }) {
            agentNavigationError.value = t("agent.target_closed_cannot_navigate")
            return
        }
        navigateNotice(client, observation.session, observation.window, observation.pane)
        agentNavigation.value += 1
    }
    private fun navigate(client: ClientConnection, session: String, window: String, pane: String) {
        if (client.session.value != session || client.pane.value != pane ||
            (client.status.value.window != null && client.status.value.window != window)) client.selectTarget(session, window, pane)
    }
    private fun navigateNotice(client: ClientConnection, session: String, window: String, pane: String) {
        if (workspaceEnabled) openSession(client, session, window, pane)
        else { selectConnection(client); navigate(client, session, window, pane) }
    }
    @Synchronized fun update(value: LocalState) {
        repository.save(value)
        languageChanged(value.preferences.language)
        local.value = value
    }
    @Synchronized fun modify(transform: (LocalState) -> LocalState) { update(transform(local.value)) }
    @Synchronized fun open(host: HostProfile) {
        val existing = host.lastSession?.let { findSession(host.id, it) }
            ?: connections.value.firstOrNull { it.host.id == host.id && it.session.value == null }
        if (existing != null) { selectConnection(existing); return }
        createConnection(host)
    }
    private fun findSession(hostId: String, session: String): ClientConnection? =
        connections.value.firstOrNull { it.host.id == hostId && it.session.value == session }
            ?: reservations[hostId to session]?.takeIf { it in connections.value && (it.session.value == null || it.session.value == session) }

    /** Reserve the host/session before connect() can start asynchronous work. */
    private fun createConnection(host: HostProfile): ClientConnection {
        val connection = connectionFactory(host, credentials, local, ::modify)
        host.lastSession?.let { reservations[host.id to it] = connection }
        val id = java.util.UUID.randomUUID().toString()
        tabClients[id] = connection
        connections.value += connection
        expandedHosts.value += host.id
        workspace.value = workspace.value.add(id)
        clearShortcutModifiers()
        selected.value = connection
        claimFocusedSizes()
        resources(connection).sessionJob = agentScope.launch {
            connection.session.collect { session -> synchronized(this@AppModel) {
                if (session != null && connection in connections.value) {
                    reservations.entries.removeAll { it.value === connection }
                    reservations[connection.host.id to session] = connection
                }
            } }
        }
        connection.connect()
        return connection
    }
    @Synchronized fun openSession(source: ClientConnection, session: String, window: String? = null, pane: String? = null,
        expectedGeneration: Long = source.generation): ClientConnection? {
        val probe = hostProbes.value.values.firstOrNull { it.client === source && it.live }
        if ((source !in connections.value && probe == null) || source.generation != expectedGeneration ||
            source.status.value.phase !in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)) return null
        if (probe != null) return openHostSession(probe.host, session, window, pane)
        val existing = findSession(source.host.id, session)
        if (existing != null) {
            selectConnection(existing)
            if (window != null || pane != null) selectWhenReady(existing, session, window, pane)
            return existing
        }
        // A selection connection has no remote session yet and can become this session's tab.
        val client = if (source.session.value == null && reservations.values.none { it === source }) {
            reservations[source.host.id to session] = source
            selectConnection(source)
            source.selectTarget(session, window, pane)
            source
        } else createConnection(source.host.copy(lastSession = session, lastPane = null))
        if (client !== source && (window != null || pane != null)) selectWhenReady(client, session, window, pane)
        return client
    }
    /** A session created from [source] opens like a click on its row. */
    fun createSession(source: ClientConnection, name: String) {
        touchProbe(source.host.id)
        val generation = source.generation
        source.createSession(name) { id ->
            if (source.generation != generation) return@createSession
            if (workspaceEnabled) openSession(source, id, expectedGeneration = generation)
            else source.selectTarget(id)
        }
    }
    private fun selectWhenReady(client: ClientConnection, session: String, window: String?, pane: String?) {
        val generation = client.generation
        if (client.status.value.phase == Phase.Ready && client.session.value == session) {
            client.selectTarget(session, window, pane); return
        }
        agentScope.launch {
            combine(client.status, client.session) { status, current -> status.phase to current }.first {
                client.generation != generation || client !in connections.value ||
                    it.first in setOf(Phase.Ready, Phase.Failed, Phase.Disconnected, Phase.MissingSession)
            }
            synchronized(this@AppModel) {
                if (client in connections.value && client.generation == generation && client.status.value.phase == Phase.Ready && client.session.value == session)
                    client.selectTarget(session, window, pane)
            }
        }
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
        val composer = synchronized(this) {
            val owned = resources.remove(connection)
            owned?.sessionJob?.cancel()
            reservations.entries.removeAll { it.value === connection }
            val id = tabId(connection)
            if (id != null) { workspace.value = workspace.value.close(id); tabClients.remove(id) }
            connections.value -= connection
            if (selected.value == connection) selected.value = tabClient(workspace.value.activeTab)
            claimFocusedSizes()
            owned?.shortcutInput?.clear()
            owned?.composer
        }
        // Native state polling can call modify(); never close a native owner under this lock.
        composer?.close()
        connection.close()
    }
    fun disconnectAll() { connections.value.forEach { it.disconnect() } }

    /**
     * Opens [session] of [host] from a probe's or a cached listing: activates its tab, binds the
     * host's tab still selecting a session, or starts a new tab.
     */
    @Synchronized fun openHostSession(host: HostProfile, session: String, window: String? = null, pane: String? = null): ClientConnection {
        touchProbe(host.id)
        findSession(host.id, session)?.let { existing ->
            selectConnection(existing)
            if (window != null || pane != null) selectWhenReady(existing, session, window, pane)
            return existing
        }
        val selecting = connections.value.firstOrNull { c ->
            c.host.id == host.id && c.session.value == null && reservations.values.none { it === c } &&
                c.status.value.phase in setOf(Phase.Ready, Phase.Selecting, Phase.MissingSession)
        }
        if (selecting != null) {
            reservations[host.id to session] = selecting
            selectConnection(selecting)
            selecting.selectTarget(session, window, pane)
            return selecting
        }
        val saved = local.value.hosts.firstOrNull { it.id == host.id } ?: host
        val client = createConnection(saved.copy(lastSession = session, lastPane = null))
        if (window != null || pane != null) selectWhenReady(client, session, window, pane)
        return client
    }
    /** Expanding a host that has no tab starts its probe; a host's tab lists it otherwise. */
    fun expandHost(host: HostProfile) {
        val start = synchronized(this) {
            expandedHosts.value += host.id
            connections.value.none { it.host.id == host.id } && hostProbes.value[host.id]?.live != true
        }
        if (start) startProbe(host)
    }
    fun collapseHost(hostId: String) {
        val probe = synchronized(this) {
            expandedHosts.value -= hostId
            hostProbes.value[hostId]?.also { hostProbes.value -= hostId }
        }
        probe?.client?.close()
    }
    /** Refreshes the connection listing [host]; a closed probe connects again. */
    fun refreshHost(host: HostProfile, source: ClientConnection?) {
        val probe = hostProbes.value[host.id]
        when {
            source != null && source in connections.value -> source.refresh()
            probe != null && probe.live -> { touchProbe(host.id); probe.client.refresh() }
            else -> startProbe(host)
        }
    }
    fun touchProbe(hostId: String) {
        synchronized(this) {
            hostProbes.value[hostId]?.let { hostProbes.value += hostId to it.copy(used = clock()) }
        }
    }
    private fun startProbe(host: HostProfile) {
        // A probe never attaches: without a remembered target it stops at session selection.
        val client = connectionFactory(host.copy(lastSession = null, lastPane = null), credentials, local) { }
        val (started, replaced) = synchronized(this) {
            if (host.id !in expandedHosts.value) return@synchronized false to null
            val previous = hostProbes.value[host.id]
            hostProbes.value += host.id to HostProbe(host, client, live = true, cached = RemoteState(), used = clock())
            if (probeExpiry?.isActive != true) probeExpiry = agentScope.launch {
                while (isActive && hostProbes.value.values.any { it.live }) { delay(1_000); expireProbes() }
            }
            true to previous
        }
        replaced?.client?.close()
        if (started) client.connect() else client.close()
    }
    /**
     * Closes probes that lost their purpose: a collapsed or deleted host, a host a tab now lists,
     * the app in the background, or [HostProbe.IDLE_MS] without use. An idle or background probe
     * keeps its last rows read-only; refreshing lists them again.
     */
    internal fun expireProbes(now: Long = clock()) {
        val closing = synchronized(this) {
            val closing = mutableListOf<ClientConnection>()
            hostProbes.value = hostProbes.value.mapNotNull { (id, probe) ->
                val gone = id !in expandedHosts.value || local.value.hosts.none { it.id == id } ||
                    connections.value.any { it.host.id == id }
                when {
                    gone -> { closing += probe.client; null }
                    probe.live && (!foreground || now - probe.used >= HostProbe.IDLE_MS) -> {
                        closing += probe.client
                        id to probe.copy(live = false, cached = probe.client.remote.value)
                    }
                    else -> id to probe
                }
            }.toMap()
            closing
        }
        // Native state polling can call modify(); never close a native owner under this lock.
        closing.forEach { it.close() }
    }
    fun close() {
        var probes = emptyList<ClientConnection>()
        val (drafts, clients) = synchronized(this) {
            agentScope.cancel()
            val owned = resources.values.mapNotNull { it.composer } to connections.value
            resources.clear(); connections.value = emptyList(); selected.value = null; visibleConnection = null; visibleConnections = emptySet()
            tabClients.clear(); reservations.clear(); inputBlocks.value = emptySet()
            probes = hostProbes.value.values.map { it.client }; hostProbes.value = emptyMap()
            chooseSettingsFile = {}; sendAgentNotification = {}; foreground = false
            owned
        }
        drafts.forEach { it.close() }; cloudSync?.close(); clients.forEach { it.close() }; probes.forEach { it.close() }
        synchronized(agentInbox) { agentInbox.close() }
    }
}

/** A tab-less connection listing one host's sessions. Once closed ([live] false), [cached] keeps its last rows. */
data class HostProbe(val host: HostProfile, val client: ClientConnection, val live: Boolean, val cached: RemoteState, val used: Long) {
    val remote: RemoteState get() = if (live) client.remote.value else cached
    companion object { const val IDLE_MS = 60_000L }
}

/** Where a host's session tree comes from: its selected tab, its most recent tab, then its probe. */
data class HostTreeSource(val client: ClientConnection, val probe: HostProbe?) {
    val live: Boolean get() = probe?.live ?: true
}
fun hostTreeSource(hostId: String, connections: List<ClientConnection>, selected: ClientConnection?, probes: Map<String, HostProbe>): HostTreeSource? {
    val tab = selected?.takeIf { it.host.id == hostId && it in connections } ?: connections.lastOrNull { it.host.id == hostId }
    if (tab != null) return HostTreeSource(tab, null)
    return probes[hostId]?.let { HostTreeSource(it.client, it) }
}
