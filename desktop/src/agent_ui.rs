use super::*;
use dgl_core::agent::{self, Agent};

fn endpoint(host: &HostProfile) -> String {
    format!("{}@{}", host.username, host.endpoint())
}
impl Desktop {
    pub fn tick_agents(&mut self, cx: &mut Context<Self>) {
        let now = agent::now_ms();
        if now.saturating_sub(self.agent_tick) < 250 {
            return;
        }
        self.agent_tick = now;
        let active = cx.windows().into_iter().any(|h| {
            cx.update_window(h, |_, w, _| w.is_window_active())
                .unwrap_or(false)
        });
        let mut visible = HashSet::new();
        let mut live = HashSet::new();
        let old_count = self.inbox.recent.len();
        let old_unread = self.inbox.recent.iter().filter(|n| !n.read).count();
        for tab in &self.tabs {
            let state = tab.connection.state();
            let key = endpoint(&tab.host);
            if state.agents.current(now) && state.phase == Phase::Ready {
                live.extend(
                    state
                        .agents
                        .observations
                        .iter()
                        .map(|o| format!("{}:{}:{}", key, state.agents.server, o.run)),
                );
                if active
                    && self.dialog.is_none()
                    && self.error.is_none()
                    && self
                        .workspace
                        .groups
                        .values()
                        .any(|g| g.selected == Some(tab.id))
                {
                    for pane in state.render_targets.keys() {
                        visible.insert(format!("{}:{}:{}", key, state.agents.server, pane));
                    }
                }
            }
            self.inbox.ingest(
                &key,
                &state.agents,
                &self.local.preferences.agent_notifications,
                now,
            );
        }
        self.inbox.read_visible(&visible);
        for notice in self.inbox.ready(
            now,
            &visible,
            &live,
            &self.local.preferences.agent_notifications,
        ) {
            if !notifications::show(&notice) {
                self.notification_status =
                    "OS 알림을 요청하지 못했습니다. 앱 내 알림은 유지됩니다.".into();
            }
        }
        for id in notifications::take_clicks() {
            self.open_agent_notice(&id, cx);
            cx.activate(true);
        }
        if old_count != self.inbox.recent.len()
            || old_unread != self.inbox.recent.iter().filter(|n| !n.read).count()
        {
            cx.notify();
        }
    }
    fn open_agent_notice(&mut self, id: &str, cx: &mut Context<Self>) {
        #[cfg(debug_assertions)]
        if id == "native-probe:native-probe" {
            eprintln!("DGL_NOTIFICATION_PROBE_CLICKED");
        }
        let notice = self.inbox.recent.iter().find(|n| n.id() == id).cloned();
        self.inbox.mark_read(id);
        let Some(n) = notice else {
            self.error = Some("대상 연결이 종료되었습니다.".into());
            return;
        };
        let tab = self.tabs.iter().find(|t| {
            let state = t.connection.state();
            endpoint(&t.host) == n.endpoint
                && state.phase == Phase::Ready
                && state.agents.current(agent::now_ms())
                && state.agents.server == n.event.server
                && state
                    .agents
                    .observations
                    .iter()
                    .any(|o| o.run == n.event.run && o.pane == n.event.pane)
        });
        if let Some(tab) = tab {
            if let Some(group) = self
                .workspace
                .groups
                .iter()
                .find(|(_, g)| g.tabs.contains(&tab.id))
                .map(|(id, _)| *id)
            {
                self.workspace.select(group, tab.id);
            }
            let c = tab.connection.clone();
            self.submit(
                &c,
                Action::SelectTarget {
                    session: n.event.session,
                    window: Some(n.event.window),
                    pane: Some(n.event.pane),
                },
            );
            self.dialog = None;
        } else {
            self.error = Some("대상 연결 또는 패널이 종료되어 이동할 수 없습니다.".into());
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
        let mut label = snapshot.label(None, agent::now_ms());
        let unread: Vec<_> =
            self.inbox
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
        if snapshot.current(agent::now_ms())
            && let Some(n) = unread.iter().min_by_key(|n| n.event.kind.priority())
        {
            label = format!(
                "{} · 읽지 않은 사건 {}건",
                n.event.kind.label(),
                unread.len()
            );
        }
        let icon = if label.contains("질문") {
            "?"
        } else if label.contains("승인") {
            "◇"
        } else if label.contains("오류") {
            "!"
        } else if label.contains("응답") {
            "○"
        } else if label.contains("작업 사건") {
            "↻"
        } else {
            "—"
        };
        let id = format!("agent-{}-{}", state.handle, pane);
        let title = host.alias.clone();
        let target_pane = pane.to_owned();
        let tip = label.clone();
        let mut badge = self
            .button(
                id,
                format!(
                    "{icon}{}",
                    if unread.is_empty() {
                        String::new()
                    } else {
                        unread.len().to_string()
                    }
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
            .tooltip(move |_, cx| cx.new(|_| workspace_ui::Tip(tip.clone())).into());
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
        self.agent_details_state(connection.clone(), alias, pane, connection.state(), cx)
    }
    pub(super) fn agent_details_state(
        &self,
        connection: Arc<Connection>,
        alias: String,
        pane: Option<&str>,
        state: dgl_core::connection::State,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut body = div()
            .id("agent-details")
            .flex()
            .flex_col()
            .gap_3()
            .child(format!("에이전트 상태 연동 · {alias}"))
            .child("마지막 관측 사건입니다. 현재 입력·승인 대기 또는 작업 성공을 뜻하지 않습니다.")
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
                filtered.label(None, agent::now_ms())
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
            body = body.child(format!(
                "{} · {}{}",
                o.agent.label(),
                hierarchy::target_label(&state.remote, Some(&o.session), Some(&o.pane)),
                if o.actor != "parent" {
                    " · 서브에이전트"
                } else {
                    ""
                }
            ));
            body = body.child(
                o.last
                    .as_ref()
                    .map(|e| {
                        format!(
                            "{} · {} · {}초 전",
                            e.kind.label(),
                            e.event,
                            agent::now_ms().saturating_sub(e.at) / 1000
                        )
                    })
                    .unwrap_or_else(|| "미연동 · 상태 미확인".into()),
            );
            body = body.child(format!(
                "진단 정보 · 세션 ID: {} · 창 ID: {} · 패널 ID: {}",
                o.session, o.window, o.pane
            ));
            body = body.child(format!(
                "수신 확인 사건: {}",
                o.supported
                    .iter()
                    .map(|k| k.label())
                    .collect::<Vec<_>>()
                    .join(" · ")
            ));
            let target = o.clone();
            let c = connection.clone();
            body = body.child(self.button(
                format!("agent-go-{}-{}", o.run, o.actor),
                "패널로 이동",
                cx,
                move |s, _, _| {
                    let current = c.state();
                    if current.phase == Phase::Ready
                        && current.agents.current(agent::now_ms())
                        && current
                            .agents
                            .observations
                            .iter()
                            .any(|o| o.run == target.run && o.pane == target.pane)
                    {
                        if let Some((group, tab)) = s
                            .tabs
                            .iter()
                            .find(|t| t.connection.state().handle == current.handle)
                            .and_then(|t| {
                                s.workspace
                                    .groups
                                    .iter()
                                    .find(|(_, g)| g.tabs.contains(&t.id))
                                    .map(|(group, _)| (*group, t.id))
                            })
                        {
                            s.workspace.select(group, tab);
                        }
                        s.submit(
                            &c,
                            Action::SelectTarget {
                                session: target.session.clone(),
                                window: Some(target.window.clone()),
                                pane: Some(target.pane.clone()),
                            },
                        );
                        s.dialog = None;
                    } else {
                        s.error = Some("대상 연결 또는 패널이 종료되었습니다.".into());
                    }
                },
            ));
        }
        if pane.is_some() {
            return body;
        }
        body = body.child("선택 설치: Python 3 도우미·사용자 훅을 추가하고 기존 설정을 백업합니다. 대화·명령 본문은 저장하지 않습니다.");
        let c = connection.clone();
        body = body.child(self.button(
            "agent-inspect",
            "설치 전 검사 · 변경 내용 보기",
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
        if let Some(preview) = state.agent_setup {
            if let Some(error) = preview.get("error").and_then(|v| v.as_str()) {
                body = body.child(error.to_owned());
            }
            if let Some(token) = preview.get("token").and_then(|v| v.as_str()) {
                for field in ["python", "tmux", "helper", "database"] {
                    body = body.child(format!(
                        "{field}: {}",
                        preview[field].as_str().unwrap_or("미확인")
                    ));
                }
                if let Some(configs) = preview["configs"].as_array() {
                    for c in configs {
                        body = body.child(format!(
                            "{} · {}\n{}\n{} · {}\n추가 훅: {}",
                            c["agent"].as_str().unwrap_or(""),
                            c["version"].as_str().unwrap_or(""),
                            c["path"].as_str().unwrap_or(""),
                            if c["installed"].as_bool() == Some(true) {
                                "설치됨 / 활성화 확인 필요"
                            } else {
                                "설치되지 않음"
                            },
                            if c["supported"].as_bool() == Some(true) {
                                "지원 버전"
                            } else {
                                "미검증 버전 · 설치 제외"
                            },
                            c["events"]
                        ));
                        if c["experimental"].as_bool() == Some(true) {
                            body = body.child("실험 지원 · 실제 CLI 수신 미검증");
                        }
                        body = body.child(format!("수집 가능 사건: {}", c["capabilities"]));
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
                body = body.child("Codex는 /hooks에서 직접 검토·신뢰해야 합니다. 새 실행 또는 실제 훅 수신으로 확인됩니다. 기존 실행은 종료하지 않습니다.");
                for (op, label) in [("install", "연동 설치"), ("remove", "dgl 훅만 해제")] {
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
        let mut body = div()
            .id("recent-agents")
            .flex()
            .flex_col()
            .gap_2()
            .child("최근 알림 · 이번 실행 중 최대 100건")
            .child("읽음은 질문 응답·승인 처리를 뜻하지 않습니다.");
        for n in &self.inbox.recent {
            let id = n.id();
            body = body
                .child(format!(
                    "{}{} · {} · 진단 정보: 세션 ID {} / 창 ID {} / 패널 ID {}{} · {} · {}초 전",
                    if n.read { "" } else { "● " },
                    n.event.agent.label(),
                    n.endpoint,
                    n.event.session,
                    n.event.window,
                    n.event.pane,
                    if n.event.actor != "parent" {
                        " · 서브에이전트"
                    } else {
                        ""
                    },
                    n.event.kind.label(),
                    agent::now_ms().saturating_sub(n.event.at) / 1000
                ))
                .child(self.button(
                    format!("notice-{id}"),
                    "패널로 이동 · 읽음",
                    cx,
                    move |s, _, cx| s.open_agent_notice(&id, cx),
                ));
        }
        body
    }
    pub fn agent_settings(&self, cx: &mut Context<Self>) -> Div {
        let settings = &self.local.preferences.agent_notifications;
        let mut body = div().flex().flex_col().child("에이전트 알림");
        for (i, label, enabled) in [
            (0, "질문·승인 요청", settings.questions),
            (1, "응답 종료 신호", settings.responses),
            (2, "명시적 오류", settings.errors),
            (3, "OS 알림", settings.os),
        ] {
            body = body.child(self.button(format!("notification-setting-{i}"), format!("{} {label}", if enabled { "☑" } else { "☐" }), cx, move |s, _, _| {
                let settings = &mut s.local.preferences.agent_notifications;
                match i { 0 => settings.questions = !settings.questions, 1 => settings.responses = !settings.responses, 2 => settings.errors = !settings.errors, _ => {
                    settings.os = !settings.os;
                    if settings.os && !notifications::authorize() { s.notification_status = "OS 알림 등록 실패: 패키지 실행과 시스템 알림 설정을 확인하세요.".into(); }
                } }
                s.save();
            }));
        }
        body.child("OS 권한·집중 모드와 관계없이 앱 내 알림은 유지됩니다.")
            .child(self.notification_status.clone())
    }
}
