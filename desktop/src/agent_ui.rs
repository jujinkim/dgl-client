use super::*;
use dgl_core::agent::{self, Agent};

/// Agent notices collected from every connection and the notification button's state.
#[derive(Default)]
pub(crate) struct AgentState {
    pub inbox: agent::Inbox,
    pub tick: u64,
    pub status: String,
    pub anchor: Rc<RefCell<Bounds<Pixels>>>,
    pub hud: crate::agent_status_ui::Hud,
    #[cfg(debug_assertions)]
    pub probe_at: Option<u64>,
}

fn endpoint(host: &HostProfile) -> String {
    format!("{}@{}", host.username, host.endpoint())
}

fn observation_label(agent: Agent, phase: agent::LivePhase) -> String {
    phase.known_label().map_or_else(
        || agent.label().into(),
        |label| format!("{} · {label}", agent.label()),
    )
}
impl Desktop {
    pub fn tick_agents(&mut self, cx: &mut Context<Self>) {
        let now = agent::now_ms();
        if now.saturating_sub(self.agents.tick) < 250 {
            return;
        }
        self.agents.tick = now;
        let active = cx.windows().into_iter().any(|h| {
            cx.update_window(h, |_, w, _| w.is_window_active())
                .unwrap_or(false)
        });
        let mut visible = HashSet::new();
        let old_count = self.agents.inbox.recent.len();
        let old_unread = self.agents.inbox.recent.iter().filter(|n| !n.read).count();
        let sources: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| (endpoint(&tab.host), tab.connection.state()))
            .collect();
        for (tab, (key, state)) in self.tabs.iter().zip(&sources) {
            if state.agents.current(now) && state.phase == Phase::Ready {
                if self.dialog.is_none()
                    && self.snackbar.is_none()
                    && self.workspace.title_updates_enabled(tab.id, active)
                {
                    for pane in state.render_targets.keys() {
                        visible.insert(agent::pane_key(key, &state.agents.server, pane));
                    }
                }
            }
        }
        let observed = sources
            .iter()
            .filter(|(_, state)| state.observing_agents())
            .map(|(key, state)| (key.as_str(), &state.agents));
        for notice in self.agents.inbox.tick(
            observed,
            &visible,
            &self.local.preferences.agent_notifications,
            now,
        ) {
            if !notifications::show(&notice) {
                self.agents.status = "agent.os_notification_request_failed".into();
            }
        }
        for id in notifications::take_clicks() {
            self.open_agent_notice(&id, cx);
            cx.activate(true);
        }
        if old_count != self.agents.inbox.recent.len()
            || old_unread != self.agents.inbox.recent.iter().filter(|n| !n.read).count()
        {
            cx.notify();
        }
    }
    pub(super) fn open_agent_notice(&mut self, id: &str, cx: &mut Context<Self>) {
        #[cfg(debug_assertions)]
        if id == "native-probe:native-probe" {
            eprintln!("DGL_NOTIFICATION_PROBE_CLICKED");
        }
        let notice = self
            .agents
            .inbox
            .recent
            .iter()
            .find(|n| n.id() == id)
            .cloned();
        self.agents.inbox.mark_read(id);
        let Some(n) = notice else {
            self.snackbar
                .show_snackbar(t("agent.target_connection_closed"));
            return;
        };
        let tab = self
            .tabs
            .iter()
            .filter_map(|t| {
                let state = t.connection.state();
                let valid = endpoint(&t.host) == n.endpoint
                    && state.phase == Phase::Ready
                    && state.agents.current(agent::now_ms())
                    && state.agents.server == n.event.server
                    && state.agents.observations.iter().any(|o| {
                        o.run == n.event.run
                            && o.session == n.event.session
                            && o.window == n.event.window
                            && o.pane == n.event.pane
                    });
                valid.then_some((t, state.generation.connection))
            })
            .min_by_key(|(t, _)| self.session_tab(&t.host.id, &n.event.session) != Some(t.id));
        if let Some((tab, generation)) = tab {
            let c = tab.connection.clone();
            self.select_session_tab(
                &c,
                generation,
                &n.event.session,
                Some(n.event.window),
                Some(n.event.pane),
                cx,
            );
            self.focus_active_terminal(cx);
            self.dialog = None;
        } else if let Some(id) = self
            .tabs
            .iter()
            .find(|t| {
                endpoint(&t.host) == n.endpoint && t.session.as_deref() == Some(&n.event.session)
            })
            .map(|t| t.id)
        {
            // A retained tab remains a navigation destination while it recovers.
            // No pane operation is queued without fresh server/run evidence.
            self.activate_tab(id);
            self.dialog = None;
            self.focus_active_terminal(cx);
        } else {
            self.snackbar
                .show_snackbar(t("agent.target_closed_cannot_navigate"));
        }
        cx.notify();
    }
    pub fn agent_badge(
        &self,
        connection: Arc<Connection>,
        host: &HostProfile,
        pane: &str,
        state: &dgl_core::connection::State,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let mut filtered = state.agents.clone();
        filtered.observations.retain(|o| {
            if pane.starts_with('@') {
                o.window == pane
            } else {
                o.pane == pane
            }
        });
        let snapshot = &filtered;
        let now = agent::now_ms();
        let observation = snapshot.representative(now)?;
        let phase = snapshot.phase_of(observation, now);
        let unread =
            self.agents
                .inbox
                .recent
                .iter()
                .filter(|n| {
                    snapshot.current(now)
                        && !n.read
                        && n.endpoint == endpoint(host)
                        && n.event.server == snapshot.server
                        && (if pane.starts_with('@') {
                            pane == n.event.window
                        } else {
                            pane == n.event.pane
                        })
                        && snapshot.observations.iter().any(|o| {
                            o.run == n.event.run && o.pane == n.event.pane && o.last.is_some()
                        })
                })
                .count();
        let label = observation_label(observation.agent, phase);
        let label = if unread == 0 {
            label
        } else {
            tr!("agent.unread_events", label, unread)
        };
        let id = format!("agent-{}-{}", state.handle, pane);
        let title = host.alias.clone();
        let target_pane = pane.to_owned();
        // No outline: the badge reads as part of the row, not as a separate box.
        Some(
            self.button_base(id, cx, move |s, _, _| {
                s.dialog = Some(Dialog::Agents(
                    connection.clone(),
                    title.clone(),
                    Some(target_pane.clone()),
                ));
            })
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            // Service, status and unread count fit without clipping.
            .w(px(56.))
            .px_1()
            .flex_shrink_0()
            .overflow_hidden()
            .text_xs()
            .native_tooltip(label)
            .child(
                svg()
                    .path(observation.agent.icon())
                    .size(px(15.))
                    .flex_shrink_0()
                    .text_color(rgb(self.palette.fg)),
            )
            .when_some(phase.icon_kind(), |d, kind| {
                d.child(UiIcon::agent(Some(kind)).render(self.palette.fg))
            })
            .when(unread > 0, |d| d.child(unread.to_string())),
        )
    }
    pub fn agent_details(
        &self,
        connection: Arc<Connection>,
        alias: String,
        pane: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.agent_details_state(connection.clone(), alias, pane, &connection.state(), cx)
    }
    pub(super) fn agent_details_state(
        &self,
        connection: Arc<Connection>,
        alias: String,
        pane: Option<&str>,
        state: &dgl_core::connection::State,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let now = agent::now_ms();
        let mut snapshot = state.agents.clone();
        snapshot.observations.retain(|o| {
            pane.is_none_or(|p| {
                if p.starts_with('@') {
                    p == o.window
                } else {
                    p == o.pane
                }
            })
        });
        let mut body = div()
            .id("agent-details")
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("agent-details-title")
                    .text_lg()
                    .child(tr!("agent.integration_named", alias))
                    .native_tooltip(t("agent.last_observed_note")),
            )
            .when_some(snapshot.representative(now), |body, o| {
                let phase = snapshot.phase_of(o, now);
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .when_some(phase.icon_kind(), |d, kind| {
                            d.child(UiIcon::agent(Some(kind)).render(self.palette.fg))
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(observation_label(o.agent, phase)),
                        ),
                )
            });
        for o in snapshot
            .observations
            .iter()
            .filter(|o| snapshot.has_display_details(o, now))
        {
            let last = o.last.as_ref().filter(|_| snapshot.current(now));
            let mut details = vec![tr!("agent.diagnostics_ids", o.session, o.window, o.pane)];
            if let Some(event) = last {
                let supported = o
                    .supported
                    .iter()
                    .map(|k| k.label())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if !supported.is_empty() {
                    details.push(tr!("agent.observed_event_types", supported));
                }
                details.push(event.event.clone());
            }
            body = body.child(
                div()
                    .id(SharedString::from(format!(
                        "agent-observation-{}-{}",
                        o.run, o.actor
                    )))
                    .native_tooltip(details.join("\n"))
                    .child(format!(
                        "{} · {}{}",
                        observation_label(o.agent, snapshot.phase_of(o, now)),
                        hierarchy::target_label(&state.remote, Some(&o.session), Some(&o.pane)),
                        if o.actor != "parent" {
                            t("agent.subagent_suffix")
                        } else {
                            ""
                        }
                    ))
                    .when_some(last, |d, e| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_sm()
                                .child(UiIcon::agent(Some(e.kind)).render(self.palette.fg))
                                .child(tr!(
                                    "agent.event_age_seconds",
                                    e.kind.label(),
                                    now.saturating_sub(e.at) / 1000
                                )),
                        )
                    }),
            );
            let target = o.clone();
            let c = connection.clone();
            let generation = state.generation.connection;
            body = body.child(self.button(
                format!("agent-go-{}-{}", o.run, o.actor),
                t("agent.go_pane"),
                cx,
                move |s, w, cx| {
                    let current = c.state();
                    if current.phase == Phase::Ready
                        && current.generation.connection == generation
                        && current.agents.current(agent::now_ms())
                        && current.agents.observations.iter().any(|o| {
                            o.run == target.run
                                && o.session == target.session
                                && o.window == target.window
                                && o.pane == target.pane
                        })
                    {
                        s.select_session_tab(
                            &c,
                            current.generation.connection,
                            &target.session,
                            Some(target.window.clone()),
                            Some(target.pane.clone()),
                            cx,
                        );
                        s.group_focus(s.workspace.active, w, cx);
                        s.dialog = None;
                    } else {
                        s.snackbar.show_snackbar(t("agent.target_closed"));
                    }
                },
            ));
        }
        if pane.is_some() {
            return body;
        }
        body.child(t("agent.basic_observation")).child(self.button(
            "agent-advanced-hooks",
            t("agent.advanced_hooks"),
            cx,
            move |s, _, _| {
                s.inspect_agent_hooks(&connection);
                s.dialog = Some(Dialog::AgentHooks(connection.clone(), alias.clone()));
            },
        ))
    }

    fn inspect_agent_hooks(&mut self, connection: &Arc<Connection>) {
        self.agents.hud.collect = None;
        self.submit(
            connection,
            Action::AgentSetup {
                operation: "inspect".into(),
                token: None,
                status: false,
            },
        );
    }

    pub(super) fn agent_hook_settings(
        &self,
        connection: Arc<Connection>,
        alias: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let state = connection.state();
        let mut body = div()
            .id("agent-hook-settings")
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_lg().child(tr!("agent.hooks_named", alias)))
            .child(
                div()
                    .id("agent-install-help")
                    .child(t("agent.install_description"))
                    .native_tooltip(t("agent.optional_setup")),
            );
        let c = connection.clone();
        body = body.child(self.button(
            "agent-inspect",
            t("agent.inspect_before_install"),
            cx,
            move |s, _, _| s.inspect_agent_hooks(&c),
        ));
        if let Some(preview) = &state.agent_setup {
            if let Some(error) = preview.get("error").and_then(|v| v.as_str()) {
                body = body.child(error.to_owned());
            }
            if let Some(token) = preview.get("token").and_then(|v| v.as_str()) {
                for field in ["python", "tmux", "helper", "database"] {
                    body = body.child(format!(
                        "{field}: {}",
                        preview[field].as_str().unwrap_or(t("agent.unknown"))
                    ));
                }
                if let Some(configs) = preview["configs"].as_array() {
                    for c in configs {
                        body = body.child(tr!(
                            "agent.integration_summary",
                            c["agent"].as_str().unwrap_or(""),
                            c["version"].as_str().unwrap_or(""),
                            c["path"].as_str().unwrap_or(""),
                            if c["installed"].as_bool() == Some(true) {
                                t("agent.installed_check_activation")
                            } else {
                                t("agent.not_installed")
                            },
                            if c["supported"].as_bool() == Some(true) {
                                t("agent.supported_version")
                            } else {
                                t("agent.unverified_version")
                            },
                            c["events"]
                        ));
                        if let Some(collection) = crate::agent_status_ui::collection_state(c) {
                            body = body.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "agent-status-collection-{}",
                                        c["agent"].as_str().unwrap_or("")
                                    )))
                                    .child(collection),
                            );
                        }
                        if c["experimental"].as_bool() == Some(true) {
                            body = body.child(t("agent.experimental_unverified"));
                        }
                        body = body.child(tr!("agent.available_event_types", c["capabilities"]));
                        if let Some(limitations) =
                            c["limitations"].as_str().filter(|s| !s.is_empty())
                        {
                            body = body.child(limitations.to_owned());
                        }
                        if let Some(error) = c["error"].as_str().filter(|s| !s.is_empty()) {
                            body = body.child(error.to_owned());
                        }
                    }
                }
                body = body.child(
                    div()
                        .id("agent-codex-help")
                        .child(t("agent.codex_trust_hooks_install"))
                        .native_tooltip(t("agent.codex_trust_hooks_real_event")),
                );
                let collect = self.collect_choice(preview);
                body = body
                    .child(self.button(
                        "agent-status-collect",
                        (
                            if collect {
                                UiIcon::CheckboxChecked
                            } else {
                                UiIcon::Checkbox
                            },
                            t("agent_status.collect"),
                        ),
                        cx,
                        move |s, _, _| s.agents.hud.collect = Some(!collect),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(self.palette.muted))
                            .child(t("agent_status.collect_description")),
                    );
                for (op, label) in [
                    ("install", t("agent.install_integration")),
                    ("remove", t("agent.remove_dagali_hooks_only")),
                ] {
                    let c = connection.clone();
                    let token = token.to_owned();
                    body = body.child(self.button(
                        format!("agent-{op}"),
                        label,
                        cx,
                        move |s, _, _| {
                            s.submit(
                                &c,
                                Action::AgentSetup {
                                    operation: op.into(),
                                    token: Some(token.clone()),
                                    status: op == "install" && collect,
                                },
                            );
                        },
                    ));
                }
            }
        }
        body
    }
    pub fn recent_agents(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut body = div().id("recent-agents").flex().flex_col().gap_2();
        if self.agents.inbox.recent.is_empty() {
            return body.child(
                div()
                    .id("notifications-empty")
                    .debug_selector(|| "notifications-empty".into())
                    .py_3()
                    .text_sm()
                    .text_color(rgb(self.palette.muted))
                    .child(t("agent.no_notifications_received")),
            );
        }
        for n in &self.agents.inbox.recent {
            let id = n.id();
            let details = tr!(
                "agent.diagnostics_ids",
                n.event.session,
                n.event.window,
                n.event.pane
            );
            let tip = format!(
                "{}\n{} · {} · {}\n{}",
                details,
                n.event.server,
                n.event.run,
                n.event.event,
                t("agent.read_does_not_answer")
            );
            body = body.child(
                div()
                    .id(SharedString::from(format!("notice-row-{id}")))
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap_1()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(self.palette.line))
                    .native_tooltip(tip)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap_1()
                            .text_sm()
                            .when(!n.read, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("notice-unread-{}", n.id())))
                                        .native_tooltip(t("common.unread"))
                                        .child(UiIcon::Unread.render(self.palette.accent)),
                                )
                            })
                            .child(UiIcon::agent(Some(n.event.kind)).render(self.palette.fg))
                            .child(format!(
                                "{} · {}{}",
                                n.event.agent.label(),
                                n.event.kind.label(),
                                if n.event.actor != "parent" {
                                    t("agent.subagent_suffix")
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(self.palette.muted))
                            .child(tr!(
                                "agent.event_age_seconds",
                                n.endpoint,
                                agent::now_ms().saturating_sub(n.event.at) / 1000
                            )),
                    )
                    .child(self.button(
                        format!("notice-{id}"),
                        t("agent.go_pane"),
                        cx,
                        move |s, _, cx| s.open_agent_notice(&id, cx),
                    )),
            );
        }
        body
    }
    /// The agent notification group of the agents settings page.
    pub fn agent_settings(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let settings = &self.local.preferences.agent_notifications;
        let mut rows: Vec<AnyElement> = [
            (
                0,
                t("agent.questions_approval_requests"),
                settings.questions,
            ),
            (1, t("agent.response_end_signals"), settings.responses),
            (2, t("agent.explicit_errors"), settings.errors),
            (3, t("agent.os_notifications"), settings.os),
        ]
        .into_iter()
        .map(|(i, label, enabled)| {
            self.settings_check(
                format!("notification-setting-{i}"),
                label,
                enabled,
                cx,
                move |s, _, _| {
                    let settings = &mut s.local.preferences.agent_notifications;
                    match i {
                        0 => settings.questions = !settings.questions,
                        1 => settings.responses = !settings.responses,
                        2 => settings.errors = !settings.errors,
                        _ => {
                            settings.os = !settings.os;
                            if settings.os && !notifications::authorize() {
                                s.agents.status =
                                    "agent.os_notification_registration_failed".into();
                            }
                        }
                    }
                    s.save();
                },
            )
            .into_any_element()
        })
        .collect();
        let mut notes = div()
            .flex()
            .flex_col()
            .gap_1()
            .px(px(settings_ui::ROW_PX))
            .py(px(settings_ui::ROW_PY))
            .child(self.settings_note(t("agent.in_app_notifications_note")));
        let status = t(&self.agents.status);
        if !status.is_empty() {
            notes = notes.child(self.settings_note(status.to_owned()));
        }
        rows.push(notes.into_any_element());
        div().id("agent-settings").child(self.settings_group(
            "agent-notifications",
            Some(t("agent.notifications").into()),
            Some(t("settings.notifications_description").into()),
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, UiIcon, agent};
    use agent::{AgentEvent, Kind, LivePhase, Observation, Snapshot};

    #[test]
    fn badge_icons() {
        for (kind, icon) in [
            (Kind::Question, UiIcon::Question),
            (Kind::Approval, UiIcon::Approval),
            (Kind::Error, UiIcon::Error),
            (Kind::Response, UiIcon::Response),
            (Kind::Activity, UiIcon::Refresh),
            (Kind::Ended, UiIcon::Ended),
        ] {
            assert_eq!(UiIcon::agent(Some(kind)), icon);
        }
        assert_eq!(UiIcon::agent(None), UiIcon::Unknown);
    }

    #[test]
    fn badge_requires_a_fresh_phase_status_or_event() {
        let observation = Observation {
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            run: "run".into(),
            actor: "parent".into(),
            agent: Agent::Claude,
            last: None,
            phase: Default::default(),
            phase_seq: 0,
            supported: vec![],
            status: None,
        };
        let mut snapshot = Snapshot {
            valid: true,
            observed_at: 1000,
            ..Default::default()
        };
        assert!(snapshot.representative(1001).is_none());
        snapshot.observations.push(observation);
        assert!(snapshot.representative(1001).is_none());
        assert_eq!(snapshot.agent_in_pane("%1", 1001), Some(Agent::Claude));
        snapshot.observations[0].last = Some(AgentEvent {
            id: "event".into(),
            seq: 1,
            server: "server".into(),
            run: "run".into(),
            actor: "parent".into(),
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            agent: Agent::Claude,
            kind: Kind::Activity,
            event: "UserPromptSubmit".into(),
            at: 1000,
        });
        assert!(snapshot.representative(1001).is_some());
        snapshot.observations[0].phase = LivePhase::Command;
        snapshot.observations[0].phase_seq = 1;
        assert_eq!(
            snapshot
                .phase_of(snapshot.representative(1001).unwrap(), 1001)
                .icon_kind(),
            Some(Kind::Activity)
        );
        snapshot.gap = true;
        assert!(snapshot.representative(1001).is_none());
        snapshot.observations[0].phase_seq = 0;
        assert!(snapshot.representative(1001).is_some());
        assert!(snapshot.representative(11_000).is_none());
        snapshot.invalidate();
        assert!(snapshot.representative(1001).is_none());
    }
}
