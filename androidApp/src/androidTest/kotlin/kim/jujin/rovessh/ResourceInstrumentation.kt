package kim.jujin.rovessh

import android.app.Activity
import android.os.Bundle
import android.os.Debug
import android.os.Process
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.runner.AndroidJUnitRunner
import kim.jujin.rovessh.core.*
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.io.File
import java.util.WeakHashMap

/** Explicit instrumentation component; never contributes a fifth regression test. */
class ResourceInstrumentation : AndroidJUnitRunner() {
    override fun onStart() {
        var success = false
        try {
            val config = Json.parseToJsonElement(File(targetContext.cacheDir, "resource-config.json").readText()).jsonObject.toMutableMap()
            config["appPid"] = JsonPrimitive(Process.myPid())
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                val platform = AndroidResourcePlatform(scenario, config["presentationMode"]?.jsonPrimitive?.content == "first-presentation")
                try { runBlocking { ResourceScenario(JsonObject(config), File(targetContext.cacheDir, "resource-state.json"), platform).run() }; success = true }
                finally { platform.restore() }
            }
        } catch (e: Throwable) { android.util.Log.e("ResourceProbe", "Probe failed: ${e.javaClass.simpleName}", e) }
        finish(if (success) Activity.RESULT_OK else Activity.RESULT_CANCELED, Bundle().apply { putString("resource_result", if (success) "passed" else "failed") })
    }
}
private class AndroidResourcePlatform(private val scenario: ActivityScenario<MainActivity>, private val firstPresentation: Boolean) : ResourcePlatform {
    private lateinit var app: AppModel
    private lateinit var original: LocalState
    private val live = WeakHashMap<WebView, Boolean>()
    init {
        scenario.onActivity { activity ->
            app = (activity.application as DglApplication).connections().app
            original = app.local.value
            app.update(original.copy(preferences = original.preferences.copy(scrollback = 10000)))
        }
    }
    fun restore() { app.update(original) }
    private fun find(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) for (i in 0 until view.childCount) find(view.getChildAt(i))?.let { return it }
        return null
    }
    private suspend fun web(): WebView = withTimeout(15000) {
        var result: WebView? = null
        while (result == null) {
            withContext(Dispatchers.Main) { scenario.onActivity { result = find(it.window.decorView) } }
            if (result == null) delay(50)
        }
        result!!
    }
    private suspend fun js(view: WebView, script: String): String = withTimeout(5000) {
        val result = CompletableDeferred<String>()
        withContext(Dispatchers.Main) { view.evaluateJavascript(script) { result.complete(it) } }
        result.await()
    }
    override suspend fun attach(model: ClientModel) {
        scenario.onActivity { app.connections.value = listOf(model); app.selected.value = model }
        val view = web()
        // Instrument the actual xterm instance without changing the bundled asset/public API.
        withTimeout(15000) {
            while (js(view, "typeof Terminal === 'function' && typeof window.receive === 'function'") != "true") delay(50)
        }
        js(view, """(() => { const write=Terminal.prototype.write;
          Terminal.prototype.write=function(data,ack) { window.__resourceTerm=this; return write.call(this,data,ack); };
          window.__resourceReceive=window.receive; window.__resourcePending=0; window.__resourceDelayed=0;
          window.receive=function(b,r,id) { window.__resourcePending=b.length;
            const deliver=()=>{window.__resourceReceive(b,r,id); window.__resourcePending=0;};
            if(window.__resourceSlow) { window.__resourceDelayed++; setTimeout(deliver,3000); } else deliver(); };
          return true; })()""")
        synchronized(live) { live[view] = true }
    }
    override suspend fun detach(model: ClientModel) {
        scenario.onActivity { app.connections.value = emptyList(); app.selected.value = null }
        withTimeout(10000) {
            while (true) { var exists = false; scenario.onActivity { exists = find(it.window.decorView) != null }; if (!exists) break; delay(50) }
        }
    }
    override suspend fun recreate() { scenario.recreate() }
    override suspend fun presentation() { MigrationPresentationProbe(scenario, web(), firstPresentation).run() }
    override suspend fun slowConsumer(enabled: Boolean) {
        val view = web()
        if (!enabled) check(js(view, "window.__resourceDelayed > 0") == "true") { "backpressure_not_exercised" }
        js(view, "window.__resourceSlow=$enabled")
    }
    override suspend fun metrics(): Map<String, Long> {
        val status = File("/proc/self/status").readText()
        fun value(name: String) = Regex("(?m)^$name:\\s+(\\d+)").find(status)?.groupValues?.get(1)?.toLong() ?: error("missing_$name")
        var view: WebView? = null
        // onActivity from a worker calls waitForIdleSync, which can starve under
        // continuous terminal output. Main-thread dispatch performs the bounded
        // lookup directly, without waiting for the application to become idle.
        withContext(Dispatchers.Main) {
            if (app.selected.value != null) scenario.onActivity { view = find(it.window.decorView) }
        }
        var scrollback = 0L; var rendered = 0L; var queued = 0L
        view?.let {
            val raw = js(it, """JSON.stringify((()=>{const t=window.__resourceTerm;if(!t)return {h:0,n:0,q:0};
                const b=t.buffer.active;let n=0;for(let i=b.baseY;i<b.length;i++){
                const m=b.getLine(i).translateToString().match(/DGL:(\d+)/);if(m)n=Math.max(n,Number(m[1]));}
                return {h:b.baseY,n:n,q:window.__resourcePending||0};})())""")
            if (raw != "null") {
                val metrics = Json.parseToJsonElement(Json.parseToJsonElement(raw).jsonPrimitive.content).jsonObject
                scrollback = metrics.getValue("h").jsonPrimitive.long; rendered = metrics.getValue("n").jsonPrimitive.long; queued = metrics.getValue("q").jsonPrimitive.long
            }
        }
        check(scrollback <= 10000); check(queued <= 22000)
        return mapOf("rssBytes" to value("VmRSS") * 1024, "osPeakRssBytes" to value("VmHWM") * 1024,
            "fd" to (File("/proc/self/fd").list()?.size?.toLong() ?: error("missing_fd")), "threads" to value("Threads"),
            "gcCount" to (Debug.getRuntimeStat("art.gc.gc-count")?.toLongOrNull() ?: error("missing_gc")),
            "gcMillis" to (Debug.getRuntimeStat("art.gc.gc-time")?.toLongOrNull() ?: error("missing_gc_time")),
            "scrollback" to scrollback, "renderSequence" to rendered, "queuedBytes" to queued,
            "liveTerminals" to synchronized(live) { live.size.toLong() })
    }
}
