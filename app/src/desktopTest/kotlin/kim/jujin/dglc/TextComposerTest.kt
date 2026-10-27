package kim.jujin.dglc

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kim.jujin.dglc.core.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.*
import kotlin.test.*

@OptIn(ExperimentalCoroutinesApi::class)
class TextComposerTest {
    private class Connection : ClientConnection {
        override val host = HostProfile("composer", "composer", "example.test", username = "test")
        override val refreshing = MutableStateFlow(false)
        override val status = MutableStateFlow(ConnectionStatus(Phase.Ready))
        override val remote = MutableStateFlow(RemoteState())
        override val session = MutableStateFlow<String?>("\$1")
        override val pane = MutableStateFlow<String?>("%1")
        override val trustRequest = MutableStateFlow<UnknownHostKey?>(null)
        override val notice = MutableStateFlow<String?>(null)
        override val inputNotice = SnackbarState()
        override val tmuxProblem = MutableStateFlow<TmuxProblem?>(null)
        var epoch = 1L
        val writes = mutableListOf<ByteArray>()
        private val results = mutableListOf<CompletableDeferred<InputCompletion>>()
        val result get() = results.last()
        var enterSupported = true
        var enterFailure: Exception? = null
        override fun encodeKeyboard(key: String, text: String?, modifiers: TerminalModifiers, applicationCursor: Boolean): ByteArray? {
            assertEquals("enter", key)
            assertNull(text)
            assertEquals(TerminalModifiers(), modifiers)
            assertFalse(applicationCursor)
            return if (enterSupported) TerminalKey.Enter.bytes() else null
        }
        override fun captureInputTarget() = InputTarget(epoch, session.value, pane.value!!)
        override suspend fun sendInput(target: InputTarget, bytes: ByteArray): InputCompletion {
            assertEquals(captureInputTarget(), target)
            if (writes.isNotEmpty()) enterFailure?.let { throw it }
            val result = CompletableDeferred<InputCompletion>()
            writes += bytes
            results += result
            return result.await()
        }
        override fun input(bytes: ByteArray) = error("Composer must use acknowledged input")
        override fun connect() {}
        override fun trust() {}
        override fun selectTarget(session: String, window: String?, pane: String?) {}
        override fun selectSession(id: String) {}
        override fun selectPane(id: String) {}
        override fun createSession(name: String) {}
        override fun manage(command: TmuxCommand) {}
        override fun refresh() {}
        override fun bindSurface(value: TerminalSurface?) {}
        override fun unbindSurface(expected: TerminalSurface) {}
        override fun resize(value: Size) {}
        override fun restore() {}
        override fun disconnect() {}
        override fun close() {}
    }
    private val paste: suspend (String) -> ByteArray = { text -> text.encodeToByteArray() }

    @Test fun unicodeSelectionDraftAndCompletionAreConnectionOwned() = runTest {
        val client = Connection()
        val draft = TextComposer(client, this)
        val other = TextComposer(Connection(), this)
        assertFalse(draft.includeEnter.value)
        val edited = TextFieldValue("한글😀\n두 번째 줄", TextRange(2, 4))
        draft.open(); draft.edit(edited); draft.dismiss(); draft.open()
        assertEquals(edited, draft.value.value)
        assertTrue(other.value.value.text.isEmpty())
        draft.send({ true }, paste); draft.send({ true }, paste); runCurrent()
        assertEquals(1, client.writes.size)
        assertContentEquals(edited.text.encodeToByteArray(), client.writes.single())
        assertTrue(draft.sending.value)
        draft.edit(TextFieldValue("cannot overwrite in flight"))
        draft.dismiss()
        assertEquals(edited, draft.value.value)
        client.result.complete(InputCompletion()); runCurrent()
        assertEquals("", draft.value.value.text)
        assertFalse(draft.opened.value)
        assertFalse(draft.sending.value)
        draft.close(); other.close()
    }

    @Test fun failedSendPreservesDraftAndNeverAutomaticallyRetries() = runTest {
        val client = Connection(); val draft = TextComposer(client, this)
        draft.open(); draft.edit(TextFieldValue("한글\n😀")); draft.includeEnter.value = true
        draft.send({ true }, paste); runCurrent()
        assertEquals("한글\n😀", client.writes.single().decodeToString())
        client.result.complete(InputCompletion("Network")); runCurrent(); advanceUntilIdle()
        assertEquals("한글\n😀", draft.value.value.text)
        assertEquals("Network", draft.error.value)
        assertEquals(1, client.writes.size)
        assertFalse(draft.sending.value)
        draft.close()
        assertTrue(draft.value.value.text.isEmpty())
    }

    @Test fun changedTargetsBeforeAndDuringPreparationDoNotSend() = runTest {
        for (change in 0..3) {
            val client = Connection(); val draft = TextComposer(client, this)
            var selected = true
            draft.open(); draft.edit(TextFieldValue("draft"))
            val ready = CompletableDeferred<Unit>()
            draft.send({ selected }) { text -> ready.await(); paste(text) }
            runCurrent()
            when (change) {
                0 -> client.epoch++
                1 -> client.session.value = "\$2"
                2 -> client.pane.value = "%2"
                3 -> selected = false
            }
            ready.complete(Unit); runCurrent()
            draft.send({ selected }, paste); runCurrent()
            assertTrue(client.writes.isEmpty())
            assertEquals("draft", draft.value.value.text)
            assertNotNull(draft.error.value)
            draft.close()
        }
    }

