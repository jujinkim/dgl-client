package kim.jujin.dglc

import android.content.pm.ApplicationInfo
import android.widget.FrameLayout
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import kim.jujin.dglc.core.*
import kim.jujin.dglc.RustKeyTools
import kim.jujin.dglc.ui.AndroidTerminal
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class AndroidPlatformTest {
    private val instrumentation get() = InstrumentationRegistry.getInstrumentation()
    @Test fun keystoreRoundTripTamperingAndBackupExclusion() {
        val context = instrumentation.targetContext
        val store = AndroidCredentialStore(context)
        val reference = UUID.randomUUID().toString()
        val generated = RustKeyTools.ed25519()
        try {
            store.write(reference, generated.credential)
            assertTrue("Protected credential round trip", generated.credential.privateKey == store.read(reference)?.privateKey)
            val file = File(context.noBackupFilesDir, "credentials/$reference.sealed")
            assertFalse(file.readBytes().toString(Charsets.UTF_8).contains("PRIVATE KEY"))
            val bytes = file.readBytes(); bytes[bytes.lastIndex] = (bytes.last().toInt() xor 1).toByte(); file.writeBytes(bytes)
            assertThrows(Exception::class.java) { store.read(reference) }
            assertEquals(0, context.applicationInfo.flags and ApplicationInfo.FLAG_ALLOW_BACKUP)
        } finally { store.delete(reference) }
    }

    @Test fun bundledRendererImeBridgeResizeAndNavigationIsolation() = runBlocking<Unit> {
        val input = Channel<ByteArray>(32)
        val sizes = Channel<Size>(32)
        lateinit var terminal: AndroidTerminal
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                terminal = AndroidTerminal(activity, Preferences(), { input.trySend(it) }, { sizes.trySend(it) })
                activity.setContentView(FrameLayout(activity).apply { addView(terminal.view, FrameLayout.LayoutParams(-1, -1)) })
            }
            try {
                withTimeout(20_000) { terminal.reset("\u001b[31m한글 日本語\u001b[0m\r\n".toByteArray()) }
                withTimeout(5000) { assertTrue(sizes.receive().columns >= 2) }
                assertTrue(js(terminal, "document.querySelector('.xterm-helper-textarea') !== null").contains("true"))
                for (text in listOf("한글", "日本語")) {
                    js(terminal, """(() => {
                        const t=document.querySelector('.xterm-helper-textarea'); t.focus(); const prefix=t.value;
                        t.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true,data:''}));
                        t.value=prefix+'$text';
                        t.dispatchEvent(new CompositionEvent('compositionupdate',{bubbles:true,data:'$text'}));
                        t.dispatchEvent(new InputEvent('input',{bubbles:true,data:'$text',isComposing:true,inputType:'insertCompositionText'}));
                        t.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'$text'}));
                        return true;
                    })()""")
                    val committed = withTimeout(5000) { input.receive().toString(Charsets.UTF_8) }
                    assertEquals(text, committed)
                    delay(150)
                    assertTrue("IME commit must not be duplicated", input.tryReceive().isFailure)
                }
                terminal.setInputEnabled(false)
                terminal.write("\u001b[?2004h".encodeToByteArray())
                assertEquals("\u001b[200~한글😀\r두 줄\u001b[201~", terminal.preparePaste("한글😀\n두 줄", false).decodeToString())
                assertEquals("\u001b[200~붙여넣기\u001b[201~\r", terminal.preparePaste("붙여넣기", true).decodeToString())
                assertTrue("Preparing paste must never emit bridge input", input.tryReceive().isFailure)
                terminal.write("\u001b[?2004l".encodeToByteArray())
                assertEquals("한글😀\r두 줄\r", terminal.preparePaste("한글😀\r\n두 줄", true).decodeToString())
                assertEquals(65536, terminal.preparePaste("a".repeat(65536), false).size)
                try { terminal.preparePaste("a".repeat(65536), true); fail("Oversized paste accepted") } catch (_: IllegalStateException) { }
                terminal.setInputEnabled(true)
                js(terminal, "window.dgl.postMessage(JSON.stringify({type:'resize',columns:-100,rows:999999})); window.dgl.postMessage('{broken'); true")
                js(terminal, "window.location.href='https://example.invalid/'; true")
                delay(100)
                assertTrue(js(terminal, "location.href").contains("appassets.androidplatform.net"))
                withTimeout(5000) { terminal.write("\u001b[?1049h\u001b[2Jalternate\u001b[?1049l".toByteArray()) }
                scenario.onActivity { terminal.view.layoutParams = terminal.view.layoutParams.apply { height = 500 } }
                withTimeout(5000) { terminal.write("resize test".toByteArray()) }
                withTimeout(5000) { terminal.reset("CACHE_BEFORE".encodeToByteArray()) }
                withTimeout(5000) {
                    while (!js(terminal, "document.querySelector('.xterm-screen').textContent").contains("CACHE_BEFORE")) delay(20)
                }
                terminal.beginSnapshot()
                terminal.reset("CACHE_AFTER".encodeToByteArray())
                delay(200)
                assertTrue(js(terminal, "document.querySelector('.terminal-cache').textContent").contains("CACHE_BEFORE"))
                assertFalse(js(terminal, "document.querySelector('.terminal-cache').textContent").contains("CACHE_AFTER"))
                assertTrue(js(terminal, "getComputedStyle(document.querySelector('.terminal-cache'),'::after').opacity").contains("1"))
                assertTrue(js(terminal, "getComputedStyle(document.querySelector('.terminal-cache')).pointerEvents").contains("none"))
                instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
                    File(instrumentation.targetContext.cacheDir, "resize-cache.png").outputStream().use {
                        bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
                    }
                    bitmap.recycle()
                }
                withTimeout(5000) { terminal.endSnapshot() }
                withTimeout(5000) {
                    while (!js(terminal, "document.querySelector('.terminal-cache') === null").contains("true")) delay(20)
                }
                assertTrue(js(terminal, "document.querySelector('.xterm-screen').textContent").contains("CACHE_AFTER"))
                terminal.beginSnapshot()
                terminal.cancelSnapshot()
                assertTrue(js(terminal, "document.querySelector('.terminal-cache') === null").contains("true"))
                terminal.setSmoothTransitions(true)
                js(terminal, """window.__motion=[]; window.__animate=Element.prototype.animate;
                    Element.prototype.animate=function(frames,options){window.__motion.push({frames,options});return window.__animate.call(this,frames,options);}; true""")
                terminal.beginSnapshot()
                terminal.resizeRemote(Size(60, 12))
                terminal.reset("SMOOTH_AFTER".encodeToByteArray())
                withTimeout(5000) { terminal.endSnapshot() }
                assertTrue(js(terminal, "window.matchMedia('(prefers-reduced-motion: reduce)').matches || (window.__motion.length === 2 && window.__motion.every(a => a.options.duration === 240))").contains("true"))
                js(terminal, "Element.prototype.animate=window.__animate; true")
                terminal.setSmoothTransitions(false)
                terminal.cancelSnapshot()
                assertTrue(js(terminal, "document.querySelector('.xterm-screen').getAnimations().length === 0").contains("true"))
            } finally { instrumentation.runOnMainSync { terminal.close() } }
        }
    }

    @Test fun windowPanesKeepIndependentScreensAndOnlyTheContainerRequestsSize() = runBlocking<Unit> {
        val sizes = Channel<Size>(32)
        lateinit var terminal: AndroidTerminal
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                terminal = AndroidTerminal(activity, Preferences(), {}, { sizes.trySend(it) })
                activity.setContentView(FrameLayout(activity).apply { addView(terminal.view, FrameLayout.LayoutParams(-1, -1)) })
            }
            try {
                terminal.write(byteArrayOf())
                withTimeout(5000) { sizes.receive() }
                val remote = RemoteState(windows = listOf(RemoteWindow("@1", "$1", "nested", true, "", width = 81, height = 24)),
                    panes = listOf(RemotePane("%1", "@1", 40, 24, true, "", "sh"), RemotePane("%2", "@1", 40, 24, false, "", "sh", left = 41)))
                terminal.layout(remote, "@1", "%1", "first")
                for ((pane, text) in listOf("%1" to "LEFT_PERSIST", "%2" to "RIGHT_PERSIST")) {
                    terminal.outputPane(pane, pane)
                    terminal.resizeRemote(Size(40, 24))
                    terminal.reset(text.encodeToByteArray())
                }
                withTimeout(5000) { while (!js(terminal, "[...document.querySelectorAll('.pane .xterm-screen')].map(s=>s.textContent).join('|')").contains("RIGHT_PERSIST")) delay(20) }
                js(terminal, "window.__left=document.querySelectorAll('.pane .xterm')[0]; true")
                terminal.layout(remote, "@1", "%2", "second")
                assertTrue(js(terminal, "window.__left === document.querySelectorAll('.pane .xterm')[0]").contains("true"))
                val screen = js(terminal, "[...document.querySelectorAll('.pane .xterm-screen')].map(s=>s.textContent).join('|')")
                assertTrue(screen.contains("LEFT_PERSIST")); assertTrue(screen.contains("RIGHT_PERSIST"))
                assertEquals("2", js(terminal, "document.querySelectorAll('.pane').length"))
                assertTrue(js(terminal, "Math.abs(document.querySelector('.pane').getBoundingClientRect().height-document.querySelector('.pane .xterm-screen').getBoundingClientRect().height)<2").contains("true"))
                assertTrue(js(terminal, "document.querySelector('#window').click(); document.activeElement.classList.contains('xterm-helper-textarea')").contains("true"))
                delay(150)
                while (sizes.tryReceive().isSuccess) { }
                terminal.outputPane("%2", "resized")
                terminal.resizeRemote(Size(32, 12))
                terminal.write("RIGHT_APPEND".encodeToByteArray())
                delay(100)
                assertTrue("Pane grids must never request client resize", sizes.tryReceive().isFailure)
                assertTrue(js(terminal, "document.querySelectorAll('.pane .xterm-screen')[0].textContent").contains("LEFT_PERSIST"))
                terminal.layout(remote.copy(windows = remote.windows.map { it.copy(zoomed = true) }), "@1", "%2", "zoom")
                assertEquals("1", js(terminal, "[...document.querySelectorAll('.pane')].filter(p=>p.style.display!=='none').length"))
            } finally { instrumentation.runOnMainSync { terminal.close() } }
        }
    }

    private fun js(terminal: AndroidTerminal, script: String): String {
        val latch = CountDownLatch(1); var result = ""
        instrumentation.runOnMainSync { terminal.view.evaluateJavascript(script) { result = it; latch.countDown() } }
        check(latch.await(5, TimeUnit.SECONDS))
        return result
    }
}
