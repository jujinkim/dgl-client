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
                terminal.layout(remote, "@1", "%2", "before-recovery")
                terminal.connectionRecovery(true)
                assertTrue(js(terminal, "document.querySelector('.connection-cache').textContent").contains("LEFT_PERSIST"))
                assertTrue(js(terminal, "document.querySelector('.connection-cache').textContent").contains("RIGHT_PERSIST"))
                // The new layout can remove every old pane while tmux reconnects.
                terminal.layout(RemoteState(), null, "shell", null)
                terminal.outputPane("shell", "restoring")
                terminal.reset("PARTIAL_RESTORATION".encodeToByteArray())
                terminal.cancelSnapshot()
                terminal.connectionRecovery(true)
                assertEquals("0", js(terminal, "document.querySelectorAll('#window .pane').length"))
                val frozen = js(terminal, "document.querySelector('.connection-cache').textContent")
                assertTrue(frozen.contains("LEFT_PERSIST")); assertTrue(frozen.contains("RIGHT_PERSIST"))
                assertFalse(frozen.contains("PARTIAL_RESTORATION"))
                assertEquals("1", js(terminal, "document.querySelectorAll('.connection-cache').length"))
                assertTrue(js(terminal, "getComputedStyle(document.querySelector('.connection-cache')).pointerEvents === 'none'").contains("true"))
                terminal.connectionRecovery(false)
                assertTrue(js(terminal, "document.querySelector('.connection-cache') === null").contains("true"))
            } finally { instrumentation.runOnMainSync { terminal.close() } }
        }
    }

    @Test fun bundledRendererRemoteColumnsAndTouchScrolling() = runBlocking<Unit> {
        val input = Channel<Pair<String?, ByteArray>>(64)
        val selected = Channel<String>(8)
        lateinit var terminal: AndroidTerminal
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            var stage = "remote columns"
            scenario.onActivity { activity ->
                terminal = AndroidTerminal(activity, Preferences(), {}, {},
                    paneInput = { _, target, bytes -> input.trySend(target to bytes) },
                    focusPane = { pane, _ -> selected.trySend(pane) })
                activity.setContentView(FrameLayout(activity).apply { addView(terminal.view, FrameLayout.LayoutParams(-1, -1)) })
                assertFalse(terminal.view.isVerticalScrollBarEnabled)
                assertFalse(terminal.view.isHorizontalScrollBarEnabled)
            }
            suspend fun awaitJs(script: String) = withTimeout(5000) {
                while (js(terminal, script) != "true") delay(20)
            }
            suspend fun expectInput(value: String, target: String = "one") {
                val received = withTimeout(5000) { input.receive() }
                assertEquals(target, received.first)
                assertArrayEquals(value.toByteArray(Charsets.ISO_8859_1), received.second)
            }
            try {
                withTimeout(20_000) { terminal.write(byteArrayOf()) }
                // Observe actual bundled instances without adding a product test API.
                js(terminal, """window.__terms=[]; window.__open=Terminal.prototype.open;
                    Terminal.prototype.open=function(host){window.__terms.push(this);return window.__open.call(this,host);}; true""")
                val remote = RemoteState(windows = listOf(RemoteWindow("@1", "${'$'}1", "touch", true, "", width = 80, height = 24)),
                    panes = listOf(RemotePane("%1", "@1", 80, 24, true, "", "sh")))
                terminal.layout(remote, "@1", "%1", "one")
                terminal.outputPane("%1", "one")
                terminal.resizeRemote(Size(80, 24))
                terminal.reset(("\u001bc" + "X".repeat(79) + "ZQ").encodeToByteArray())
                awaitJs("window.__terms[0].element.querySelector('.xterm-rows').children[1].textContent.startsWith('Q')")
                assertEquals("80", js(terminal, "window.__terms[0].cols"))
                assertTrue("Rendered rows: " + js(terminal, "[...window.__terms[0].element.querySelector('.xterm-rows').children].slice(0,2).map(r=>r.textContent)"), js(terminal, """(() => {
                    const t=window.__terms[0], rows=t.element.querySelector('.xterm-rows').children;
                    return rows[0].textContent==='X'.repeat(79)+'Z' && rows[1].textContent.trimEnd()==='Q' &&
                        t.buffer.active.getLine(0).translateToString(true)==='X'.repeat(79)+'Z' &&
                        t.buffer.active.getLine(1).translateToString(true)==='Q';
                })()""").contains("true"))
                val geometry = org.json.JSONObject(js(terminal, """(() => {
                    const t=window.__terms[0], screen=t.element.querySelector('.xterm-screen'), host=t.element.parentElement;
                    const root=document.querySelector('#terminal'), bars=[...t.element.querySelectorAll('.xterm-scrollable-element > .scrollbar')];
                    root.scrollLeft=root.scrollWidth;
                    return {bars:bars.length, hidden:bars.every(b=>getComputedStyle(b).display==='none'),
                        scrollbar:getComputedStyle(root,'::-webkit-scrollbar').display, touch:getComputedStyle(root).touchAction,
                        gutter:host.getBoundingClientRect().width-screen.getBoundingClientRect().width,
                        overflow:screen.getBoundingClientRect().right-root.getBoundingClientRect().right};
                })()"""))
                assertTrue(geometry.toString(), geometry.getInt("bars") > 0 && geometry.getBoolean("hidden") &&
                    geometry.getString("scrollbar") == "none" && geometry.getString("touch") == "pan-x" &&
                    kotlin.math.abs(geometry.getDouble("gutter")) < 1 && geometry.getDouble("overflow") <= 1)
                js(terminal, """window.__point=(type,y,x=50,index=0,id=7)=>{
                    const t=window.__terms[index], s=t.element.querySelector('.xterm-screen').getBoundingClientRect();
                    t.element.dispatchEvent(new PointerEvent(type,{bubbles:true,cancelable:true,pointerType:'touch',
                        pointerId:id,isPrimary:true,clientX:s.left+x,clientY:s.top+y}));
                };
                window.__drag=(rows,index=0,x=50)=>{
                    const t=window.__terms[index], h=t.element.querySelector('.xterm-screen').getBoundingClientRect().height/t.rows;
                    window.__point('pointerdown',100,x,index);window.__point('pointermove',100+rows*h,x,index);
                    window.__point('pointerup',100+rows*h,x,index);
                }; true""")
                stage = "SGR wheel"
                terminal.write("\u001b[?1000h\u001b[?1006h".encodeToByteArray())
                js(terminal, "window.__drag(2); true")
                repeat(2) {
                    stage = "SGR up report $it"
                    val received = withTimeout(5000) { input.receive() }
                    assertEquals("one", received.first)
                    assertTrue(received.second.decodeToString().matches(Regex("\u001b\\[<64;\\d+;\\d+M")))
                }
                stage = "SGR down"
                js(terminal, "window.__drag(-1); true")
                assertTrue(withTimeout(5000) { input.receive() }.second.decodeToString().matches(Regex("\u001b\\[<65;\\d+;\\d+M")))
                stage = "binary wheel"
                terminal.write("\u001b[?1006l".encodeToByteArray())
                js(terminal, "window.__drag(1,0,window.__terms[0].element.querySelector('.xterm-screen').getBoundingClientRect().width*0.95); true")
                val binary = withTimeout(5000) { input.receive() }.second
                assertEquals(6, binary.size); assertArrayEquals(byteArrayOf(27, 91, 77, 96), binary.copyOfRange(0, 4))
                // Increase the grid so a legacy coordinate exceeds ASCII, proving
                // that the binary bridge does not accidentally UTF-8 encode it.
                stage = "binary high byte"
                terminal.resizeRemote(Size(160, 24))
                awaitJs("window.__terms[0].cols===160")
                js(terminal, "window.__drag(1,0,window.__terms[0].element.querySelector('.xterm-screen').getBoundingClientRect().width*0.8); true")
                val highByte = withTimeout(5000) { input.receive() }.second
                assertEquals(6, highByte.size); assertTrue((highByte[4].toInt() and 255) > 127)
                stage = "alternate screen arrows"
                terminal.resizeRemote(Size(80, 24))
                terminal.write("\u001b[?1000l\u001b[?1049h\u001b[?1l".encodeToByteArray())
                js(terminal, "window.__drag(1); true"); expectInput("\u001b[A")
                js(terminal, "window.__drag(-1); true"); expectInput("\u001b[B")
                terminal.write("\u001b[?1h".encodeToByteArray())
                js(terminal, "window.__drag(1); true"); expectInput("\u001bOA")
                stage = "local history"
                terminal.write(("\u001b[?1049l" + (1..80).joinToString("") { "\r\nHISTORY_${it}" }).encodeToByteArray())
                awaitJs("window.__terms[0].buffer.active.baseY>20")
                js(terminal, "window.__before=window.__terms[0].buffer.active.viewportY; window.__drag(3); true")
                awaitJs("window.__terms[0].buffer.active.viewportY===window.__before-3")
                js(terminal, "window.__drag(-1); true")
                awaitJs("window.__terms[0].buffer.active.viewportY===window.__before-2")
                assertTrue("History must not send input", input.tryReceive().isFailure)

                // Real Android touch delivery checks pan-x, compatibility click
                // suppression and IME behavior, not just synthetic DOM events.
                stage = "native touch"
                js(terminal, """document.querySelector('#terminal').scrollLeft=0; window.__before=window.__terms[0].buffer.active.viewportY;
                    window.__touchTrace=[];
                    for(const name of ['pointerdown','pointermove','pointerup','pointercancel','gotpointercapture','lostpointercapture','mousedown','click'])
                        document.addEventListener(name,e=>window.__touchTrace.push({type:e.type,pointer:e.pointerType,primary:e.isPrimary,x:e.clientX,y:e.clientY,target:e.target.className}),true);
                    window.addEventListener('error',e=>window.__touchTrace.push({error:e.message})); true""")
                val location = IntArray(2)
                instrumentation.runOnMainSync { terminal.view.getLocationOnScreen(location) }
                val cssScale = terminal.view.width / js(terminal, "document.querySelector('#terminal').clientWidth").toFloat()
                var down = android.os.SystemClock.uptimeMillis()
                suspend fun touch(action: Int, offset: Float, offsetX: Float = 0f) {
                    // The remote pane can be shorter than the WebView. Start
                    // inside its cells, not in the unused bottom half.
                    val properties = arrayOf(android.view.MotionEvent.PointerProperties().apply {
                        id = 0; toolType = android.view.MotionEvent.TOOL_TYPE_FINGER
                    })
                    val coords = arrayOf(android.view.MotionEvent.PointerCoords().apply {
                        x = location[0] + 50f * cssScale + offsetX; y = location[1] + 100f * cssScale + offset
                        pressure = 1f; size = 1f
                    })
                    val event = android.view.MotionEvent.obtain(down, android.os.SystemClock.uptimeMillis(), action, 1,
                        properties, coords, 0, 0, 1f, 1f, 0, 0, android.view.InputDevice.SOURCE_TOUCHSCREEN, 0)
                    try { assertTrue(instrumentation.uiAutomation.injectInputEvent(event, true)) } finally { event.recycle() }
                    delay(20)
                }
                touch(android.view.MotionEvent.ACTION_DOWN, 0f)
                for (step in 1..8) touch(android.view.MotionEvent.ACTION_MOVE, step * 15f)
                touch(android.view.MotionEvent.ACTION_UP, 120f)
                assertTrue(js(terminal, "window.__touchTrace.some(e=>e.type==='pointerdown' && e.pointer==='touch')").contains("true"))
                awaitJs("window.__terms[0].buffer.active.viewportY<window.__before")
                delay(150)
                assertTrue(js(terminal, "!document.activeElement.classList.contains('xterm-helper-textarea')").contains("true"))
                assertTrue(input.tryReceive().isFailure)
                scenario.onActivity { assertFalse(terminal.view.rootWindowInsets.isVisible(android.view.WindowInsets.Type.ime())) }

                stage = "native horizontal pan"
                val historyBeforePan = js(terminal, "window.__terms[0].buffer.active.viewportY")
                down = android.os.SystemClock.uptimeMillis()
                val startX = 180f * cssScale
                touch(android.view.MotionEvent.ACTION_DOWN, 0f, startX)
                for (step in 1..8) touch(android.view.MotionEvent.ACTION_MOVE, 0f, startX - step * 15f)
                touch(android.view.MotionEvent.ACTION_UP, 0f, startX - 120f)
                awaitJs("document.querySelector('#terminal').scrollLeft>0")
                assertEquals(historyBeforePan, js(terminal, "window.__terms[0].buffer.active.viewportY"))
                assertTrue(input.tryReceive().isFailure)

                stage = "split selection"
                val split = remote.copy(windows = remote.windows.map { it.copy(width = 81) }, panes = listOf(
                    RemotePane("%1", "@1", 40, 24, true, "", "sh"), RemotePane("%2", "@1", 40, 24, false, "", "sh", left = 41)))
                terminal.layout(split, "@1", "%1", "one")
                for (pane in listOf("%1", "%2")) {
                    terminal.outputPane(pane, "one"); terminal.resizeRemote(Size(40, 24))
                    terminal.reset(("\u001bc" + "L".repeat(39) + "RQ\u001b[?1000h\u001b[?1006h").encodeToByteArray())
                }
                // Snapshot replacement intentionally preserves a history reader's
                // position. Inspect the new split grid at the live bottom.
                js(terminal, "window.__terms.forEach(t=>t.scrollToBottom()); true")
                awaitJs("window.__terms.every(t=>t.cols===40 && t.element.querySelector('.xterm-rows').children[0].textContent==='L'.repeat(39)+'R')")
                js(terminal, "window.__drag(2,1); true")
                assertEquals("%2", withTimeout(5000) { selected.receive() })
                assertTrue(input.tryReceive().isFailure)
                terminal.layout(split, "@1", "%2", "two")
                js(terminal, "window.__drag(1,1); true")
                assertEquals("two", withTimeout(5000) { input.receive() }.first)
                // A partial row cannot be replayed after input destination changes.
                js(terminal, "window.__point('pointerdown',100,50,1); window.__point('pointermove',110,50,1); true")
                terminal.layout(split, "@1", "%2", "three")
                js(terminal, "window.__point('pointermove',180,50,1); window.__point('pointerup',180,50,1); true")
                delay(100); assertTrue(input.tryReceive().isFailure)
            } catch (failure: Throwable) {
                throw AssertionError("Touch stage: $stage; renderer=" + js(terminal, "({terms:window.__terms.map(t=>({cols:t.cols,rows:t.rows,mode:t.modes.mouseTrackingMode,buffer:t.buffer.active.type,viewport:t.buffer.active.viewportY})),trace:window.__touchTrace})"), failure)
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
