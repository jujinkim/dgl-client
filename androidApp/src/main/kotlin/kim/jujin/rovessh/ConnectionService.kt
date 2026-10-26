package kim.jujin.rovessh

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.os.PowerManager
import kim.jujin.rovessh.core.t
import kotlinx.coroutines.*

/** Maintains user-started interactive SSH; never schedules or resurrects a connection. */
class ConnectionService : Service() {
    companion object {
        const val CHANNEL = "ssh-connections-v1"
        const val NOTIFICATION = 2001
        const val STOP = "kim.jujin.dglc.STOP_CONNECTIONS"
    }
    private var runtime: AndroidConnectionRuntime? = null
    private var expectedStop = false
    private var promoted = false
    private var wakeLock: PowerManager.WakeLock? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var wakeJob: Job? = null
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onCreate() {
        super.onCreate()
        runtime = (application as DglApplication).currentConnections()
        try {
            refreshNotification()
            if (runtime?.attach(this) != true) stopMaintaining()
        } catch (_: RuntimeException) {
            runtime?.serviceFailed(); stopMaintaining()
        }
    }
    fun refreshNotification() {
        if (expectedStop) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, t("SSH 연결 유지"), NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val stop = PendingIntent.getService(this, 1, Intent(this, ConnectionService::class.java).setAction(STOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val notification = Notification.Builder(this, CHANNEL).setSmallIcon(R.drawable.ic_terminal)
            .setContentTitle(t("SSH 연결 유지")).setContentText(t("홈 이동·화면 잠금 중에도 연결을 유지합니다."))
            .setOngoing(true).setOnlyAlertOnce(true).setContentIntent(open)
            .addAction(Notification.Action.Builder(null, t("앱 열기"), open).build())
            .addAction(Notification.Action.Builder(null, t("모든 연결 종료"), stop).build()).build()
        startForeground(NOTIFICATION, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        promoted = true
    }
    fun setAwake(value: Boolean) {
        if (value && !expectedStop) {
            if (wakeLock == null) wakeLock = getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "dgl:ssh-connections").apply { setReferenceCounted(false) }
            if (wakeJob?.isActive != true) {
                wakeLock?.acquire(10 * 60 * 1000L)
                wakeJob = scope.launch {
                    while (isActive) {
                        delay(5 * 60 * 1000L)
                        wakeLock?.acquire(10 * 60 * 1000L)
                    }
                }
            }
        } else {
            wakeJob?.cancel(); wakeJob = null
            if (wakeLock?.isHeld == true) wakeLock?.release()
        }
    }
    fun stopMaintaining() {
        expectedStop = true; setAwake(false)
        if (promoted) stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == STOP) { runtime?.app?.disconnectAll(); stopMaintaining() }
        else if (expectedStop) {
            expectedStop = false
            runtime = (application as DglApplication).currentConnections()
            try {
                refreshNotification()
                if (runtime?.attach(this) != true) stopMaintaining()
            } catch (_: RuntimeException) { runtime?.serviceFailed(); stopMaintaining() }
        }
        return START_NOT_STICKY
    }
    override fun onTaskRemoved(rootIntent: Intent?) {
        (application as DglApplication).removeTask(runtime)
        stopMaintaining()
        super.onTaskRemoved(rootIntent)
    }
    override fun onDestroy() {
        setAwake(false); runtime?.detach(this, expectedStop); runtime = null
        scope.cancel()
        super.onDestroy()
    }
}
