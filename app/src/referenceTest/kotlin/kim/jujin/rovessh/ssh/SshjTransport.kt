package kim.jujin.rovessh.ssh

import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.common.Buffer
import net.schmizz.sshj.connection.channel.direct.Session
import net.schmizz.sshj.userauth.UserAuthException
import net.schmizz.sshj.userauth.password.PasswordUtils
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.security.MessageDigest
import java.security.Security
import org.bouncycastle.jce.provider.BouncyCastleProvider
import net.schmizz.sshj.common.SecurityUtils
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicBoolean

class SshjTransport : SshTransport {
    init { SshCrypto.initialize() }
    @Volatile private var client: SSHClient? = null
    private val channels = CopyOnWriteArrayList<Session>()
    private val tmuxDiscovery = Mutex()
    @Volatile private var tmuxPath: String? = null
    @Volatile private var discoveryDetails = TmuxDiagnostics()
    override val tmuxDiagnostics get() = discoveryDetails

    override suspend fun connect(host: HostProfile, credential: Credential, trustedFingerprint: String?) = withContext(Dispatchers.IO) {
        close()
        val ssh = SSHClient()
        client = ssh
        ssh.connectTimeout = 12_000
        ssh.timeout = 15_000
        var keyFailure: Exception? = null
        ssh.addHostKeyVerifier(object : net.schmizz.sshj.transport.verification.HostKeyVerifier {
          override fun findExistingAlgorithms(hostname: String, port: Int) = emptyList<String>()
          override fun verify(hostname: String, port: Int, key: java.security.PublicKey): Boolean {
            val blob = Buffer.PlainBuffer().putPublicKey(key).compactData
            val fingerprint = "SHA256:" + Base64.getEncoder().withoutPadding().encodeToString(MessageDigest.getInstance("SHA-256").digest(blob))
            return when {
                trustedFingerprint == null -> { keyFailure = UnknownHostKey(host.endpoint, fingerprint); false }
                trustedFingerprint != fingerprint -> { keyFailure = ChangedHostKey(); false }
                else -> true
            }
          }
        })
        try {
            ssh.connect(host.hostname, host.port)
            when (host.auth) {
                AuthKind.Password -> ssh.authPassword(host.username, credential.password)
                AuthKind.PrivateKey -> {
                    require(credential.privateKey.length in 32..131072) { "Invalid private key" }
                    val key = try {
                        ssh.loadKeys(credential.privateKey, null, PasswordUtils.createOneOff(credential.passphrase.toCharArray()))
                            .also { it.private }
                    } catch (_: Exception) { throw ClientFailure(Failure.Authentication, "개인키 또는 암호를 확인하세요.") }
                    ssh.authPublickey(host.username, key)
                }
            }
            ssh.connection.keepAlive.keepAliveInterval = 10
        } catch (e: Exception) {
            close()
            keyFailure?.let { throw it }
            if (e is CancellationException) throw e
            if (e is ClientFailure) throw e
            if (e is UserAuthException) throw ClientFailure(Failure.Authentication, "인증 실패: 사용자 이름과 자격 증명을 확인하세요.")
            throw ClientFailure(Failure.Network, "SSH 연결에 실패했습니다.", generateSequence<Throwable>(e) { it.cause }.take(5).joinToString("/") { it.javaClass.simpleName })
        }
    }

    override suspend fun execute(command: String): ByteArray = withContext(Dispatchers.IO) {
        val owner = connected()
        val session = owner.startSession()
        channels += session
        try {
            withTimeout(15_000) {
                val process = session.exec(command)
                coroutineScope {
                    val errors = async(Dispatchers.IO) { runInterruptible { boundedRead(process.errorStream, 65536) } }
                    val output = runInterruptible { boundedRead(process.inputStream, 4 shl 20) }
                    errors.await()
                    process.join(5, java.util.concurrent.TimeUnit.SECONDS)
                    if (process.exitStatus != 0) throw RemoteCommandException(process.exitStatus)
                    output
                }
            }
        } finally { channels -= session; session.close() }
    }

    private suspend fun tmuxExecutable(): String = tmuxDiscovery.withLock {
        val owner = connected()
        tmuxPath ?: run {
            val result = try { TmuxDiscovery.find(execute = ::execute) } catch (e: TmuxDiscoveryException) {
                if (client === owner) discoveryDetails = e.diagnostics
                throw e
            }
            if (connected() !== owner) throw ClientFailure(Failure.Network, "SSH 연결이 변경되었습니다.")
            discoveryDetails = result
            requireNotNull(result.executable).also { tmuxPath = it }
        }
    }

    override suspend fun tmuxVersion() = execute(TmuxCommand.quote(tmuxExecutable()) + " -V").decodeToString()

    override suspend fun executeTmux(command: TmuxCommand) = execute(command.shellWithExecutable(tmuxExecutable()))

