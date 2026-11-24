//! Agent status HUD: the status-bar chip, its popover, the sidebar panel and the
//! settings card. Values the helper did not report stay unknown and are never shown as 0.
use super::*;
use dgl_core::agent::{
    self, Agent, AgentStatus, AgentStatusSettings, Observation, Snapshot, UsageLimit,
};

const POPOVER_WIDTH: f32 = 400.;

/// HUD state that is not a preference. It stays on this device for this run only.
#[derive(Default)]
pub(crate) struct Hud {
    /// Where the status-bar chip and the sidebar panel last painted, to place the popover.
    pub chip: Rc<RefCell<Bounds<Pixels>>>,
    pub panel: Rc<RefCell<Bounds<Pixels>>>,
    /// The open popover belongs to the sidebar panel rather than the chip.
    pub from_sidebar: bool,
    /// The sidebar panel is folded.
    pub collapsed: bool,
    /// "Include detailed status collection" in the integration dialog; `None` follows inspect.
    pub collect: Option<bool>,
    /// Supplies the selected session and a server-wide poll in UI tests.
    #[cfg(test)]
    pub snapshot: Option<(String, Snapshot)>,
}

/// The active pane's parent CLI, if this session has observed one.
fn focused<'a>(
    snapshot: &'a Snapshot,
    session: Option<&str>,
    pane: Option<&str>,
) -> Option<&'a Observation> {
    let session = session?;
    snapshot
        .observations
        .iter()
        .filter(|o| o.session == session && Some(o.pane.as_str()) == pane)
        .min_by_key(|o| (o.actor != "parent", o.status.is_none()))
}

fn percent(value: f64) -> String {
    format!("{value:.0}")
}

