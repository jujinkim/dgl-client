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

    @Test fun chipShowsEnabledKnownPartsAndDropsThemInOrderWhenNarrow() = english {
        val status = AgentStatus(at = 1, model = ModelInfo("claude-opus", "Opus"), effort = "high",
            context = ContextUsage(percent = 42.4), cache = CacheUsage(input = 10, read = 90, write = 0), cost_usd = 1.234,
            limits = listOf(UsageLimit("five_hour", used_percent = 63.0), UsageLimit("seven_day", used_percent = 20.0), UsageLimit("x")))
        val o = observation("%1", status = status)
        val all = AgentStatusSettings(cost = true)
        val parts = agentStatusChipParts(o, status.limits, all)
        assertEquals(listOf("model", "effort", "context", "usage", "cache", "cost"), parts.map { it.field })
        assertEquals("Opus · high · Context 42% · 5h 63% · Hit ratio 90% · Est. cost \$1.23", agentStatusChipVariants(parts).first())
        assertEquals(listOf(
            "Opus · high · Context 42% · 5h 63% · Hit ratio 90% · Est. cost \$1.23",
            "Opus · high · Context 42% · 5h 63% · Hit ratio 90%",
            "Opus · high · Context 42% · 5h 63%",
            "Opus · Context 42% · 5h 63%",
            "Opus · Context 42%",
            "Context 42%",
            ""), agentStatusChipVariants(parts))
        // Defaults hide cost; disabled and unknown items never appear, and nothing is filled with 0.
        assertEquals(listOf("model", "effort", "context", "usage", "cache"), agentStatusChipParts(o, status.limits, AgentStatusSettings()).map { it.field })
        assertEquals(listOf("Context 42%", ""), agentStatusChipVariants(agentStatusChipParts(observation("%2", status = AgentStatus(context = ContextUsage(percent = 42.0))),
            emptyList(), AgentStatusSettings(model = false))))
        assertEquals(emptyList(), agentStatusChipParts(observation("%3"), emptyList(), all))
        assertEquals("Thinking", agentStatusChipParts(observation("%4", status = AgentStatus(thinking = true)), emptyList(), all).single().text)
        assertEquals("five_hour", agentTopLimit(status.limits)?.id)
        assertNull(agentTopLimit(listOf(UsageLimit("x"))))
    }

    @Test fun focusPrefersTheFocusedPaneParentThenAnyStatus() {
        val status = AgentStatus(at = 1)
        val snapshot = AgentSnapshot(valid = true, observations = listOf(
            observation("%1", "Codex"), observation("%2", "Claude", status, actor = "sub"), observation("%2", "Claude", status),
            observation("%3", "Agy", status)))
        assertEquals(observation("%2", "Claude", status), agentStatusFocus(snapshot, "%2"))
        // A focused pane without a status falls back to the first parent observation that has one.
        assertEquals(observation("%2", "Claude", status), agentStatusFocus(snapshot, "%1"))
        assertEquals(observation("%2", "Claude", status), agentStatusFocus(snapshot, null))
        assertEquals("%3", agentStatusFocus(AgentSnapshot(valid = true, observations = listOf(observation("%1", "Codex"),
            observation("%2", "Claude", status, actor = "sub"), observation("%3", "Agy", status))), "%1")?.pane)
        assertEquals("%1", agentStatusFocus(AgentSnapshot(valid = true, observations = listOf(observation("%1", "Codex"))), "%9")?.pane)
        assertNull(agentStatusFocus(AgentSnapshot(valid = true), "%1"))
        assertEquals(listOf("%2", "%2", "%1", "%3"), agentStatusOrder(snapshot, "%2").map { it.pane })
    }

    @Test fun missingItemsExplainWhetherTheyAreUnknownUncollectedOrUnsupported() {
        val hook = AgentStatus(sources = listOf("hook"), model = ModelInfo("m"))
        val mod = AgentStatus(sources = listOf("mod"))
        assertEquals(AgentFieldState.Shown, agentFieldState("Claude", "model", hook, emptyList()))
        assertEquals(AgentFieldState.NotCollected, agentFieldState("Claude", "usage", hook, emptyList()))
        assertEquals(AgentFieldState.NotCollected, agentFieldState("Claude", "breakdown", hook, emptyList()))
        // Like desktop: no limit data is explained only for a known non-subscription account.
        assertEquals(AgentFieldState.Unknown, agentFieldState("Claude", "usage", mod, emptyList()))
        assertEquals(AgentFieldState.NoLimits, agentFieldState("Claude", "usage",
            mod.copy(account = AccountInfo(kind = "api_key")), emptyList()))
        assertEquals(AgentFieldState.Shown, agentFieldState("Claude", "usage", mod, listOf(UsageLimit("five_hour", used_percent = 1.0))))
        assertEquals(AgentFieldState.Unknown, agentFieldState("Claude", "context", mod, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Codex", "breakdown", hook, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Codex", "cost", hook, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Codex", "usage", hook, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Codex", "account", null, emptyList()))
        assertEquals(AgentFieldState.Unknown, agentFieldState("Codex", "context", hook, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Grok", "usage", hook, emptyList()))
        // Codex and Grok have nothing to install, so nothing of theirs reads as "not collected".
        assertEquals(AgentFieldState.Unknown, agentFieldState("Grok", "context", hook, emptyList()))
        assertEquals(AgentFieldState.Unknown, agentFieldState("Grok", "model", null, emptyList()))
        assertEquals("agent_status.unknown", agentNoStatusKey("Codex"))
        assertEquals("agent_status.not_collected", agentNoStatusKey("Agy"))
        assertEquals(AgentFieldState.NotCollected, agentFieldState("Agy", "account", hook, emptyList()))
        assertEquals(AgentFieldState.NotCollected, agentFieldState("Claude", "model", null, emptyList()))
        assertEquals(AgentFieldState.NotSupported, agentFieldState("Agy", "breakdown", null, emptyList()))
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
