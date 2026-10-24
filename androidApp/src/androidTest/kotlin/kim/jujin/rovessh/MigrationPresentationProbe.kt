package kim.jujin.rovessh

import android.os.SystemClock
import android.os.Trace
import android.view.KeyEvent
import android.view.ViewTreeObserver
import android.view.Window
import android.webkit.ConsoleMessage
import android.webkit.WebChromeClient
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.*

/** Only in the instrumentation APK. Timestamps come from Perfetto, not JS clocks.
 * VisualStateCallback/onDraw prove content is drawable, NOT that it was presented.
 * The collector must join that draw's frame token to SurfaceFlinger presentation.
 */
class MigrationPresentationProbe(
    private val scenario: ActivityScenario<MainActivity>, private val view: WebView,
    private val firstPresentation: Boolean = false
) {
    @Volatile private var inputId = 0
    @Volatile private var waiting: CompletableDeferred<Unit>? = null
    private var listener: ViewTreeObserver.OnDrawListener? = null
    private var active = true
    private fun mark(name: String) { Trace.beginSection(name); Trace.endSection() }
    private fun clockMark() {
        val before = System.nanoTime()
        val boot = SystemClock.elapsedRealtimeNanos()
        val after = System.nanoTime()
        mark("DGL_R1_CLOCK_${before}_${boot}_$after")
    }

    private suspend fun js(script: String): String = withTimeout(5000) {
        val result = CompletableDeferred<String>()
        withContext(Dispatchers.Main) { view.evaluateJavascript(script) { result.complete(it) } }
        result.await()
    }

    private fun rendered(id: Int) {
        view.post {
            if (!active || id != inputId || waiting == null) return@post
            mark("DGL_R1_RENDER_$id")
            view.postVisualStateCallback(id.toLong(), object : WebView.VisualStateCallback() {
                override fun onComplete(requestId: Long) {
                    if (!active || id != inputId) return
                    mark("DGL_R1_VISUAL_$id")
                    var observed = false
                    val draw = ViewTreeObserver.OnDrawListener {
                        if (active && !observed) {
                            observed = true
                            mark("DGL_R1_DRAW_$id")
                            waiting?.complete(Unit)
                        }
                    }
                    listener = draw
                    view.viewTreeObserver.addOnDrawListener(draw)
                    view.invalidate()
                }
            })
        }
    }

    suspend fun run() {
        var window: Window? = null
        var original: Window.Callback? = null
        var chrome: WebChromeClient? = null
        try {
            withContext(Dispatchers.Main) {
                check(Trace.isEnabled()) { "perfetto_app_tracing_disabled" }
                mark("DGL_R1_PROBE_BEGIN"); clockMark()
                scenario.onActivity {
                    window = it.window; original = it.window.callback
                    it.window.callback = object : Window.Callback by original!! {
                        override fun dispatchKeyEvent(event: KeyEvent): Boolean {
                            if (event.keyCode == KeyEvent.KEYCODE_F10 && event.action == KeyEvent.ACTION_DOWN) {
                                inputId++; mark("DGL_R1_INPUT_$inputId")
                            }
                            return original!!.dispatchKeyEvent(event)
                        }
                    }
                }
                chrome = view.webChromeClient
                view.webChromeClient = object : WebChromeClient() {
                    override fun onConsoleMessage(message: ConsoleMessage): Boolean {
                        val id = message.message().removePrefix("DGL_R1_RENDER_").toIntOrNull()
                        if (message.message().startsWith("DGL_R1_RENDER_") && id != null) rendered(id)
                        if (message.message() == "DGL_R1_NO_CHANGE_$inputId") {
                            mark("DGL_R1_NO_CHANGE_$inputId"); waiting?.complete(Unit)
                        }
                        return true
                    }
                    override fun onPermissionRequest(request: android.webkit.PermissionRequest) { request.deny() }
                }
                view.requestFocus()
            }
            check(js("""(() => {
                const t=window.__resourceTerm; if(!t) return false;
                window.__r1id=0; window.__r1pending=null; window.__r1timer=null;
                window.__r1cursorBlink=t.options.cursorBlink;
                window.__r1keyHandler=t._core._customKeyEventHandler;
                t.options.cursorBlink=false; t.focus();
                const mark=(phase,id)=>performance.mark('DGL_R1_JS_'+phase+'_'+id);
                const snapshot=()=>{
                    const b=t.buffer.active, rows=[];
                    for(let y=0;y<t.rows;y++) rows.push(b.getLine(b.viewportY+y)?.translateToString(false));
                    return JSON.stringify([b.cursorX,b.cursorY,b.viewportY,rows]);
                };
                // Start on a visible empty line. No screen text is written to the trace.
                t.write('\r\n'); t.scrollToBottom();
                t.attachCustomKeyEventHandler(e => {
                    if(e.key !== 'F10') return true;
                    if(e.type === 'keydown') {
                        const id=window.__r1armed, before=snapshot(); window.__r1armed=null;
                        if(!Number.isInteger(id)) return false;
                        mark('KEY',id);
                        const b=t.buffer.active, x=b.cursorX, y=b.baseY+b.cursorY;
                        const char=String.fromCharCode(64+id);
                        const changed=()=>snapshot()!==before && t.buffer.active.getLine(y)?.getCell(x)?.getChars()===char;
                        if ($firstPresentation && id===17) {
                            // Force an unrelated render and prove that it did not change cells/cursor.
                            t.refresh(0,t.rows-1);
                            window.__r1timer=setTimeout(()=>{
                                if(snapshot()!==before) { mark('UNEXPECTED_CHANGE',id); return; }
                                mark('NO_CHANGE',id); console.debug('DGL_R1_NO_CHANGE_'+id);
                            },200);
                        } else {
                            let content=false;
                            const once=t.onRender(event => {
                                if(!content || !changed() || y-b.viewportY<event.start || y-b.viewportY>event.end) return;
                                once.dispose(); window.__r1pending=null;
                                mark('RENDER_CONTENT',id); console.debug('DGL_R1_RENDER_'+id);
                            });
                            window.__r1pending=once;
                            const write=()=>{ mark('WRITE',id); t.write(char,()=>{
                                if(!changed()) { mark('UNEXPECTED_CHANGE',id); return; }
                                content=true; mark('CONTENT',id);
                            }); };
                            if($firstPresentation && id>8) window.__r1timer=setTimeout(write,100); else write();
                        }
                    }
                    return false;
                });
                return true;
            })()""") == "true")
            delay(500)
            val instrumentation = InstrumentationRegistry.getInstrumentation()
            repeat(if (firstPresentation) 17 else 8) {
                check(js("window.__r1armed=${inputId + 1}; true") == "true")
                waiting = CompletableDeferred()
                val now = SystemClock.uptimeMillis()
                // A non-text OS key bypasses IME text conversion; this is not IME acceptance.
                instrumentation.sendKeySync(KeyEvent(now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_F10, 0))
                instrumentation.sendKeySync(KeyEvent(now, now, KeyEvent.ACTION_UP, KeyEvent.KEYCODE_F10, 0))
                withTimeout(5000) { waiting!!.await() }
                withContext(Dispatchers.Main) {
                    listener?.let { view.viewTreeObserver.removeOnDrawListener(it) }; listener = null
                }
                waiting = null
                delay(150)
            }
            check(inputId == if (firstPresentation) 17 else 8)
            withContext(Dispatchers.Main) { clockMark() }
            if (firstPresentation) { delay(300); return }
            // A scroll-only hook smoke check; resize/tab coverage and 3600 samples remain separate.
            withContext(Dispatchers.Main) { mark("DGL_R1_SCROLL_BEGIN") }
            check(js("""(() => { const t=window.__resourceTerm;
                if(t.buffer.active.baseY < 10) return false;
                window.__r1scroll=true; let n=0;
                function step() { if(!window.__r1scroll) return;
                    t.scrollLines(n++%120<60 ? -1 : 1); requestAnimationFrame(step); }
                step(); return true; })()""") == "true")
            delay(3000)
            js("window.__r1scroll=false")
            withContext(Dispatchers.Main) { mark("DGL_R1_SCROLL_END") }
            delay(300) // Let the last frame reach SurfaceFlinger before stopping the trace.
        } finally {
            withContext(NonCancellable + Dispatchers.Main) {
                active = false
                listener?.let { view.viewTreeObserver.removeOnDrawListener(it) }
                view.evaluateJavascript("""window.__r1scroll=false; clearTimeout(window.__r1timer);
                    window.__r1pending?.dispose();
                    if(window.__resourceTerm) {
                        window.__resourceTerm.attachCustomKeyEventHandler(window.__r1keyHandler);
                        window.__resourceTerm.options.cursorBlink=window.__r1cursorBlink;
                    }""", null)
                view.webChromeClient = chrome
                original?.let { window?.callback = it }
            }
        }
    }
}