    @Test fun emptyAndOversizedPastesNeverEnterQueue() = runTest {
        val client = Connection(); val draft = TextComposer(client, this)
        draft.open(); draft.send({ true }, paste); runCurrent()
        draft.edit(TextFieldValue("한".repeat(21846))); draft.send({ true }, paste); runCurrent()
        draft.edit(TextFieldValue("a".repeat(65536))); draft.includeEnter.value = true
        draft.send({ true }) { text -> "\u001b[200~${text}\u001b[201~".encodeToByteArray() }; runCurrent()
        assertTrue(client.writes.isEmpty())
        assertNotNull(draft.error.value)
        draft.send({ true }, paste); runCurrent()
        assertEquals(65536, client.writes.single().size)
        client.result.complete(InputCompletion()); runCurrent()
        assertEquals(2, client.writes.size)
        assertContentEquals(TerminalKey.Enter.bytes(), client.writes.last())
        client.result.complete(InputCompletion()); runCurrent(); draft.close()
    }

    @Test fun enterWaitsForTextCompletionAndIsAcknowledgedSeparately() = runTest {
        val client = Connection(); val draft = TextComposer(client, this)
        val edited = TextFieldValue("한글😀\n끝\n", TextRange(2, 4))
        draft.open(); draft.edit(edited); draft.includeEnter.value = true
        draft.send({ true }, paste); draft.send({ true }, paste); runCurrent()
        assertEquals(1, client.writes.size)
        assertContentEquals(edited.text.encodeToByteArray(), client.writes.single())
        assertTrue(draft.sending.value)
        client.result.complete(InputCompletion()); runCurrent()
        assertEquals(2, client.writes.size)
        assertContentEquals(TerminalKey.Enter.bytes(), client.writes.last())
        assertEquals(edited, draft.value.value)
        assertTrue(draft.opened.value)
        assertTrue(draft.sending.value)
        draft.edit(TextFieldValue("blocked")); draft.send({ true }, paste); runCurrent()
        assertEquals(edited, draft.value.value)
        assertEquals(2, client.writes.size)
        client.result.complete(InputCompletion()); runCurrent()
        assertEquals("", draft.value.value.text)
        assertFalse(draft.includeEnter.value)
        assertFalse(draft.opened.value)
        assertFalse(draft.sending.value)
        assertNull(draft.error.value)
        draft.close()
    }

    @Test fun enterFailurePreservesDeliveredTextWithoutRetry() = runTest {
        for (failure in 0..2) {
            val client = Connection(); val draft = TextComposer(client, this)
            draft.open(); draft.edit(TextFieldValue("delivered")); draft.includeEnter.value = true
            draft.send({ true }, paste); runCurrent()
            when (failure) {
                1 -> client.enterSupported = false
                2 -> client.enterFailure = IllegalStateException("queue full")
            }
            client.result.complete(InputCompletion()); runCurrent()
            if (failure == 0) { client.result.complete(InputCompletion("Network")); runCurrent() }
            advanceUntilIdle()
            assertEquals(if (failure == 0) 2 else 1, client.writes.size)
            assertEquals("delivered", draft.value.value.text)
            assertTrue(draft.includeEnter.value)
            assertTrue(draft.opened.value)
            assertFalse(draft.sending.value)
            assertContains(draft.error.value!!, "Enter")
            draft.close()
        }
    }

    @Test fun changedTargetsAfterTextDoNotSendEnter() = runTest {
        for (change in 0..3) {
            val client = Connection(); val draft = TextComposer(client, this)
            var selected = true
            draft.open(); draft.edit(TextFieldValue("delivered")); draft.includeEnter.value = true
            draft.send({ selected }, paste); runCurrent()
            when (change) {
                0 -> client.epoch++
                1 -> client.session.value = "\$2"
                2 -> client.pane.value = "%2"
                3 -> selected = false
            }
            client.result.complete(InputCompletion()); runCurrent(); advanceUntilIdle()
            assertEquals(1, client.writes.size)
            assertEquals("delivered", draft.value.value.text)
            assertContains(draft.error.value!!, "Enter")
            assertFalse(draft.sending.value)
            draft.close()
        }
    }

    @Test fun sizeGenerationIsNotAnInputDestinationChange() {
        fun target(connection: Int = 1, pane: Int = 2, size: Int = 3) =
            """{"connection":7,"pane_id":"%1","generation":{"connection":$connection,"pane":$pane,"size":$size}}"""
        assertEquals(inputTargetIdentity(target()), inputTargetIdentity(target(size = 99)))
        assertNotEquals(inputTargetIdentity(target()), inputTargetIdentity(target(connection = 2)))
        assertNotEquals(inputTargetIdentity(target()), inputTargetIdentity(target(pane = 3)))
    }
}
