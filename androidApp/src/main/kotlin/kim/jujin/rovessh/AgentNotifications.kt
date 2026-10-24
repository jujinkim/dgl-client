package kim.jujin.rovessh

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import kim.jujin.rovessh.core.AgentNotice
import kim.jujin.rovessh.core.agentMessage

/** No service, wake lock or push. The ViewModel's existing connections own collection. */
class AgentNotifications(private val context: Context) {
    companion object {
        const val CHANNEL = "agent-observations-v1"
        const val NOTICE = "dgl.agent.notice"
    }
    private val manager = context.getSystemService(NotificationManager::class.java)
    init {
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "에이전트 사건", NotificationManager.IMPORTANCE_DEFAULT).apply {
            description = "질문·승인 요청·응답 종료 신호·명시적 오류"
        })
    }
    fun show(notice: AgentNotice) {
        if (context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return
        if (!manager.areNotificationsEnabled()) return
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            data = Uri.Builder().scheme("dgl-agent").authority("event").appendPath(notice.id).build()
            putExtra(NOTICE, notice.id)
        }
        val pending = PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_terminal)
            .setContentTitle("${notice.event.agent} · ${notice.event.pane}" + if (notice.event.actor != "parent") " · 서브에이전트" else "")
            .setContentText(agentMessage(notice.event.kind)).setContentIntent(pending).setAutoCancel(true)
            .setOnlyAlertOnce(true).build()
        manager.notify(notice.key, 1, notification)
    }
}
