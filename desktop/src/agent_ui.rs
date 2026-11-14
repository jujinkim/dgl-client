use super::*;
use dgl_core::agent::{self, Agent};

/// Agent notices collected from every connection and the notification button's state.
#[derive(Default)]
pub(crate) struct AgentState {
    pub inbox: agent::Inbox,
    pub tick: u64,
    pub status: String,
    pub anchor: Rc<RefCell<Bounds<Pixels>>>,
    #[cfg(debug_assertions)]
    pub probe_at: Option<u64>,
}

fn endpoint(host: &HostProfile) -> String {
    format!("{}@{}", host.username, host.endpoint())
}

fn badge_kind(
    snapshot: &agent::Snapshot,
    unread: &[&agent::Notice],
    now: u64,
) -> Option<agent::Kind> {
    if !snapshot.current(now) {
        return None;
    }
    unread
        .iter()
        .min_by_key(|n| n.event.kind.priority())
        .map(|n| n.event.kind)
        .or_else(|| snapshot.display_kind(None, now))
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
                if active
                    && self.dialog.is_none()
                    && self.snackbar.is_none()
                    && self
                        .workspace
                        .groups
                        .values()
                        .any(|g| g.selected == Some(tab.id))
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
    ) -> Stateful<Div> {
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
        let mut label = snapshot.label(None, now);
        let unread: Vec<_> =
            self.agents
                .inbox
                .recent
                .iter()
                .filter(|n| {
                    !n.read
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
                .collect();
        if snapshot.current(now)
            && let Some(n) = unread.iter().min_by_key(|n| n.event.kind.priority())
        {
            label = tr!("agent.unread_events", n.event.kind.label(), unread.len());
        }
        let icon = UiIcon::agent(badge_kind(snapshot, &unread, now));
        let id = format!("agent-{}-{}", state.handle, pane);
        let title = host.alias.clone();
        let target_pane = pane.to_owned();
        let tip = label.clone();
        let mut badge = self
            .button(
                id,
                (
                    icon,
                    if unread.is_empty() {
                        String::new()
                    } else {
                        unread.len().to_string()
                    },
                ),
                cx,
                move |s, _, _| {
                    s.dialog = Some(Dialog::Agents(
                        connection.clone(),
                        title.clone(),
                        Some(target_pane.clone()),
                    ));
                },
            )
            .flex()
            .items_center()
            .gap_1()
            .w(px(48.))
            .px_1()
            .flex_shrink_0()
            .overflow_hidden()
            .text_xs()
            .native_tooltip(tip);
        if unread.len() < 10
            && let Some(a) = Agent::ALL
                .into_iter()
                .find(|a| snapshot.observations.iter().any(|o| o.agent == *a))
        {
            badge = badge.child(
                svg()
                    .path(a.icon())
                    .size(px(15.))
                    .text_color(rgb(self.palette.fg)),
            );
        }
        badge
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
            .child({
                let mut filtered = state.agents.clone();
                filtered.observations.retain(|o| {
                    pane.is_none_or(|p| {
                        if p.starts_with('@') {
                            p == o.window
                        } else {
                            p == o.pane
                        }
                    })
                });
                let now = agent::now_ms();
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(UiIcon::agent(filtered.display_kind(None, now)).render(self.palette.fg))
                    .child(div().flex_1().min_w_0().child(filtered.label(None, now)))
            });
        for o in state.agents.observations.iter().filter(|o| {
            pane.is_none_or(|p| {
                if p.starts_with('@') {
                    p == o.window
                } else {
                    p == o.pane
                }
            })
        }) {
            let details = tr!("agent.diagnostics_ids", o.session, o.window, o.pane);
            let supported = tr!(
                "agent.observed_event_types",
                o.supported
                    .iter()
                    .map(|k| k.label())
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
            body = body.child(
                div()
                    .id(SharedString::from(format!(
                        "agent-observation-{}-{}",
                        o.run, o.actor
                    )))
                    .native_tooltip(format!(
                        "{details}\n{supported}\n{}",
                        o.last.as_ref().map_or("", |e| e.event.as_str())
                    ))
                    .child(format!(
                        "{} · {}{}",
                        o.agent.label(),
                        hierarchy::target_label(&state.remote, Some(&o.session), Some(&o.pane)),
                        if o.actor != "parent" {
                            t("agent.subagent_suffix")
                        } else {
                            ""
                        }
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_sm()
                            .child(
                                UiIcon::agent(o.last.as_ref().map(|e| e.kind))
                                    .render(self.palette.fg),
                            )
                            .child(
                                o.last
                                    .as_ref()
                                    .map(|e| {
                                        tr!(
                                            "agent.event_age_seconds",
                                            e.kind.label(),
                                            agent::now_ms().saturating_sub(e.at) / 1000
                                        )
                                    })
                                    .unwrap_or_else(|| t("agent.not_linked").into()),
                            ),
                    ),
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
        body = body.child(
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
            move |s, _, _| {
                s.submit(
                    &c,
                    Action::AgentSetup {
                        operation: "inspect".into(),
                        token: None,
                    },
                );
            },
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
    pub fn agent_settings(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let settings = &self.local.preferences.agent_notifications;
        let mut body = div()
            .id("agent-settings")
            .flex()
            .flex_col()
            .native_tooltip(t("agent.in_app_notifications_note"));
        for (i, label, enabled) in [
            (
                0,
                t("agent.questions_approval_requests"),
                settings.questions,
            ),
            (1, t("agent.response_end_signals"), settings.responses),
            (2, t("agent.explicit_errors"), settings.errors),
            (3, t("agent.os_notifications"), settings.os),
        ] {
            body = body.child(self.button(
                format!("notification-setting-{i}"),
                (
                    if enabled {
                        UiIcon::CheckboxChecked
                    } else {
                        UiIcon::Checkbox
                    },
                    label,
                ),
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
            ));
        }
        body.child(t(&self.agents.status).to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, UiIcon, agent, badge_kind};
    use agent::{AgentEvent, Kind, Notice, Observation, Snapshot};

    #[test]
    fn badge_uses_unread_priority_and_stale_observations_are_unknown() {
        let event = AgentEvent {
            id: "event".into(),
            seq: 1,
            server: "server".into(),
            run: "run".into(),
            actor: "parent".into(),
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            agent: Agent::Codex,
            kind: Kind::Question,
            event: "Question".into(),
            at: 1000,
        };
        let mut snapshot = Snapshot {
            valid: true,
            observed_at: 1000,
            observations: vec![Observation {
                session: event.session.clone(),
                window: event.window.clone(),
                pane: event.pane.clone(),
                run: event.run.clone(),
                actor: event.actor.clone(),
                agent: event.agent,
                last: Some(event.clone()),
                supported: vec![],
            }],
            ..Default::default()
        };
        let response = Notice {
            endpoint: "host".into(),
            event: AgentEvent {
                kind: Kind::Response,
                ..event.clone()
            },
            read: false,
        };
        let error = Notice {
            event: AgentEvent {
                kind: Kind::Error,
                ..event
            },
            ..response.clone()
        };
        assert_eq!(badge_kind(&snapshot, &[], 1001), Some(Kind::Question));
        assert_eq!(
            badge_kind(&snapshot, &[&response], 1001),
            Some(Kind::Response)
        );
        assert_eq!(
            badge_kind(&snapshot, &[&response, &error], 1001),
            Some(Kind::Error)
        );
        assert_eq!(badge_kind(&snapshot, &[&error], 11000), None);
        snapshot.gap = true;
        assert_eq!(badge_kind(&snapshot, &[&error], 1001), None);
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
}
