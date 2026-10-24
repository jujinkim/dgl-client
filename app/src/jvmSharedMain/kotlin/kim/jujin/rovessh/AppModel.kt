package kim.jujin.rovessh

import kim.jujin.rovessh.core.*
import kim.jujin.rovessh.storage.LocalRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

typealias ConnectionFactory = (HostProfile, CredentialStore, StateFlow<LocalState>, ((LocalState) -> LocalState) -> Unit) -> ClientConnection
class AppModel(val repository: LocalRepository, val credentials: CredentialStore, val storageDescription: String,
    val keys: KeyTools,
    private val connectionFactory: ConnectionFactory,
) {
    val local = MutableStateFlow(repository.read())
    val connections = MutableStateFlow<List<ClientConnection>>(emptyList())
    val selected = MutableStateFlow<ClientConnection?>(null)
    @Synchronized fun update(value: LocalState) { repository.save(value); local.value = value }
    @Synchronized fun modify(transform: (LocalState) -> LocalState) { update(transform(local.value)) }
    fun open(host: HostProfile) {
        val connection = connectionFactory(host, credentials, local, ::modify)
        connections.value += connection; selected.value = connection; connection.connect()
    }
    fun removeConnection(connection: ClientConnection) {
        connection.close(); connections.value -= connection
        if (selected.value == connection) selected.value = connections.value.lastOrNull()
    }
    fun close() { connections.value.forEach { it.close() } }
}
