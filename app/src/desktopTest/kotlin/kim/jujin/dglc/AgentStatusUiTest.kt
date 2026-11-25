package kim.jujin.dglc

import kim.jujin.dglc.core.*
import kim.jujin.dglc.ui.*
import kotlin.test.*

class AgentStatusUiTest {
    private fun observation(pane: String, agent: String = "Claude", status: AgentStatus? = null, actor: String = "parent") =
        AgentObservation("$1", "@1", pane, "run$pane", actor, agent, status = status)
    private fun <T> english(block: () -> T): T {
        val previous = I18n.language
        try { I18n.configure("en", "ko"); return block() } finally { I18n.configure(previous.code, "ko") }
    }

    @Test fun statusButtonDescriptionIncludesEnabledKnownValues() = english {
        val status = AgentStatus(at = 1, model = ModelInfo("claude-opus", "Opus"), effort = "high",
            context = ContextUsage(percent = 42.4), cache = CacheUsage(input = 10, read = 90, write = 0), cost_usd = 1.234,
            limits = listOf(UsageLimit("five_hour", used_percent = 63.0), UsageLimit("seven_day", used_percent = 20.0), UsageLimit("x")))
        val all = AgentStatusSettings(cost = true)
        val standard = "Opus · high · Context 42% · 5h 63% · Hit ratio 90%"
        assertEquals("$standard · Est. cost \$1.23", agentStatusSummary(status, status.limits, all))
        assertEquals(standard, agentStatusSummary(status, status.limits, AgentStatusSettings()))
        assertEquals("Context 42%", agentStatusSummary(status.copy(model = null, effort = null, cache = null), emptyList(), AgentStatusSettings()))
        assertEquals("", agentStatusSummary(null, emptyList(), all))
        assertEquals("Thinking", agentStatusSummary(AgentStatus(thinking = true), emptyList(), all))
        assertEquals("five_hour", agentTopLimit(status.limits)?.id)
        assertNull(agentTopLimit(listOf(UsageLimit("x"))))
    }

    @Test fun statusChipFollowsTheSelectedSessionAndPrefersItsFocusedPane() {
        val status = AgentStatus(at = 1, model = ModelInfo("model"))
        val outside = observation("%8", "Grok", status).copy(session = "$2", window = "@2")
        val snapshot = AgentSnapshot(valid = true, observations = listOf(
            outside,
            observation("%1", "Codex"), observation("%2", "Claude", status, actor = "sub"), observation("%2", "Claude", status),
            observation("%3", "Agy").copy(phase = "idle")))
        assertEquals(observation("%2", "Claude", status), agentStatusFocus(snapshot, "$1", "%2", 1))
        assertNull(agentStatusFocus(snapshot, "$1", "%1", 1))
        assertEquals("idle", agentStatusFocus(snapshot, "$1", "%3", 1)?.phase)
        assertNull(agentStatusFocus(snapshot, "$1", "%2", 10_000))
        assertNull(agentStatusFocus(snapshot, "$1", "%9", 1))
        assertEquals(outside, agentStatusFocus(snapshot, "$2", "%8", 1))
        assertNull(agentStatusFocus(snapshot, "$3", "%8", 1))
        assertNull(agentStatusFocus(snapshot, null, "%8", 1))
        assertEquals(listOf("%2", "%2", "%1", "%3"), agentStatusOrder(snapshot.inSession("$1"), "%2").map { it.pane })
        assertEquals(5, agentStatusOrder(snapshot.observations, "%2").size)
    }

    @Test fun hookSettingsExplainStatusCollectionState() {
        assertEquals("agent_status.collect.installed", agentCollectionKey(true, true, false))
        assertEquals("agent_status.collect.replaced", agentCollectionKey(true, false, true))
        assertEquals("agent_status.not_collected", agentCollectionKey(true, false, false))
        assertNull(agentCollectionKey(false, false, false))
    }

    @Test fun breakdownSharesUseTheWindowAndNeverExceedIt() {
        val context = ContextUsage(breakdown = listOf(ContextPart("messages", tokens = 30_000), ContextPart("free", tokens = 0),
            ContextPart("system_prompt", tokens = 50_000)), breakdown_total = 80_000, breakdown_window = 200_000)
        assertEquals(listOf(0.15, 0.25), agentBreakdownShares(context).map { it.second })
        val overfull = agentBreakdownShares(ContextUsage(breakdown = listOf(ContextPart("messages", tokens = 150), ContextPart("skills", tokens = 100)),
            breakdown_window = 200))
        assertEquals(listOf(0.75, 0.25), overfull.map { it.second })
        assertEquals(listOf(0.6, 0.4), agentBreakdownShares(ContextUsage(breakdown = listOf(ContextPart("a", tokens = 3), ContextPart("b", tokens = 2)))).map { it.second })
        assertEquals(emptyList(), agentBreakdownShares(ContextUsage()))
        assertEquals("1.20", agentCostText(1.2))
        assertEquals("42", agentPercentText(42.4))
    }
}