    override suspend fun openControl(sessionId: String): SshTransport.Stream = withContext(Dispatchers.IO) {
        require(sessionId.matches(Regex("\\$[0-9]+")))
        val executable = tmuxExecutable()
        val owner = connected()
        val session = owner.startSession()
        channels += session
        try {
            // No PTY. Never -d: attaching must not detach other clients.
            val command = session.exec(TmuxCommand.quote(executable) + " -u -C attach-session -t " + TmuxCommand.quote(sessionId))
            SshStream(owner, session, command.inputStream, command.outputStream, command.errorStream, null)
        } catch (e: Exception) { channels -= session; session.close(); throw e }
    }

    override suspend fun openShell(size: Size): SshTransport.Stream = withContext(Dispatchers.IO) {
        val owner = connected()
        val session = owner.startSession()
        channels += session
        try {
            session.allocatePTY("xterm-256color", size.columns, size.rows, 0, 0, emptyMap())
            val shell = session.startShell()
            SshStream(owner, session, shell.inputStream, shell.outputStream, shell.errorStream, shell)
        } catch (e: Exception) { channels -= session; session.close(); throw e }
    }

    private fun connected() = client?.takeIf { it.isAuthenticated } ?: throw ClientFailure(Failure.Network, "SSH 연결이 없습니다.")

    override fun close() {
        tmuxPath = null
        discoveryDetails = TmuxDiagnostics()
        val ssh = client; client = null
        runCatching { ssh?.close() }
        channels.forEach { runCatching { it.close() } }; channels.clear()
    }

    private inner class SshStream(
        val owner: SSHClient, val session: Session, val input: InputStream, val output: OutputStream, errors: InputStream, val shell: Session.Shell?,
    ) : SshTransport.Stream() {
        private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        private val closed = AtomicBoolean()
        init { scope.launch { runCatching { boundedRead(errors, 65536) }; } }
        override suspend fun read(): ByteArray? = runInterruptible(Dispatchers.IO) {
            // SSHJ may retain a receive window after close. Do not make a slow
            // renderer drain that obsolete output before detecting the disconnect.
            if (closed.get() || client !== owner || !owner.isConnected) return@runInterruptible null
            val bytes = ByteArray(16384)
            val n = input.read(bytes)
            if (n < 0 || closed.get() || client !== owner || !owner.isConnected) null else bytes.copyOf(n)
        }
        override suspend fun write(bytes: ByteArray) = runInterruptible(Dispatchers.IO) { output.write(bytes); output.flush() }
        override suspend fun resize(size: Size) = withContext(Dispatchers.IO) {
            shell?.changeWindowDimensions(size.columns, size.rows, 0, 0)
            Unit
        }
        override fun close() {
            if (!closed.compareAndSet(false, true)) return
            // EOF tells the control client to exit. Drain its bounded pending
            // output without rendering it, so a full receive window cannot hold
            // the peer's shutdown behind terminal backpressure.
            val shutdown = scope.async {
                runCatching {
                    coroutineScope {
                        val discard = launch { runInterruptible { val bytes = ByteArray(16384); while (input.read(bytes) >= 0) {} } }
                        // A PTY shell does not interpret SSH stdin EOF as terminal
                        // hangup; closing its channel supplies that signal instead.
                        if (shell == null) {
                            runInterruptible { output.close() }
                            discard.join()
                        }
                        runInterruptible { session.close() }
                    }
                }.isSuccess
            }
            try {
                // Sending CHANNEL_CLOSE before the EOF response has drained can
                // strand tmux's output queue. Allow graceful EOF first, but never
                // block a caller through SSHJ's full network timeout on shutdown.
                val graceful = runCatching { runBlocking { withTimeoutOrNull(1500) { shutdown.await() } } == true }.getOrDefault(false)
                if (!graceful) runCatching { owner.close() }
            } finally { channels -= session; scope.cancel() }
        }
    }
}

internal object SshCrypto {
    private var initialized = false
    @Synchronized fun initialize() {
        if (initialized) return
        // Android's reduced built-in provider has the same "BC" name but lacks SSH algorithms.
        // Provider registration is local to this app process; AndroidKeyStore is untouched.
        if (Security.getProvider("BC") !is BouncyCastleProvider) {
            Security.removeProvider("BC")
            check(Security.addProvider(BouncyCastleProvider()) >= 0)
        }
        SecurityUtils.setSecurityProvider("BC")
        initialized = true
    }
}

class RemoteCommandException(val exitStatus: Int? = null) : Exception("원격 명령을 완료하지 못했습니다. 대상을 새로고침하세요.")

private fun boundedRead(input: InputStream, limit: Int): ByteArray {
    val result = ByteArrayOutputStream()
    val buffer = ByteArray(8192)
    while (true) {
        val n = input.read(buffer)
        if (n < 0) break
        if (result.size() + n > limit) throw ProtocolException("SSH response limit exceeded")
        result.write(buffer, 0, n)
    }
    return result.toByteArray()
}
