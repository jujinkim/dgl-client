package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kotlin.test.*

class InputRejectionsTest {
    @Test fun nativeCodesDistinguishPreparationQueueAndChangedTargets() {
        val messages = listOf("NotReady", "QueueFull", "StaleGeneration", "Cancelled").map(::nativeInputError)
        assertEquals(4, messages.toSet().size)
        for (code in listOf("NotReady", "QueueFull", "Cancelled", "Closed", "Network")) {
            assertFalse(nativeInputError(code).contains("대상 변경"))
            assertFalse(nativeInputError(code).contains("패널이 변경"))
        }
    }
    @Test fun repeatedRejectionsCoalesceWithinEachRecoveryCycle() {
        val gate = InputRejections()
        gate.phase(Phase.Restoring)
        assertNotNull(gate.reject("NotReady"))
        assertNull(gate.reject("NotReady"))
        gate.phase(Phase.Restoring)
        assertNull(gate.reject("QueueFull"))
        gate.phase(Phase.Ready)
        assertNotNull(gate.reject("QueueFull"))
        gate.phase(Phase.Reconnecting)
        assertNotNull(gate.reject("NotReady"))
        assertNull(gate.reject("NotReady"))
        gate.phase(Phase.Restoring)
        gate.phase(Phase.Failed)
        assertNull(gate.reject("Closed"), "restore and exhausted failure belong to the same cycle")
        gate.status(ConnectionStatus(phase = Phase.Ready))
        gate.status(ConnectionStatus(phase = Phase.Ready, checking = true))
        assertNotNull(gate.reject("NotReady"))
        gate.phase(Phase.Reconnecting)
        assertNull(gate.reject("NotReady"), "failed check and reconnect belong to the same cycle")
    }
}
