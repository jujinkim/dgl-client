package kim.jujin.dglc.core

import kotlinx.serialization.json.Json
import kotlin.test.*

class AgentStatusTest {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
    private fun observation(pane: String, agent: String, status: String?, window: String = "@1") =
        """{"session":"$1","window":"$window","pane":"$pane","run":"run$pane","actor":"parent","agent":"$agent",
            "last":null,"supported":[]${status?.let { ",\"status\":$it" } ?: ""},"futureObservationField":1}"""
    private fun snapshot(vararg observations: String) =
        json.decodeFromString(AgentSnapshot.serializer(), """{"server":"s","valid":true,"observations":[${observations.joinToString(",")}]}""")

    @Test fun fullSampleParsesWithWireNamesAndUnknownFieldsAreIgnored() {
        val previous = I18n.language
        try {
            I18n.configure("en", "ko")
            val status = """{"at":1763790000000,"sources":["mod","hook"],"model":{"id":"claude-opus-4-x","name":"Opus 4.x","future":1},
                "effort":"high","thinking":true,
                "context":{"used":84000,"size":200000,"percent":42.0,"compact_percent":85.0,
                  "breakdown":[{"category":"messages","name":"Messages","tokens":30000,"kind":"used"},
                               {"category":"future_kind","name":"Plugin data","tokens":10}],
                  "breakdown_total":120000,"breakdown_window":200000},
                "cache":{"input":1200,"read":80000,"write":2800,"output":900,"ttl":"5m","last_at":1763790000000},
                "cost_usd":1.23,"account":{"kind":"subscription","plan":"max","key":"abc"},
                "limits":[{"id":"five_hour","used_percent":63.0,"resets_at":1763795000},
                          {"id":"gemini-pro","label":"Gemini Pro","remaining_fraction":0.25}],
                "alert":"rate_limit","unknown_top_level":{"x":1}}"""
            val parsed = snapshot(observation("%1", "Claude", status), observation("%2", "Codex", "{}"), observation("%3", "Agy", null))
            val s = parsed.observations[0].status!!
            assertEquals("Opus 4.x", s.model?.name)
            assertEquals("high", s.effort)
            assertEquals(true, s.thinking)
            assertEquals(42.0, s.contextPercent())
            assertEquals(85.0, s.context?.compact_percent)
            assertEquals(200000L, s.context?.breakdown_window)
            assertEquals(80000.0 / 84000.0, s.cacheHitRatio()!!, 1e-9)
            assertEquals(300_000L, s.cache?.ttlMs())
            assertEquals(1763790300000L, s.cache?.estimatedExpiryMs())
            assertEquals(1.23, s.cost_usd)
            assertEquals("Messages", s.context!!.breakdown[0].label())
            assertEquals("other", s.context!!.breakdown[1].categoryKey)
            assertEquals("Plugin data", s.context!!.breakdown[1].label())
            assertEquals("Subscription", s.account?.kindLabel())
            assertEquals("5h", s.limits[0].title())
            assertEquals(63.0, s.limits[0].used())
            assertEquals(1000L, s.limits[0].resetsInS(1763794000000))
            assertEquals("Gemini Pro", s.limits[1].title())
            assertEquals(75.0, s.limits[1].used())
            assertEquals("Rate limit reached", agentAlertLabel("rate_limit"))
            assertNull(agentAlertLabel("future"))
            assertFalse(s.isStale(1763790001000))
            assertTrue(s.isStale(1763790000000 + AGENT_STATUS_STALE_MS))
            // An empty status is all unknown, never zero; a missing status is null.
            val empty = parsed.observations[1].status!!
            assertEquals(AgentStatus(), empty)
            assertNull(empty.contextPercent())
            assertNull(empty.cacheHitRatio())
            assertNull(parsed.observations[2].status)
            // Re-encoding keeps the Rust wire names the core reads back.
            val wire = json.encodeToString(AgentStatus.serializer(), s)
            listOf("\"cost_usd\"", "\"compact_percent\"", "\"breakdown_window\"", "\"last_at\"", "\"hit_ratio\"",
                "\"used_percent\"", "\"remaining_fraction\"", "\"resets_at\"").forEach { assertTrue(it in wire, it) }
            assertEquals(s, json.decodeFromString(AgentStatus.serializer(), wire))
        } finally { I18n.configure(previous.code, "") }
    }

    @Test fun statusesFilterByTargetAndLimitsFollowTheNewestAccountSample() {
        val parsed = snapshot(
            observation("%1", "Claude", """{"at":50,"limits_at":10,"account":{"key":"k"},"limits":[{"id":"five_hour","used_percent":10.0}],"context":{"used":50,"size":200}}"""),
            observation("%2", "Claude", """{"at":20,"account":{"key":"k"},"limits":[{"id":"five_hour","used_percent":30.0}]}""", window = "@2"),
            observation("%3", "Claude", """{"at":30,"account":{"key":"other"},"limits":[{"id":"five_hour","used_percent":90.0}]}"""),
            observation("%4", "Codex", """{"at":40,"limits":[{"id":"x","used_percent":5.0}]}"""),
            observation("%5", "Claude", null, window = "@2"),
        )
        assertEquals(listOf("%1", "%2", "%3", "%4"), parsed.statuses().map { it.pane })
        assertEquals(listOf("%2"), parsed.statuses("@2").map { it.pane })
        assertEquals(listOf("%3"), parsed.statuses("%3").map { it.pane })
        assertEquals(listOf("%1", "%2", "%3", "%4"), parsed.statuses("$1").map { it.pane })
        assertTrue(parsed.statuses("%5").isEmpty())
        assertEquals(30.0, parsed.freshestLimits("Claude", "k").single().used_percent)
        assertTrue(parsed.freshestLimits("Codex", "k").isEmpty())
        val o = parsed.observations
        assertEquals(30.0, parsed.limitsFor(o[0]).single().used_percent)
        assertEquals(90.0, parsed.limitsFor(o[2]).single().used_percent)
        assertEquals(5.0, parsed.limitsFor(o[3]).single().used_percent)
        assertTrue(parsed.limitsFor(o[4]).isEmpty())
        assertEquals(25.0, o[0].status!!.contextPercent())
    }

    @Test fun tokenAndDurationFormatsMatchTheCore() {
        listOf(0L to "0", 999L to "999", 1_000L to "1k", 1_250L to "1.3k", 9_949L to "9.9k", 9_950L to "10k",
            84_000L to "84k", 999_499L to "999k", 999_500L to "1M", 1_200_000L to "1.2M", 9_960_000L to "10M",
            12_400_000L to "12M").forEach { (n, text) -> assertEquals(text, formatTokens(n), "$n") }
        listOf(0L to "0s", 59L to "59s", 60L to "1m", 180L to "3m", 3_600L to "1h", 7_500L to "2h 5m", 86_400L to "1d",
            4 * 86_400L to "4d", 4 * 86_400L + 3 * 3_600 + 59 to "4d 3h").forEach { (s, text) -> assertEquals(text, formatDurationS(s), "$s") }
        assertNull(CacheUsage(ttl = "5분").ttlMs())
        assertNull(CacheUsage(ttl = "m").ttlMs())
        assertEquals(3_600_000L, CacheUsage(ttl = "1h").ttlMs())
    }
}
