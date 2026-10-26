package kim.jujin.dglc.core

import kotlin.test.*

class BackgroundResumeTest {
    @Test fun backgroundStopKeepsTargetAndResumesOnlyOnce() {
        val resume = BackgroundResume()
        resume.suspend(Phase.Ready, false, "$1", "%2")
        resume.suspend(Phase.Disconnected, false, null, null)
        assertTrue(resume.pending)
        assertEquals(BackgroundResume.Target("$1", "%2"), resume.take())
        assertNull(resume.take())
        assertFalse(resume.pending)
        resume.suspend(Phase.Shell, false, null, null)
        assertEquals(BackgroundResume.Target(null, null), resume.take())
    }
    @Test fun explicitStopsFailuresAndMissingSessionsNeverReserveResume() {
        val resume = BackgroundResume()
        for (phase in listOf(Phase.Disconnected, Phase.Failed, Phase.TrustRequired, Phase.MissingSession)) {
            resume.suspend(phase, false, "$1", "%2")
            assertNull(resume.take(), phase.name)
        }
        for (phase in Phase.entries) {
            resume.suspend(phase, true, "$1", "%2")
            assertNull(resume.take(), phase.name)
        }
        for (reason in listOf("disconnect", "close", "stop retrying", "task removal")) {
            resume.suspend(Phase.Reconnecting, false, "$1", "%2")
            resume.cancel()
            assertNull(resume.take(), reason)
        }
    }
}
