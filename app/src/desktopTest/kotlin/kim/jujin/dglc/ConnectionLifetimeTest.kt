package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kotlin.test.*

class ConnectionLifetimeTest {
    private class Fixture(val startSucceeds: Boolean = true) {
        var starts = 0; var stops = 0; var disconnects = 0; var disposals = 0; var awake = false
        val lifetime = ConnectionLifetime({ starts++; startSucceeds }, { stops++ }, { awake = it }, { disconnects++ }, { disposals++ })
        fun update(phase: Phase, visible: Boolean = true, keep: Boolean = true) = lifetime.update(keep, visible, listOf(phase))
    }
    @Test fun homeLockAndRecreationReuseConnectionsUntilTaskRemoval() {
        val f = Fixture()
        f.update(Phase.Connecting)
        f.update(Phase.Ready, visible = false)
        assertTrue(f.awake)
        f.update(Phase.Restoring, visible = false)
        f.update(Phase.Ready)
        assertFalse(f.awake)
        assertEquals(1, f.starts)
        assertEquals(0, f.disconnects)
        f.lifetime.taskRemoved(); f.lifetime.taskRemoved()
        assertFalse(f.awake)
        assertEquals(1, f.stops)
        assertEquals(1, f.disposals)
        f.update(Phase.Connecting)
        assertEquals(1, f.starts, "a removed task cannot restart its engine")
    }
    @Test fun disablingMaintenanceAndRetryExhaustionReleaseResources() {
        val f = Fixture()
        f.update(Phase.Ready, visible = false)
        assertTrue(f.awake)
        f.update(Phase.Failed, visible = false)
        assertFalse(f.awake)
        assertEquals(1, f.stops)
        f.update(Phase.Reconnecting)
        assertEquals(2, f.starts)
        f.update(Phase.Ready, keep = false)
        assertEquals(0, f.disconnects, "changing a preference must preserve foreground SSH")
        f.update(Phase.Ready, visible = false, keep = false)
        f.update(Phase.Restoring, visible = false, keep = false)
        assertEquals(1, f.disconnects)
        assertEquals(2, f.stops)
        assertFalse(f.awake)
    }
    @Test fun rejectedOrLostServiceDoesNotReconnectOrLoop() {
        val f = Fixture(false)
        f.update(Phase.Connecting)
        f.update(Phase.Ready, visible = false)
        assertFalse(f.awake)
        assertEquals(1, f.starts)
        assertEquals(0, f.disconnects)
        val running = Fixture()
        running.update(Phase.Ready, visible = false)
        running.lifetime.serviceFailed()
        running.update(Phase.Reconnecting, visible = false)
        assertFalse(running.awake)
        assertEquals(1, running.starts)
        running.update(Phase.Disconnected)
        assertEquals(1, running.stops)
    }
    @Test fun maintenanceContinuesWhileAnyConnectionIsActive() {
        val f = Fixture()
        f.lifetime.update(true, false, listOf(Phase.Failed, Phase.Ready))
        assertTrue(f.awake)
        f.lifetime.update(true, false, listOf(Phase.Failed, Phase.Disconnected))
        assertFalse(f.awake)
        assertEquals(1, f.stops)
    }
}
