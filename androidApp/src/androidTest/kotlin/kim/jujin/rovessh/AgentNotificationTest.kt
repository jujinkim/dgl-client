package kim.jujin.rovessh

import android.Manifest
import android.app.NotificationManager
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.core.app.ActivityScenario
import androidx.lifecycle.ViewModelProvider
import kim.jujin.dgl.RustAgentInbox
import kim.jujin.rovessh.core.*
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AgentNotificationTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    private fun event(id: String) = AgentEvent(id, 1, "server", "run", "parent", "$0", "@0", "%0", "Claude", "Approval", "PermissionRequest", System.currentTimeMillis())
    @Test fun nativeInboxDeduplicatesVisibleReadAndDoesNotResolveObservation() {
        val inbox = RustAgentInbox()
        try {
            val e = event("duplicate")
            val snapshot = AgentSnapshot(server = "server", valid = true, observed_at = System.currentTimeMillis(), events = listOf(e))
            val sources = listOf("host" to snapshot, "host" to snapshot)
            val result = inbox.tick(sources, setOf("host:server:%0"), AgentNotificationSettings(os = true))
            assertEquals(1, result.recent.size)
            assertTrue(result.recent.single().read)
            assertEquals("Approval", result.recent.single().event.kind)
            assertTrue(result.deliver.isEmpty())
            assertEquals("Approval", snapshot.events.single().kind)
        } finally { inbox.close() }
    }
    @Test fun channelPermissionAndClickIntentRetainOnlyEventId() {
        val context = instrumentation.targetContext
        val manager = context.getSystemService(NotificationManager::class.java)
        val notifications = AgentNotifications(context)
        val notice = AgentNotice("host", event("permission-test"))
        assertNotNull(manager.getNotificationChannel(AgentNotifications.CHANNEL))
        manager.cancelAll()
        instrumentation.uiAutomation.revokeRuntimePermission(context.packageName, Manifest.permission.POST_NOTIFICATIONS)
        notifications.show(notice)
        assertTrue(manager.activeNotifications.isEmpty())
        instrumentation.uiAutomation.grantRuntimePermission(context.packageName, Manifest.permission.POST_NOTIFICATIONS)
        try {
            notifications.show(notice)
            val deadline = System.currentTimeMillis() + 5000
            while (manager.activeNotifications.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(50)
            val sent = manager.activeNotifications.single()
            assertEquals(AgentNotifications.CHANNEL, sent.notification.channelId)
            assertNotNull(sent.notification.contentIntent)
            assertEquals(notice.key, sent.tag)
            assertTrue(sent.notification.extras.getCharSequence("android.text").toString().contains("발생"))
            notifications.show(notice)
            assertEquals(1, manager.activeNotifications.size)
            // Exercise the real PendingIntent and Activity callback. An expired
            // in-memory target must produce an explanation without connecting.
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                var launchIntent: android.content.Intent? = null
                scenario.onActivity { launchIntent = it.intent; sent.notification.contentIntent.send() }
                instrumentation.waitForIdleSync()
                var delivered = false
                val clickDeadline = System.currentTimeMillis() + 5000
                while (!delivered && System.currentTimeMillis() < clickDeadline) {
                    scenario.onActivity { activity ->
                        val model = ViewModelProvider(activity)[ClientViewModel::class.java].app
                        delivered = model.agentNavigationError.value?.contains("종료") == true
                        assertTrue(model.connections.value.isEmpty())
                    }
                    if (!delivered) Thread.sleep(50)
                }
                assertTrue("notification click must report a missing target", delivered)
                // onNewIntent replaces the Activity's intent. Restore the launch
                // identity so ActivityScenario can match its teardown callbacks.
                scenario.onActivity {
                    it.intent = launchIntent
                    ViewModelProvider(it)[ClientViewModel::class.java].app.agentNavigationError.value = null
                }
                instrumentation.waitForIdleSync()
            }
        } finally { manager.cancelAll() }
    }
}