/// The most used limit, the one worth a glance.
pub(crate) fn top_limit(limits: &[UsageLimit]) -> Option<(&UsageLimit, f64)> {
    limits
        .iter()
        .filter_map(|l| Some((l, l.used()?)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Known, enabled status values for the icon's tooltip.
fn status_summary(
    status: &AgentStatus,
    limits: &[UsageLimit],
    settings: &AgentStatusSettings,
) -> String {
    let mut parts = Vec::new();
    if settings.model
        && let Some(model) = status
            .model
            .as_ref()
            .and_then(|m| m.name.as_deref().or(m.id.as_deref()))
            .filter(|m| !m.trim().is_empty())
    {
        parts.push(model.to_owned());
    }
    if settings.effort
        && let Some(effort) = status.effort.as_deref().filter(|e| !e.trim().is_empty())
    {
        parts.push(effort.to_owned());
    }
    if settings.context
        && let Some(value) = status.context_percent()
    {
        parts.push(format!("ctx {}%", percent(value)));
    }
    if settings.usage
        && let Some((limit, used)) = top_limit(limits)
    {
        parts.push(format!("{} {}%", limit.title(), percent(used)));
    }
    if settings.cache
        && let Some(ratio) = status.cache_hit_ratio()
    {
        parts.push(tr!("agent_status.cache.hit_ratio", percent(ratio * 100.)));
    }
    if settings.cost
        && let Some(cost) = status.cost_usd
    {
        parts.push(tr!("agent_status.cost", format!("{cost:.2}")));
    }
    parts.join(" · ")
}

/// A value the helper reports, or why it is missing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Field {
    Account,
    Usage,
    Context,
    Breakdown,
    Cache,
    Cost,
}
/// Why a field has no value: the CLI has no source for it, the opt-in collector is missing,
/// or the value is simply unknown right now.
pub(crate) fn missing(agent: Agent, status: Option<&AgentStatus>, field: Field) -> &'static str {
    let sources = status.map_or(&[][..], |s| s.sources.as_slice());
    let rich = |source: &str| sources.iter().any(|s| s == source);
    match (agent, field) {
        (Agent::Codex | Agent::Grok, Field::Account) => t("agent_status.not_supported"),
        (Agent::Codex | Agent::Grok, Field::Usage) if !rich("command") => {
            t("agent_status.not_collected")
        }
        (Agent::Codex | Agent::Grok | Agent::Agy, Field::Breakdown) => {
            t("agent_status.no_breakdown")
        }
        (Agent::Codex, Field::Cost) => t("agent_status.not_supported"),
        (Agent::Claude, _) if !rich("mod") => t("agent_status.not_collected"),
        (Agent::Agy, _) if !rich("statusline") => t("agent_status.not_collected"),
        (Agent::Claude | Agent::Agy, Field::Usage)
            if status
                .and_then(|s| s.account.as_ref())
                .is_some_and(|a| a.kind.as_deref() != Some("subscription")) =>
        {
            t("agent_status.limit.none")
        }
        _ => t("agent_status.unknown"),
    }
}

/// Breakdown colours by category; `free` is the bar's empty track.
fn category_color(category: &str, palette: &theme::Palette) -> u32 {
    let base = match category {
        "system_prompt" => 0x8e6bd8,
        "system_tools" => 0x3a8fd9,
        "mcp_tools" => 0x1fa59a,
        "custom_agents" => 0xd9822b,
        "memory_files" => 0xc9a227,
        "skills" => 0x5aa846,
        "messages" => 0xd2508a,
        "autocompact_buffer" => 0x8a8f99,
        "deferred" => 0x6d7fb0,
        "free" => return palette.line,
        _ => 0xa86b4f,
    };
    dgl_core::accent::readable(base, &[palette.panel], 3.)
}
fn warning_color(palette: &theme::Palette) -> u32 {
    dgl_core::accent::readable(0xd64545, &[palette.panel], 4.5)
}

impl Desktop {
    /// The active tab's connection state, the HUD's single source.
    pub(crate) fn hud_state(&self) -> Option<Arc<dgl_core::connection::State>> {
        let state = self.active_tab()?.connection.state();
        #[cfg(test)]
        if let Some((session, snapshot)) = &self.agents.hud.snapshot {
            let mut state = (*state).clone();
            state.session = Some(session.clone());
            state.pane = Some("%1".into());
            state.agents = snapshot.clone();
            return Some(Arc::new(state));
        }
        Some(state)
    }
    fn toggle_agent_status(&mut self, from_sidebar: bool) {
        let open = matches!(self.dialog, Some(Dialog::AgentStatus))
            && self.agents.hud.from_sidebar == from_sidebar;
        self.agents.hud.from_sidebar = from_sidebar;
        self.dialog = (!open).then_some(Dialog::AgentStatus);
    }
    fn gauge(&self, fraction: f64, color: u32, width: Option<f32>) -> Div {
        div()
            .h(px(6.))
            .when_some(width, |d, w| d.w(px(w)).flex_shrink_0())
            .when(width.is_none(), |d| d.w_full())
            .rounded_full()
            .overflow_hidden()
            .bg(rgb(self.palette.line))
            .child(
                div()
                    .h_full()
                    .w(relative(fraction.clamp(0., 1.) as f32))
                    .rounded_full()
                    .bg(rgb(color)),
            )
    }
    fn limit_color(&self, used: f64) -> u32 {
        if used >= 90. {
            warning_color(&self.palette)
        } else {
            self.palette.accent
        }
    }
    fn agent_icon(&self, agent: Agent, size: f32) -> Svg {
        svg()
            .path(agent.icon())
            .size(px(size))
            .flex_shrink_0()
            .text_color(rgb(self.palette.fg))
    }

    /// The status-bar chip for the focused pane's agent, `None` when nothing is shown.
    pub(crate) fn agent_status_chip(
        &self,
        wide: bool,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let settings = &self.local.preferences.agent_status;
        if !settings.status_bar {
            return None;
        }
        let state = self.hud_state()?;
        let now = agent::now_ms();
        if !state.agents.has_fresh_poll(now) {
            return None;
        }
        let observation = focused(
            &state.agents,
            state.session.as_deref(),
            state.pane.as_deref(),
        )?;
        let status = observation.status.as_ref();
        let summary = status.map_or_else(String::new, |s| {
            status_summary(s, state.agents.limits_for(observation), settings)
        });
        let dim = status.is_some_and(|s| s.is_stale(now));
        let alert = status
            .and_then(|s| s.alert.as_deref())
            .and_then(agent::alert_label);
        let p = self.palette;
        let icon_color = if alert.is_some() {
            warning_color(&p)
        } else if dim {
            p.muted
        } else {
            p.fg
        };
        let phase = state.agents.phase_of(observation, now).label();
        let mut chip = vec![phase.to_owned()];
        if wide && state.agents.current(now) {
            if settings.context
                && let Some(s) = status
                && s.context_at
                    .or(s.at)
                    .is_some_and(|at| now.saturating_sub(at) < agent::STATUS_STALE_MS)
                && let Some(value) = s.context_percent()
            {
                chip.push(format!("ctx {}%", percent(value)));
            }
            if settings.usage
                && let Some(s) = state.agents.limit_sample_for(observation)
                && now.saturating_sub(s.limits_sampled_at()) < agent::STATUS_STALE_MS
                && let Some((_, used)) = top_limit(&s.limits)
            {
                chip.push(tr!(
                    "agent_status.remaining",
                    percent((100. - used).max(0.))
                ));
            }
        }
        let tip = [
            observation.agent.label().to_owned(),
            phase.to_owned(),
            summary,
            alert.unwrap_or_default().to_owned(),
            t("agent_status.open_details").to_owned(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
        Some(
            self.button_base("agent-status-chip", cx, |s, _, _| {
                s.toggle_agent_status(false)
            })
            // Preserve the prior focus until the modal records it.
            .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
            .child(
                canvas(
                    {
                        let anchor = self.agents.hud.chip.clone();
                        move |bounds, _, _| *anchor.borrow_mut() = bounds.dilate(px(1.))
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .relative()
            .h(px(24.))
            .px_1()
            .gap_1()
            .text_xs()
            .flex()
            .items_center()
            .justify_center()
            .native_tooltip(tip)
            .child(
                self.agent_icon(observation.agent, 16.)
                    .text_color(rgb(icon_color)),
            )
            .child(chip.join(" · ")),
        )
    }

    /// The popover listing every agent of the active connection.
    pub(super) fn render_agent_status_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let panel = div()
            .id("agent-status-popover")
            .debug_selector(|| "agent-status-popover".into())
            .w(px(
                POPOVER_WIDTH.min((f32::from(viewport.width) - 16.).max(0.))
            ))
            .max_h(px((f32::from(viewport.height) - 60.).max(0.)))
            .flex()
            .flex_col()
            .min_h_0()
            .p_3()
            .gap_2()
            .bg(rgb(self.palette.panel))
            .border_1()
            .border_color(rgb(self.palette.line))
            .rounded_lg()
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, w, cx| {
                w.prevent_default();
                cx.stop_propagation();
            })
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .child(div().text_lg().flex_1().child(t("agent_status.title")))
                    .child(
                        self.button("close-agent-status", t("common.close"), cx, |s, _, cx| {
                            s.dismiss_dialog(cx)
                        }),
                    ),
            )
            .child(
                div()
                    .id("agent-status-list")
                    .debug_selector(|| "agent-status-list".into())
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.modal.scroll)
                    .child(self.agent_status_details()),
            );
        let anchor = if self.agents.hud.from_sidebar {
            self.agents.hud.panel.clone()
        } else {
            self.agents.hud.chip.clone()
        };
        self.popover_overlay("agent-status-overlay", panel.into_any_element(), anchor, cx)
    }

    fn muted_line(&self, text: impl Into<SharedString>) -> Div {
        div()
            .text_xs()
            .text_color(rgb(self.palette.muted))
            .child(text.into())
    }
    fn field_line(&self, label: &str, value: impl Into<SharedString>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .text_sm()
            .child(
                div()
                    .text_color(rgb(self.palette.muted))
                    .child(format!("{label}:")),
            )
            .child(value.into())
    }
    fn section(&self, title: &str) -> Div {
        div().flex().flex_col().gap_1().pt_1().child(
            div()
                .text_sm()
                .text_color(rgb(self.palette.accent))
                .child(title.to_owned()),
        )
    }

    /// The chip opens this session's details. The sidebar opens the full host list.
    fn agent_status_details(&self) -> Div {
        let body = div().flex().flex_col().gap_3();
        let Some(state) = self.hud_state() else {
            return body.child(self.muted_line(t("agent_status.none")));
        };
        let now = agent::now_ms();
        let snapshot = &state.agents;
        if !snapshot.valid {
            return body.child(self.muted_line(t("agent_status.unknown")));
        }
        if !snapshot.has_fresh_poll(now) {
            return body.child(self.muted_line(t("agent_status.stale")));
        }
        let mut observations = snapshot
            .observations
            .iter()
            .filter(|o| {
                self.agents.hud.from_sidebar || state.session.as_deref() == Some(o.session.as_str())
            })
            .peekable();
        if observations.peek().is_none() {
            return body.child(self.muted_line(t("agent_status.none")));
        }
        observations.fold(body, |body, o| {
            body.child(self.observation_status(&state, o, now))
        })
    }

    fn observation_status(
        &self,
        state: &dgl_core::connection::State,
        o: &Observation,
        now: u64,
    ) -> Stateful<Div> {
        let p = self.palette;
        let settings = &self.local.preferences.agent_status;
        let status = o.status.as_ref();
        let mut block = div()
            .id(SharedString::from(format!(
                "agent-status-{}-{}",
                o.run, o.actor
            )))
            .flex()
            .flex_col()
            .gap_1()
            .pb_2()
            .border_b_1()
            .border_color(rgb(p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.agent_icon(o.agent, 15.))
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "{} · {}{}",
                        o.agent.label(),
                        hierarchy::target_label(&state.remote, Some(&o.session), Some(&o.pane)),
                        if o.actor != "parent" {
                            t("agent.subagent_suffix")
                        } else {
                            ""
                        }
                    ))),
            );
        block = block.child(self.muted_line(state.agents.phase_of(o, now).label()));
        let Some(status) = status else {
            let text = match o.agent {
                Agent::Claude | Agent::Agy => t("agent_status.not_collected"),
                Agent::Codex | Agent::Grok => t("agent_status.unknown"),
            };
            return block.child(self.muted_line(text));
        };
        if let Some(at) = status.at {
            block = block.child(self.muted_line(tr!(
                "agent_status.observed_ago",
                agent::format_duration_s(now.saturating_sub(at) / 1000)
            )));
        }
        if status.is_stale(now) {
            block = block.child(
                div()
                    .text_xs()
                    .text_color(rgb(warning_color(&p)))
                    .child(t("agent_status.stale")),
            );
        }
        if let Some(alert) = status.alert.as_deref().and_then(agent::alert_label) {
            block = block.child(
                div()
                    .debug_selector(|| "agent-status-alert".into())
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .text_color(rgb(warning_color(&p)))
                    .child(UiIcon::Error.render(warning_color(&p)))
                    .child(alert),
            );
        }
        if settings.model {
            let model = status
                .model
                .as_ref()
                .and_then(|m| m.name.as_deref().or(m.id.as_deref()))
                .unwrap_or(t("agent_status.unknown"));
            let thinking = if status.thinking == Some(true) {
                format!(" · {}", t("agent_status.thinking"))
            } else {
                String::new()
            };
            block = block.child(
                self.field_line(t("agent_status.field.model"), format!("{model}{thinking}")),
            );
        }
        if settings.effort {
            block = block.child(
                self.field_line(
                    t("agent_status.field.effort"),
                    status
                        .effort
                        .clone()
                        .unwrap_or_else(|| t("agent_status.unknown").into()),
                ),
            );
        }
        if settings.account {
            let value = status
                .account
                .as_ref()
                .and_then(|a| {
                    let kind = a.kind_label();
                    let parts: Vec<&str> = kind
                        .into_iter()
                        .chain(a.plan.as_deref().filter(|p| !p.trim().is_empty()))
                        .collect();
                    (!parts.is_empty()).then(|| parts.join(" · "))
                })
                .unwrap_or_else(|| missing(o.agent, Some(status), Field::Account).into());
            block = block.child(self.field_line(t("agent_status.field.account"), value));
        }
        if settings.usage {
            block = block.child(self.usage_section(state, o, status, now));
        }
        // The breakdown is its own setting: it shows even with the context figures off.
        if settings.context || settings.breakdown {
            block =
                block.child(self.context_section(o, status, settings.context, settings.breakdown));
        }
        if settings.cache {
            block = block.child(self.cache_section(o, status, now));
        }
        if settings.cost {
            block = block.child(self.field_line(
                t("agent_status.field.cost"),
                status.cost_usd.map_or_else(
                    || missing(o.agent, Some(status), Field::Cost).into(),
                    |c| tr!("agent_status.cost", format!("{c:.2}")),
                ),
            ));
        }
        block
    }

    fn usage_section(
        &self,
        state: &dgl_core::connection::State,
        o: &Observation,
        status: &AgentStatus,
        now: u64,
    ) -> Div {
        let mut section = self.section(t("agent_status.field.usage"));
        let limits = state.agents.limits_for(o);
        if limits.is_empty() {
            let reason =
                if state.agents.account_limits && matches!(o.agent, Agent::Codex | Agent::Grok) {
                    t("agent_status.unknown")
                } else {
                    missing(o.agent, Some(status), Field::Usage)
                };
            return section.child(self.muted_line(reason));
        }
        for limit in limits {
            let used = limit.used();
            let id = limit.id.clone();
            section = section.child(
                div()
                    .debug_selector(move || format!("agent-status-limit-{id}"))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .text_sm()
                            .child(div().flex_1().min_w_0().truncate().child(limit.title()))
                            .child(used.map_or_else(
                                || t("agent_status.unknown").to_owned(),
                                |u| tr!("agent_status.limit.used", percent(u)),
                            ))
                            .when_some(limit.resets_in_s(now), |d, secs| {
                                d.child(div().text_color(rgb(self.palette.muted)).child(tr!(
                                    "agent_status.limit.resets_in",
                                    agent::format_duration_s(secs)
                                )))
                            }),
                    )
                    .when_some(used, |d, used| {
                        d.child(self.gauge(used / 100., self.limit_color(used), None))
                    }),
            );
        }
        section
    }

    fn context_section(
        &self,
        o: &Observation,
        status: &AgentStatus,
        figures: bool,
        breakdown: bool,
    ) -> Div {
        let p = self.palette;
        let mut section = self.section(t(if figures {
            "agent_status.field.context"
        } else {
            "agent_status.field.breakdown"
        }));
        let context = status.context.as_ref();
        match status.context_percent().filter(|_| figures) {
            Some(value) => {
                section = section
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .text_sm()
                            .child(format!("{}%", percent(value)))
                            .when_some(
                                context.and_then(|c| Some((c.used?, c.size?))),
                                |d, (used, size)| {
                                    d.child(tr!(
                                        "agent_status.context.tokens",
                                        agent::format_tokens(used),
                                        agent::format_tokens(size)
                                    ))
                                },
                            ),
                    )
                    .child(
                        self.gauge(value / 100., p.accent, None)
                            .debug_selector(|| "agent-status-context-gauge".into()),
                    );
            }
            None if figures => {
                section =
                    section.child(self.muted_line(missing(o.agent, Some(status), Field::Context)));
            }
            None => {}
        }
        if let Some(compact) = context.and_then(|c| c.compact_percent).filter(|_| figures) {
            section = section
                .child(self.muted_line(tr!("agent_status.context.compact_at", percent(compact))));
        }
        if !breakdown {
            return section;
        }
        let parts = context.map_or(&[][..], |c| c.breakdown.as_slice());
        if parts.is_empty() {
            return section.child(self.muted_line(missing(
                o.agent,
                Some(status),
                Field::Breakdown,
            )));
        }
        let window = context
            .and_then(|c| c.breakdown_window.or(c.size))
            .filter(|w| *w > 0)
            .unwrap_or_else(|| parts.iter().filter_map(|p| p.tokens).sum::<u64>().max(1));
        let mut bar = div()
            .debug_selector(|| "agent-status-breakdown".into())
            .flex()
            .w_full()
            .h(px(8.))
            .rounded_sm()
            .overflow_hidden()
            .bg(rgb(p.line));
        let mut legend = div().flex().flex_col().gap_1().text_xs();
        for part in parts {
            let key = part.category_key();
            let color = category_color(key, &p);
            let share = part.tokens.map(|n| n as f64 / window as f64);
            if key != "free"
                && let Some(share) = share
            {
                bar = bar.child(
                    div()
                        .h_full()
                        .flex_shrink_0()
                        .w(relative(share.clamp(0., 1.) as f32))
                        .bg(rgb(color)),
                );
            }
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .size(px(8.))
                            .flex_shrink_0()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(p.control_border))
                            .bg(rgb(color)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(part.label()))
                    .child(part.tokens.map_or_else(
                        || t("agent_status.unknown").to_owned(),
                        |n| {
                            format!(
                                "{} · {}%",
                                agent::format_tokens(n),
                                percent(share.unwrap_or(0.) * 100.)
                            )
                        },
                    )),
            );
        }
        if figures {
            section = section.child(self.muted_line(t("agent_status.field.breakdown")));
        }
        section.child(bar).child(legend)
    }

    fn cache_section(&self, o: &Observation, status: &AgentStatus, now: u64) -> Div {
        let mut section = self
            .section(t("agent_status.field.cache"))
            .child(self.muted_line(t("agent_status.cache.server_side")));
        let Some(cache) = status.cache.as_ref() else {
            return section.child(self.muted_line(missing(o.agent, Some(status), Field::Cache)));
        };
        let counts: Vec<String> = [
            ("agent_status.cache.input", cache.input),
            ("agent_status.cache.read", cache.read),
            ("agent_status.cache.write", cache.write),
            ("agent_status.cache.output", cache.output),
        ]
        .into_iter()
        .filter_map(|(key, n)| Some(format!("{} {}", t(key), agent::format_tokens(n?))))
        .collect();
        if !counts.is_empty() {
            section = section.child(
                div()
                    .debug_selector(|| "agent-status-cache".into())
                    .text_sm()
                    .child(counts.join(" · ")),
            );
        }
        if let Some(ratio) = status.cache_hit_ratio() {
            section = section.child(
                div()
                    .text_sm()
                    .child(tr!("agent_status.cache.hit_ratio", percent(ratio * 100.))),
            );
        }
        if let Some(at) = cache.last_at {
            section = section.child(self.muted_line(tr!(
                "agent_status.cache.last_request",
                agent::format_duration_s(now.saturating_sub(at) / 1000)
            )));
        }
        if let (Some(ttl), Some(expiry)) = (cache.ttl.as_deref(), cache.estimated_expiry_ms()) {
            section = section.child(self.muted_line(if expiry > now {
                tr!(
                    "agent_status.cache.expires_in",
                    ttl,
                    agent::format_duration_s((expiry - now).div_ceil(1000))
                )
            } else {
                tr!("agent_status.cache.expired", ttl)
            }));
        }
        section
    }

    /// The collapsible panel under the host list: one line per agent with mini gauges.
    pub(crate) fn agent_status_sidebar(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.local.preferences.agent_status.sidebar {
            return None;
        }
        let p = self.palette;
        let collapsed = self.agents.hud.collapsed;
        let header = div()
            .relative()
            .flex()
            .items_center()
            .gap_1()
            .px_1()
            .child(
                canvas(
                    {
                        let anchor = self.agents.hud.panel.clone();
                        move |bounds, _, _| *anchor.borrow_mut() = bounds
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                self.button_base("agent-status-toggle", cx, |s, _, _| {
                    s.agents.hud.collapsed = !s.agents.hud.collapsed;
                })
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap_1()
                .text_sm()
                .text_color(rgb(p.muted))
                .child(
                    if collapsed {
                        UiIcon::ChevronRight
                    } else {
                        UiIcon::ChevronDown
                    }
                    .render(p.muted),
                )
                .child(div().min_w_0().truncate().child(t("agent_status.title"))),
            )
            .child(
                self.button("agent-status-open", UiIcon::Info, cx, |s, _, _| {
                    s.toggle_agent_status(true)
                })
                .native_tooltip(t("agent_status.open_details")),
            );
        let mut panel = div()
            .debug_selector(|| "agent-status-panel".into())
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(p.line))
            .py_1()
            .child(header);
        if collapsed {
            return Some(panel);
        }
        let mut list = div()
            .id("agent-status-panel-list")
            .max_h(px(160.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px_2()
            .pb_1()
            .text_xs();
        let now = agent::now_ms();
        let state = self.hud_state();
        let observations = state
            .as_ref()
            .filter(|s| s.agents.has_fresh_poll(now))
            .map_or(&[][..], |s| s.agents.observations.as_slice());
        if observations.is_empty() {
            list = list.child(
                div().py_1().text_color(rgb(p.muted)).child(
                    if state
                        .as_ref()
                        .is_some_and(|s| s.agents.valid && !s.agents.has_fresh_poll(now))
                    {
                        t("agent_status.stale")
                    } else {
                        t("agent_status.none")
                    },
                ),
            );
        }
        let settings = &self.local.preferences.agent_status;
        for o in observations {
            let status = o.status.as_ref();
            let model = status
                .filter(|_| settings.model)
                .and_then(|s| s.model.as_ref())
                .and_then(|m| m.name.as_deref().or(m.id.as_deref()));
            let context = status
                .filter(|_| settings.context)
                .and_then(AgentStatus::context_percent);
            let limit = state
                .as_ref()
                .filter(|_| settings.usage)
                .and_then(|s| top_limit(s.agents.limits_for(o)).map(|(l, u)| (l.title(), u)));
            let dim = status.is_none_or(|s| s.is_stale(now));
            let phase = state.as_ref().unwrap().agents.phase_of(o, now).label();
            let mut row = self
                .button_base(
                    format!("agent-status-row-{}-{}", o.run, o.actor),
                    cx,
                    |s, _, _| s.toggle_agent_status(true),
                )
                .flex()
                .items_center()
                .gap_1()
                .px_1()
                .text_color(rgb(if dim { p.muted } else { p.fg }))
                .child(self.agent_icon(o.agent, 13.))
                .child(div().flex_1().min_w_0().truncate().child(match model {
                    Some(model) => format!("{} · {phase} · {model}", o.agent.label()),
                    None => format!("{} · {phase}", o.agent.label()),
                }));
            if let Some(value) = context {
                row = row
                    .child(self.gauge(value / 100., p.accent, Some(28.)))
                    .child(div().flex_shrink_0().child(format!("{}%", percent(value))));
            }
            if let Some((title, used)) = limit {
                row = row
                    .child(
                        div()
                            .flex_shrink_0()
                            .max_w(px(48.))
                            .truncate()
                            .text_color(rgb(p.muted))
                            .child(title),
                    )
                    .child(self.gauge(used / 100., self.limit_color(used), Some(28.)))
                    .child(div().flex_shrink_0().child(format!("{}%", percent(used))));
            }
            if status.is_none() {
                row = row.child(
                    div()
                        .flex_shrink_0()
                        .text_color(rgb(p.muted))
                        .child(t("agent_status.unknown")),
                );
            }
            list = list.child(row);
        }
        panel = panel.child(list);
        Some(panel)
    }

    /// The settings card body: where the HUD shows and which values it lists.
    pub(crate) fn agent_status_settings(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.local.preferences.agent_status;
        let mut body = div()
            .flex()
            .flex_col()
            .child(self.muted_line(t("agent_status.description")));
        let check = |on: bool| {
            if on {
                UiIcon::CheckboxChecked
            } else {
                UiIcon::Checkbox
            }
        };
        let rows = [
            (0, t("agent_status.show_status_bar"), s.status_bar),
            (1, t("agent_status.show_sidebar"), s.sidebar),
            (2, t("agent_status.field.account"), s.account),
            (3, t("agent_status.field.usage"), s.usage),
            (4, t("agent_status.field.context"), s.context),
            (5, t("agent_status.field.breakdown"), s.breakdown),
            (6, t("agent_status.field.cache"), s.cache),
            (7, t("agent_status.field.model"), s.model),
            (8, t("agent_status.field.effort"), s.effort),
            (9, t("agent_status.field.cost"), s.cost),
        ];
        for (i, label, enabled) in rows {
            if i == 2 {
                body = body.child(div().pt_2().child(t("agent_status.fields")));
            }
            body = body.child(self.button(
                format!("agent-status-setting-{i}"),
                (check(enabled), label),
                cx,
                move |s, _, _| {
                    let settings = &mut s.local.preferences.agent_status;
                    let field = match i {
                        0 => &mut settings.status_bar,
                        1 => &mut settings.sidebar,
                        2 => &mut settings.account,
                        3 => &mut settings.usage,
                        4 => &mut settings.context,
                        5 => &mut settings.breakdown,
                        6 => &mut settings.cache,
                        7 => &mut settings.model,
                        8 => &mut settings.effort,
                        _ => &mut settings.cost,
                    };
                    *field = !*field;
                    s.save();
                },
            ));
        }
        body
    }

    /// The integration dialog's choice to install the detailed status collectors.
    pub(crate) fn collect_choice(&self, preview: &serde_json::Value) -> bool {
        self.agents
            .hud
            .collect
            .unwrap_or_else(|| installed_collection(preview))
    }
}

/// Whether inspect found a status collector of ours already installed.
pub(crate) fn installed_collection(preview: &serde_json::Value) -> bool {
    preview["account_limits"].as_bool() == Some(true)
        || preview["configs"].as_array().is_some_and(|configs| {
            configs
                .iter()
                .any(|c| c["status_collection"]["installed"].as_bool() == Some(true))
        })
}

/// A CLI's detailed collection state from inspect, `None` when it has nothing to install.
pub(crate) fn collection_state(config: &serde_json::Value) -> Option<String> {
    let state = &config["status_collection"];
    if state["supported"].as_bool() != Some(true) {
        return None;
    }
    Some(if state["replaced"].as_bool() == Some(true) {
        t("agent_status.collect.replaced").to_owned()
    } else if state["installed"].as_bool() == Some(true) {
        format!(
            "{} · {}",
            t("agent_status.collect.installed"),
            t("agent_status.collect.restart")
        )
    } else {
        t("agent_status.not_collected").to_owned()
    })
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's `test` attribute would replace the standard one.
    use super::{
        Agent, AgentStatus, AgentStatusSettings, Field, Observation, Snapshot, UsageLimit,
        collection_state, focused, installed_collection, missing, status_summary, t,
    };
    use dgl_core::agent::{AccountInfo, CacheUsage, ContextUsage, ModelInfo};
    use dgl_core::tr;

    fn observation(pane: &str, actor: &str, status: Option<AgentStatus>) -> Observation {
        Observation {
            session: "$1".into(),
            window: "@1".into(),
            pane: pane.into(),
            run: format!("run{pane}{actor}"),
            actor: actor.into(),
            agent: Agent::Claude,
            last: None,
            phase: Default::default(),
            phase_seq: 0,
            supported: vec![],
            status,
        }
    }
    fn full_status() -> AgentStatus {
        AgentStatus {
            at: Some(1_000),
            sources: vec!["mod".into()],
            model: Some(ModelInfo {
                id: Some("claude-opus".into()),
                name: Some("Opus".into()),
            }),
            effort: Some("high".into()),
            context: Some(ContextUsage {
                percent: Some(42.),
                ..Default::default()
            }),
            cache: Some(CacheUsage {
                hit_ratio: Some(0.96),
                ..Default::default()
            }),
            cost_usd: Some(1.5),
            account: Some(AccountInfo {
                kind: Some("subscription".into()),
                ..Default::default()
            }),
            limits: vec![
                UsageLimit {
                    id: "five_hour".into(),
                    used_percent: Some(63.),
                    ..Default::default()
                },
                UsageLimit {
                    id: "seven_day".into(),
                    used_percent: Some(20.),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn icon_tooltip_includes_enabled_known_values() {
        let status = full_status();
        let mut settings = AgentStatusSettings {
            cost: true,
            ..Default::default()
        };
        let summary = status_summary(&status, &status.limits, &settings);
        assert_eq!(
            summary,
            format!(
                "Opus · high · ctx 42% · {} 63% · {} · {}",
                status.limits[0].title(),
                tr!("agent_status.cache.hit_ratio", "96"),
                tr!("agent_status.cost", "1.50")
            )
        );
        settings.model = false;
        settings.cost = false;
        let unknown = AgentStatus {
            effort: None,
            context: None,
            ..status.clone()
        };
        assert_eq!(
            status_summary(&unknown, &[], &settings),
            tr!("agent_status.cache.hit_ratio", "96")
        );
        assert!(status_summary(&AgentStatus::default(), &[], &settings).is_empty());
    }

    #[test]
    fn chip_follows_the_selected_session_and_prefers_its_focused_pane() {
        let snapshot = Snapshot {
            valid: true,
            observed_at: 1_000,
            observations: vec![
                Observation {
                    session: "$2".into(),
                    window: "@2".into(),
                    ..observation("%8", "parent", Some(full_status()))
                },
                observation("%1", "parent", None),
                observation("%2", "sub", Some(full_status())),
                observation("%2", "parent", Some(full_status())),
            ],
            ..Default::default()
        };
        let focus = |session, pane| focused(&snapshot, session, pane);
        assert_eq!(focus(Some("$1"), Some("%2")).unwrap().actor, "parent");
        // The pane's own agent wins even without a status.
        assert_eq!(focus(Some("$1"), Some("%1")).unwrap().pane, "%1");
        assert!(focus(Some("$1"), Some("%9")).is_none());
        assert_eq!(focus(Some("$2"), Some("%8")).unwrap().pane, "%8");
        assert!(focus(Some("$3"), Some("%8")).is_none());
        assert!(focus(None, Some("%8")).is_none());
        assert!(snapshot.has_fresh_poll(10_999));
        assert!(!snapshot.has_fresh_poll(11_000));
        assert!(
            !Snapshot {
                valid: false,
                ..snapshot
            }
            .has_fresh_poll(1_000)
        );
    }

    #[test]
    fn missing_reasons_are_explicit() {
        let hooks_only = AgentStatus {
            sources: vec!["hook".into()],
            ..Default::default()
        };
        assert_eq!(
            missing(Agent::Claude, Some(&hooks_only), Field::Usage),
            t("agent_status.not_collected")
        );
        assert_eq!(
            missing(Agent::Agy, Some(&hooks_only), Field::Breakdown),
            t("agent_status.no_breakdown")
        );
        assert_eq!(
            missing(Agent::Codex, Some(&hooks_only), Field::Usage),
            t("agent_status.not_collected")
        );
        assert_eq!(
            missing(Agent::Codex, Some(&hooks_only), Field::Cost),
            t("agent_status.not_supported")
        );
        assert_eq!(
            missing(Agent::Grok, Some(&hooks_only), Field::Cost),
            t("agent_status.unknown")
        );
        let api = AgentStatus {
            sources: vec!["mod".into()],
            account: Some(AccountInfo {
                kind: Some("api_key".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            missing(Agent::Claude, Some(&api), Field::Usage),
            t("agent_status.limit.none")
        );
        assert_eq!(
            missing(Agent::Claude, Some(&api), Field::Breakdown),
            t("agent_status.unknown")
        );
    }

    #[test]
    fn integration_dialog_reads_status_collection_from_inspect() {
        let preview = serde_json::json!({"configs": [
            {"agent": "Codex", "status_collection": {"supported": false, "installed": false, "replaced": false, "kind": "hooks"}},
            {"agent": "Claude", "status_collection": {"supported": true, "installed": true, "replaced": false, "kind": "mod"}},
            {"agent": "Agy", "status_collection": {"supported": true, "installed": false, "replaced": true, "kind": "statusline"}},
            {"agent": "Grok"}
        ]});
        assert!(installed_collection(&preview));
        let configs = preview["configs"].as_array().unwrap();
        assert_eq!(collection_state(&configs[0]), None);
        assert!(
            collection_state(&configs[1])
                .unwrap()
                .contains(t("agent_status.collect.restart"))
        );
        assert_eq!(
            collection_state(&configs[2]).as_deref(),
            Some(t("agent_status.collect.replaced"))
        );
        assert_eq!(collection_state(&configs[3]), None);
        assert!(!installed_collection(&serde_json::json!({})));
    }
}
