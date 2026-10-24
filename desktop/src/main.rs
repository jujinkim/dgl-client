mod storage;
mod terminal;
mod text_input;
use dgl_core::{
    command::Operation,
    connection::{Action, Connection, Phase},
    model::{AuthKind, Credential, HostProfile, LocalState},
    ssh,
};
use gpui::{prelude::*, *};
use std::{
    io::Read,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::Terminal;
use text_input::TextInput;
actions!(dgl, [Quit]);
struct Tab {
    host: HostProfile,
    connection: Arc<Connection>,
    terminal: Entity<Terminal>,
    last_sequence: u64,
}
#[derive(Clone)]
enum PromptOp {
    Session,
    Window(String),
    RenameSession(String),
    RenameWindow(String),
}
#[derive(Clone)]
enum DeleteOp {
    Host(HostProfile),
    Remote(Arc<Connection>, Operation),
    Trust(String),
}
#[derive(Clone)]
enum Dialog {
    Host {
        original: Option<HostProfile>,
        fields: Vec<Entity<TextInput>>,
        private_key: String,
        key_auth: bool,
        favorite: bool,
    },
    Prompt {
        title: String,
        connection: Arc<Connection>,
        operation: PromptOp,
        input: Entity<TextInput>,
    },
    Delete {
        title: String,
        operation: DeleteOp,
    },
    Settings,
}
struct Desktop {
    runtime: Arc<tokio::runtime::Runtime>,
    path: PathBuf,
    local: LocalState,
    tabs: Vec<Tab>,
    selected: usize,
    dialog: Option<Dialog>,
    error: Option<String>,
    focus: FocusHandle,
}
impl Desktop {
    fn new(runtime: Arc<tokio::runtime::Runtime>, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let (local, error) = match LocalState::read(&path) {
            Ok(s) => (s, None),
            Err(e) => (
                LocalState::default(),
                Some(format!(
                    "설정을 읽지 못했습니다: {e}. 원본 파일을 보존했습니다."
                )),
            ),
        };
        cx.spawn(async move |view, cx| {
            loop {
                Timer::after(Duration::from_millis(16)).await;
                if view.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            runtime,
            path,
            local,
            tabs: vec![],
            selected: 0,
            dialog: None,
            error,
            focus: cx.focus_handle(),
        }
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for tab in &mut self.tabs {
            tab.terminal.update(cx, |t, cx| t.tick(cx));
            let state = tab.connection.state();
            if state.sequence != tab.last_sequence {
                tab.last_sequence = state.sequence;
                changed = true;
                if state.phase == Phase::Ready
                    && let Some(host) = self.local.hosts.iter_mut().find(|h| h.id == tab.host.id)
                {
                    host.last_session = state.session.clone();
                    host.last_pane = state.pane.clone();
                    host.last_connected = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                }
            }
            for result in tab.connection.completions() {
                let input = tab
                    .terminal
                    .update(cx, |t, _| t.complete(result.request_id));
                if let Some(e) = result.error
                    && (input
                        || !matches!(
                            e,
                            dgl_core::Error::Cancelled | dgl_core::Error::StaleGeneration
                        ))
                {
                    self.error = Some(format!("요청 실패: {e}"));
                    changed = true;
                }
            }
        }
        for tab in &self.tabs {
            if let Some(error) = tab.terminal.update(cx, |t, _| t.notice.take()) {
                self.error = Some(error);
                changed = true;
            }
        }
        if changed {
            if let Err(e) = self.local.save(&self.path) {
                self.error = Some(format!("설정 저장 실패: {e}"));
            }
            cx.notify();
        }
    }
    fn save(&mut self) -> bool {
        match self.local.save(&self.path) {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(format!("설정을 저장하지 못했습니다: {e}"));
                false
            }
        }
    }
    fn open(&mut self, host: HostProfile, cx: &mut Context<Self>) {
        let result = (|| {
            let credential = storage::read(&host.credential_ref)?;
            let trusted = if let Some(expected) = self.local.trust.get(&host.endpoint()) {
                let saved = storage::read(&storage::trust_ref(&host.endpoint()))?;
                if &saved.password != expected {
                    return Err(dgl_core::Error::Storage);
                }
                Some(expected.clone())
            } else {
                None
            };
            Ok((credential, trusted))
        })();
        match result {
            Ok((credential, trusted)) => {
                let _enter = self.runtime.enter();
                let connection = Connection::start(host.clone(), credential, trusted);
                let terminal = cx.new(|cx| {
                    Terminal::new(
                        connection.clone(),
                        self.local.preferences.font_size,
                        self.local.preferences.scrollback,
                        cx,
                    )
                });
                self.tabs.push(Tab {
                    host,
                    connection,
                    terminal,
                    last_sequence: 0,
                });
                self.selected = self.tabs.len() - 1;
            }
            Err(e) => self.error = Some(format!("보호 저장소를 열지 못했습니다: {e}")),
        }
        cx.notify();
    }
    fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            let tab = self.tabs.remove(index);
            tab.connection.close();
            let connection = tab.connection;
            self.runtime.spawn(async move {
                connection.join().await;
            });
            self.selected = self.selected.min(self.tabs.len().saturating_sub(1));
        }
    }
    fn reconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.selected) {
            let mut host = tab.host.clone();
            let state = tab.connection.state();
            host.last_session = state.session.or(host.last_session);
            host.last_pane = state.pane.or(host.last_pane);
            self.close_tab(self.selected);
            self.open(host, cx);
        }
    }
    fn edit(&mut self, original: Option<HostProfile>, cx: &mut Context<Self>) {
        let values = original
            .as_ref()
            .map(|h| {
                vec![
                    h.alias.clone(),
                    h.hostname.clone(),
                    h.port.to_string(),
                    h.username.clone(),
                    String::new(),
                    String::new(),
                    String::new(),
                ]
            })
            .unwrap_or_else(|| {
                vec![
                    String::new(),
                    String::new(),
                    "22".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ]
            });
        let labels = [
            "별칭",
            "호스트 / IP",
            "포트",
            "사용자 이름",
            "비밀번호 (비우면 기존 유지)",
            "키 암호",
            "개인키 파일 경로",
        ];
        let fields = values
            .iter()
            .zip(labels)
            .enumerate()
            .map(|(i, (v, l))| cx.new(|cx| TextInput::new(v, l, i == 4 || i == 5, cx)))
            .collect();
        let key_auth = original
            .as_ref()
            .is_some_and(|h| h.auth == AuthKind::PrivateKey);
        let favorite = original.as_ref().is_some_and(|h| h.favorite);
        self.dialog = Some(Dialog::Host {
            original,
            fields,
            private_key: String::new(),
            key_auth,
            favorite,
        });
    }
    fn save_host(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Host {
            original,
            fields,
            private_key,
            key_auth,
            favorite,
        }) = self.dialog.clone()
        else {
            return;
        };
        let values: Vec<_> = fields
            .iter()
            .map(|f| f.read(cx).content.to_string())
            .collect();
        let id = original.as_ref().map(|h| h.id.clone()).unwrap_or_else(|| {
            format!(
                "host-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )
        });
        let host = HostProfile {
            id: id.clone(),
            alias: values[0].trim().into(),
            hostname: values[1].trim().into(),
            port: values[2].parse().unwrap_or(0),
            username: values[3].trim().into(),
            auth: if key_auth {
                AuthKind::PrivateKey
            } else {
                AuthKind::Password
            },
            credential_ref: original
                .as_ref()
                .map(|h| h.credential_ref.clone())
                .unwrap_or(id),
            favorite,
            last_connected: original.as_ref().map(|h| h.last_connected).unwrap_or(0),
            last_session: original.as_ref().and_then(|h| h.last_session.clone()),
            last_pane: original.as_ref().and_then(|h| h.last_pane.clone()),
        };
        let result = (|| {
            host.validate()?;
            let mut credential = storage::read_optional(&host.credential_ref)?.unwrap_or_default();
            if key_auth {
                if !private_key.is_empty() {
                    credential.private_key = private_key;
                    credential.passphrase = values[5].clone();
                }
                if !values[5].is_empty() {
                    credential.passphrase = values[5].clone();
                }
                if credential.private_key.is_empty() {
                    return Err(dgl_core::Error::InvalidArgument);
                }
                ssh::public_key(&credential)?;
            } else {
                if !values[4].is_empty() {
                    credential.password = values[4].clone();
                }
                if credential.password.is_empty() {
                    return Err(dgl_core::Error::InvalidArgument);
                }
            }
            storage::write(&host.credential_ref, &credential)?;
            let mut next = self.local.clone();
            next.hosts.retain(|h| h.id != host.id);
            next.hosts.push(host);
            next.save(&self.path)?;
            self.local = next;
            Ok::<_, dgl_core::Error>(())
        })();
        match result {
            Ok(()) => self.dialog = None,
            Err(e) => {
                self.error = Some(format!(
                    "저장을 중단했습니다: {e}. 호스트·포트·자격 증명을 확인하세요."
                ))
            }
        }
    }
    fn prompt(&mut self, title: &str, operation: PromptOp, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.selected) {
            self.dialog = Some(Dialog::Prompt {
                title: title.into(),
                connection: tab.connection.clone(),
                operation,
                input: cx.new(|cx| TextInput::new("", "이름", false, cx)),
            });
        }
    }
    fn submit(&mut self, connection: &Arc<Connection>, action: Action) {
        if let Err(e) = connection.action(action) {
            self.error = Some(format!("요청 실패: {e}"));
        }
    }
    fn confirm(&mut self) {
        if let Some(Dialog::Delete { operation, .. }) = self.dialog.clone() {
            let result = match operation {
                DeleteOp::Host(host) => storage::delete(&host.credential_ref).map(|_| {
                    self.local.hosts.retain(|h| h.id != host.id);
                    self.save();
                }),
                DeleteOp::Trust(endpoint) => {
                    storage::delete(&storage::trust_ref(&endpoint)).map(|_| {
                        self.local.trust.remove(&endpoint);
                        self.save();
                    })
                }
                DeleteOp::Remote(connection, operation) => {
                    connection.action(Action::Manage { operation }).map(|_| ())
                }
            };
            if let Err(e) = result {
                self.error = Some(format!("삭제 실패: {e}"))
            } else {
                self.dialog = None;
            }
        }
    }
    fn button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        div()
            .id(ElementId::Name(id.into()))
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .border_1()
            .border_color(rgb(0x53616a))
            .hover(|s| s.bg(rgb(0x36575e)))
            .child(label.into())
            .on_click(cx.listener(move |v, _, w, cx| {
                action(v, w, cx);
                cx.notify();
            }))
    }
    fn theme(&mut self, value: &str, cx: &mut Context<Self>) {
        self.local.preferences.theme = value.into();
        self.save();
        cx.notify();
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        for tab in &self.tabs {
            tab.connection.close();
        }
        self.runtime.block_on(async {
            for tab in &self.tabs {
                tab.connection.join().await;
            }
        });
    }
}
impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = match self.local.preferences.theme.as_str() {
            "light" => false,
            "dark" => true,
            _ => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        let bg = if dark { 0x191e25 } else { 0xf4f6f7 };
        let fg = if dark { 0xd8dee9 } else { 0x18272d };
        let mut sidebar = div()
            .w(px(272.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .p_3()
            .gap_3()
            .border_r_1()
            .border_color(rgb(0x53616a))
            .child(div().text_xl().child("dgl"))
            .child(div().text_sm().child("SSH · tmux / Rust 후보"))
            .child(self.button("add", "+ 호스트", cx, |s, _, cx| s.edit(None, cx)));
        let mut hosts = self.local.hosts.clone();
        hosts.sort_by_key(|h| (!h.favorite, std::cmp::Reverse(h.last_connected)));
        let mut host_list = div()
            .id("hosts")
            .overflow_y_scroll()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2();
        if hosts.is_empty() {
            host_list = host_list.child(div().p_3().child("SSH 호스트를 추가하여 시작하세요."));
        }
        for host in hosts {
            let connect = host.clone();
            let edit = host.clone();
            let delete = host.clone();
            let label = format!("{}{}", if host.favorite { "★ " } else { "" }, host.alias);
            let id = host.id.clone();
            host_list = host_list.child(
                div()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x53616a))
                    .child(div().truncate().child(label))
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .child(format!("{}@{}:{}", host.username, host.hostname, host.port)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .text_xs()
                            .child(self.button(
                                format!("open-{id}"),
                                "연결",
                                cx,
                                move |s, _, cx| s.open(connect.clone(), cx),
                            ))
                            .child(self.button(
                                format!("edit-{id}"),
                                "편집",
                                cx,
                                move |s, _, cx| s.edit(Some(edit.clone()), cx),
                            ))
                            .child(self.button(
                                format!("del-{id}"),
                                "삭제",
                                cx,
                                move |s, _, _| {
                                    s.dialog = Some(Dialog::Delete {
                                        title: format!("호스트 {} 삭제", delete.alias),
                                        operation: DeleteOp::Host(delete.clone()),
                                    })
                                },
                            )),
                    ),
            );
        }
        sidebar = sidebar
            .child(host_list)
            .child(self.button("settings", "설정", cx, |s, _, _| {
                s.dialog = Some(Dialog::Settings)
            }))
            .child(div().text_xs().child("macOS Keychain · 후보 전용 저장소"));
        let mut tabs = div()
            .id("tabs")
            .overflow_x_scroll()
            .flex()
            .gap_2()
            .p_2()
            .border_b_1()
            .border_color(rgb(0x53616a));
        for (i, tab) in self.tabs.iter().enumerate() {
            tabs = tabs.child(
                div()
                    .flex()
                    .gap_1()
                    .when(i == self.selected, |d| {
                        d.bg(rgb(0x36575e)).text_color(rgb(0xffffff))
                    })
                    .child(self.button(
                        format!("tab-{i}"),
                        tab.host.alias.clone(),
                        cx,
                        move |s, w, cx| {
                            s.selected = i;
                            if let Some(t) = s.tabs.get(i) {
                                w.focus(&t.terminal.focus_handle(cx));
                            }
                        },
                    ))
                    .child(self.button(format!("close-{i}"), "×", cx, move |s, _, _| {
                        s.close_tab(i)
                    })),
            );
        }
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(tabs);
        if let Some(tab) = self.tabs.get(self.selected) {
            let connection = tab.connection.clone();
            let state = connection.state();
            let terminal = tab.terminal.clone();
            let phase = state.phase.clone();
            let status = state
                .error
                .map(|e| e.to_string())
                .unwrap_or_else(|| format!("{:?}", state.phase));
            let mut statusbar = div()
                .flex()
                .items_center()
                .gap_2()
                .p_2()
                .child(div().flex_1().child(status))
                .child(self.button("reconnect", "재접속", cx, |s, _, cx| s.reconnect(cx)))
                .child(self.button("disconnect", "연결 종료", cx, |s, _, _| {
                    s.close_tab(s.selected)
                }));
            if phase == Phase::TrustRequired {
                let fingerprint = state.fingerprint.clone().unwrap_or_default();
                let trust_host = tab.host.clone();
                statusbar = statusbar.child(self.button(
                    "trust",
                    "지문 확인 후 신뢰",
                    cx,
                    move |s, _, cx| {
                        let endpoint = trust_host.endpoint();
                        let c = Credential {
                            password: fingerprint.clone(),
                            private_key: String::new(),
                            passphrase: String::new(),
                        };
                        if let Err(e) = storage::write(&storage::trust_ref(&endpoint), &c) {
                            s.error = Some(format!("호스트키 저장 실패: {e}"));
                            return;
                        }
                        s.local.trust.insert(endpoint, fingerprint.clone());
                        if s.save() {
                            s.reconnect(cx);
                        }
                    },
                ));
                main=main.child(div().p_4().child(format!("처음 연결하는 서버 — {}\n{}\n관리자에게 받은 SHA-256 지문과 일치하는지 확인하세요.",tab.host.endpoint(),state.fingerprint.clone().unwrap_or_default())));
            }
            main = main.child(statusbar);
            let mut nav = div()
                .w(px(236.))
                .flex_shrink_0()
                .id("tmux-tree")
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .gap_2();
            if !matches!(
                phase,
                Phase::Shell | Phase::Closed | Phase::Failed | Phase::TrustRequired
            ) {
                nav = nav.child(self.button("new-session", "+ 세션", cx, |s, _, cx| {
                    s.prompt("세션 만들기", PromptOp::Session, cx)
                }));
                let c = connection.clone();
                nav = nav.child(self.button("refresh", "새로고침", cx, move |s, _, _| {
                    s.submit(&c, Action::Refresh)
                }));
                for session in &state.remote.sessions {
                    let id = session.id.clone();
                    let c = connection.clone();
                    nav = nav.child(self.button(
                        format!("session-{id}"),
                        format!(
                            "{} {}",
                            if state.session.as_ref() == Some(&id) {
                                "▾"
                            } else {
                                "▸"
                            },
                            session.name
                        ),
                        cx,
                        move |s, _, _| s.submit(&c, Action::SelectSession { id: id.clone() }),
                    ));
                    if state.session.as_ref() == Some(&session.id) {
                        let sid = session.id.clone();
                        nav = nav.child(self.button("new-window", "+ 창", cx, move |s, _, cx| {
                            s.prompt("창 만들기", PromptOp::Window(sid.clone()), cx)
                        }));
                        let sid = session.id.clone();
                        nav = nav.child(self.button(
                            "rename-session",
                            "세션 이름",
                            cx,
                            move |s, _, cx| {
                                s.prompt("세션 이름 변경", PromptOp::RenameSession(sid.clone()), cx)
                            },
                        ));
                        let id = session.id.clone();
                        let name = session.name.clone();
                        let c = connection.clone();
                        nav = nav.child(self.button(
                            "kill-session",
                            "세션 삭제…",
                            cx,
                            move |s, _, _| {
                                s.dialog = Some(Dialog::Delete {
                                    title: format!("세션 {name} ({id}) 삭제"),
                                    operation: DeleteOp::Remote(
                                        c.clone(),
                                        Operation::KillSession { target: id.clone() },
                                    ),
                                })
                            },
                        ));
                        for win in state
                            .remote
                            .windows
                            .iter()
                            .filter(|w| w.session == session.id)
                        {
                            nav = nav.child(
                                div()
                                    .mt_2()
                                    .text_sm()
                                    .truncate()
                                    .child(format!("{} · {}", win.id, win.name)),
                            );
                            let id = win.id.clone();
                            nav = nav.child(self.button(
                                format!("rename-{id}"),
                                "이름 변경",
                                cx,
                                move |s, _, cx| {
                                    s.prompt("창 이름 변경", PromptOp::RenameWindow(id.clone()), cx)
                                },
                            ));
                            let id = win.id.clone();
                            let name = win.name.clone();
                            let c = connection.clone();
                            nav = nav.child(self.button(
                                format!("kill-{id}"),
                                "창 삭제…",
                                cx,
                                move |s, _, _| {
                                    s.dialog = Some(Dialog::Delete {
                                        title: format!("창 {name} ({id}) 삭제"),
                                        operation: DeleteOp::Remote(
                                            c.clone(),
                                            Operation::KillWindow { target: id.clone() },
                                        ),
                                    })
                                },
                            ));
                            for pane in state.remote.panes.iter().filter(|p| p.window == win.id) {
                                let id = pane.id.clone();
                                let c = connection.clone();
                                nav = nav.child(self.button(
                                    format!("pane-{id}"),
                                    format!(
                                        "{} {} · {}×{}",
                                        if state.pane.as_ref() == Some(&id) {
                                            "●"
                                        } else {
                                            "○"
                                        },
                                        pane.command,
                                        pane.width,
                                        pane.height
                                    ),
                                    cx,
                                    move |s, _, _| {
                                        s.submit(&c, Action::SelectPane { id: id.clone() })
                                    },
                                ));
                            }
                        }
                    }
                }
            } else if phase == Phase::Shell {
                nav = nav.child("일반 SSH\n연결 종료 시 원격 작업이 끝날 수 있습니다.");
                for d in &state.diagnostics {
                    nav = nav.child(format!("{}: {} ({:?})", d.stage, d.outcome, d.exit_status));
                }
            }
            let mut area = div().flex_1().min_w_0().flex().flex_col();
            if let Some(pane) = state.pane.as_ref().filter(|p| p.as_str() != "shell") {
                area = area.child(div().text_xs().p_2().child(format!(
                    "세션 {} / 패널 {pane}",
                    state.session.as_deref().unwrap_or("—")
                )));
                let mut controls = div()
                    .id("pane-controls")
                    .overflow_x_scroll()
                    .flex()
                    .gap_2()
                    .p_2()
                    .text_xs();
                for (horizontal, label) in [(true, "좌우 분할"), (false, "상하 분할")] {
                    let c = connection.clone();
                    let id = pane.clone();
                    controls = controls.child(self.button(label, label, cx, move |s, _, _| {
                        s.submit(
                            &c,
                            Action::Manage {
                                operation: Operation::Split {
                                    target: id.clone(),
                                    horizontal,
                                },
                            },
                        )
                    }));
                }
                for direction in ['L', 'R', 'U', 'D'] {
                    let c = connection.clone();
                    let id = pane.clone();
                    controls = controls.child(self.button(
                        format!("resize-{direction}"),
                        format!("크기 {direction}"),
                        cx,
                        move |s, _, _| {
                            s.submit(
                                &c,
                                Action::Manage {
                                    operation: Operation::ResizePane {
                                        target: id.clone(),
                                        direction,
                                    },
                                },
                            )
                        },
                    ));
                }
                let c = connection.clone();
                let id = pane.clone();
                controls = controls.child(self.button(
                    "kill-pane",
                    "패널 삭제…",
                    cx,
                    move |s, _, _| {
                        s.dialog = Some(Dialog::Delete {
                            title: format!("패널 {id} 삭제"),
                            operation: DeleteOp::Remote(
                                c.clone(),
                                Operation::KillPane { target: id.clone() },
                            ),
                        })
                    },
                ));
                area = area.child(controls);
            }
            area = area.child(div().flex_1().min_h_0().child(terminal.clone()));
            let mut shortcuts = div()
                .id("shortcut-bar")
                .overflow_x_scroll()
                .flex()
                .gap_2()
                .p_2()
                .text_xs();
            for (key, label) in [
                (dgl_core::input::TerminalKey::Escape, "Esc"),
                (dgl_core::input::TerminalKey::Tab, "Tab"),
                (dgl_core::input::TerminalKey::CtrlC, "Ctrl+C"),
                (dgl_core::input::TerminalKey::CtrlD, "Ctrl+D"),
                (dgl_core::input::TerminalKey::CtrlL, "Ctrl+L"),
                (dgl_core::input::TerminalKey::Up, "↑"),
                (dgl_core::input::TerminalKey::Down, "↓"),
                (dgl_core::input::TerminalKey::Left, "←"),
                (dgl_core::input::TerminalKey::Right, "→"),
                (dgl_core::input::TerminalKey::Home, "Home"),
                (dgl_core::input::TerminalKey::End, "End"),
            ] {
                let t = terminal.clone();
                shortcuts = shortcuts.child(self.button(
                    format!("key-{label}"),
                    label,
                    cx,
                    move |_, w, cx| {
                        t.update(cx, |t, _| t.send(key.bytes().to_vec()));
                        w.focus(&t.focus_handle(cx));
                    },
                ));
            }
            area = area.child(shortcuts);
            main = main.child(div().flex_1().min_h_0().flex().child(nav).child(area));
        } else {
            main = main.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap_3()
                    .child(div().text_3xl().child("Keep the CLI."))
                    .child("세션은 서버에, 작업은 그대로.")
                    .child("호스트를 추가하고 tmux 세션에 연결하세요."),
            );
        }
        let mut root = div()
            .size_full()
            .relative()
            .font_family(".AppleSystemUIFont")
            .text_size(px(14.))
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if !e.keystroke.modifiers.platform || s.dialog.is_some() {
                    return;
                }
                match e.keystroke.key.as_str() {
                    "n" if e.keystroke.modifiers.shift => {
                        s.prompt("세션 만들기", PromptOp::Session, cx)
                    }
                    "n" => s.edit(None, cx),
                    "w" => s.close_tab(s.selected),
                    "," => s.dialog = Some(Dialog::Settings),
                    "r" => {
                        if let Some(t) = s.tabs.get(s.selected) {
                            let c = t.connection.clone();
                            s.submit(&c, Action::Refresh);
                        }
                    }
                    key => {
                        if let Ok(n) = key.parse::<usize>() {
                            if n > 0 && n <= s.tabs.len() {
                                s.selected = n - 1;
                                w.focus(&s.tabs[s.selected].terminal.focus_handle(cx));
                            }
                        } else {
                            return;
                        }
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(div().size_full().flex().child(sidebar).child(main));
        if let Some(dialog) = self.dialog.clone() {
            let mut body = div()
                .id("dialog-body")
                .overflow_y_scroll()
                .max_h(px(620.))
                .flex()
                .flex_col()
                .gap_3();
            let mut footer = div().flex().gap_2().justify_end().child(self.button(
                "cancel-dialog",
                "취소 / 닫기",
                cx,
                |s, _, _| s.dialog = None,
            ));
            match dialog {
                Dialog::Host {
                    original,
                    fields,
                    key_auth,
                    favorite,
                    ..
                } => {
                    body = body.child(div().text_xl().child(if original.is_some() {
                        "호스트 편집"
                    } else {
                        "SSH 호스트 추가"
                    }));
                    let labels = [
                        "별칭",
                        "호스트 / IP",
                        "포트",
                        "사용자 이름",
                        "비밀번호",
                        "키 암호",
                        "개인키 파일 경로",
                    ];
                    for (i, f) in fields.iter().enumerate() {
                        if (i == 4 && key_auth) || (i >= 5 && !key_auth) {
                            continue;
                        }
                        body = body
                            .child(div().text_sm().child(labels[i]))
                            .child(f.clone());
                    }
                    body = body
                        .child(self.button(
                            "auth",
                            if key_auth {
                                "인증: 개인키"
                            } else {
                                "인증: 비밀번호"
                            },
                            cx,
                            |s, _, _| {
                                if let Some(Dialog::Host { key_auth, .. }) = s.dialog.as_mut() {
                                    *key_auth = !*key_auth
                                }
                            },
                        ))
                        .child(self.button(
                            "favorite",
                            if favorite {
                                "★ 즐겨찾기"
                            } else {
                                "☆ 즐겨찾기"
                            },
                            cx,
                            |s, _, _| {
                                if let Some(Dialog::Host { favorite, .. }) = s.dialog.as_mut() {
                                    *favorite = !*favorite
                                }
                            },
                        ));
                    if key_auth {
                        body = body
                            .child(self.button(
                                "import",
                                "개인키 가져오기",
                                cx,
                                |s, _, cx| {
                                    if let Some(Dialog::Host {
                                        fields,
                                        private_key,
                                        ..
                                    }) = s.dialog.as_mut()
                                    {
                                        let path = fields[6].read(cx).content.to_string();
                                        let bytes = std::fs::File::open(path).and_then(|file| {
                                            let mut bytes = Vec::new();
                                            file.take(131073).read_to_end(&mut bytes)?;
                                            Ok(bytes)
                                        });
                                        match bytes {
                                            Ok(b) if b.len() <= 131072 => {
                                                match String::from_utf8(b) {
                                                    Ok(k) => *private_key = k,
                                                    Err(_) => {
                                                        s.error = Some("키 파일 인코딩 오류".into())
                                                    }
                                                }
                                            }
                                            _ => {
                                                s.error = Some(
                                                    "개인키를 읽지 못했습니다 (최대 128 KiB)."
                                                        .into(),
                                                )
                                            }
                                        }
                                    }
                                },
                            ))
                            .child(self.button("generate", "Ed25519 생성", cx, |s, _, _| {
                                match ssh::generate_key() {
                                    Ok(k) => {
                                        if let Some(Dialog::Host { private_key, .. }) =
                                            s.dialog.as_mut()
                                        {
                                            *private_key = k.private_key.clone();
                                        }
                                    }
                                    Err(e) => s.error = Some(e.to_string()),
                                }
                            }))
                            .child(self.button(
                                "public-key",
                                "공개키 복사",
                                cx,
                                |s, _, cx| {
                                    if let Some(Dialog::Host {
                                        original,
                                        private_key,
                                        fields,
                                        ..
                                    }) = &s.dialog
                                    {
                                        let result = (|| {
                                            let mut c = if private_key.is_empty() {
                                                storage::read(
                                                    &original
                                                        .as_ref()
                                                        .ok_or(dgl_core::Error::InvalidArgument)?
                                                        .credential_ref,
                                                )?
                                            } else {
                                                Credential {
                                                    private_key: private_key.clone(),
                                                    password: String::new(),
                                                    passphrase: String::new(),
                                                }
                                            };
                                            let passphrase = fields[5].read(cx).content.to_string();
                                            if !passphrase.is_empty() {
                                                c.passphrase = passphrase;
                                            }
                                            ssh::public_key(&c)
                                        })();
                                        match result {
                                            Ok(k) => {
                                                cx.write_to_clipboard(ClipboardItem::new_string(k))
                                            }
                                            Err(e) => s.error = Some(e.to_string()),
                                        }
                                    }
                                },
                            ));
                    }
                    footer = footer
                        .child(self.button("save-host", "저장", cx, |s, _, cx| s.save_host(cx)));
                }
                Dialog::Prompt {
                    title,
                    connection,
                    operation,
                    input,
                } => {
                    body = body
                        .child(div().text_xl().child(title))
                        .child(input.clone());
                    footer =
                        footer.child(self.button("accept-name", "확인", cx, move |s, _, cx| {
                            let name = input.read(cx).content.to_string();
                            let op = match &operation {
                                PromptOp::Session => Operation::NewSession { name },
                                PromptOp::Window(id) => Operation::NewWindow {
                                    target: id.clone(),
                                    name,
                                },
                                PromptOp::RenameSession(id) => Operation::RenameSession {
                                    target: id.clone(),
                                    name,
                                },
                                PromptOp::RenameWindow(id) => Operation::RenameWindow {
                                    target: id.clone(),
                                    name,
                                },
                            };
                            match connection.action(Action::Manage { operation: op }) {
                                Ok(_) => s.dialog = None,
                                Err(e) => s.error = Some(format!("이름을 확인하세요: {e}")),
                            }
                        }));
                }
                Dialog::Delete { title, .. } => {
                    body=body.child(div().text_xl().child(title)).child("원격 대상 삭제는 해당 프로세스를 종료합니다. 로컬 호스트 삭제는 저장된 연결 정보만 삭제합니다.");
                    footer =
                        footer.child(
                            self.button("confirm-delete", "삭제", cx, |s, _, _| s.confirm()),
                        );
                }
                Dialog::Settings => {
                    body = body.child(div().text_xl().child("설정"));
                    for (value, label) in
                        [("system", "시스템"), ("light", "밝게"), ("dark", "어둡게")]
                    {
                        body = body.child(self.button(
                            format!("theme-{value}"),
                            label,
                            cx,
                            move |s, _, cx| s.theme(value, cx),
                        ));
                    }
                    body = body.child(format!(
                        "터미널 글자 크기: {}",
                        self.local.preferences.font_size
                    ));
                    for (delta, label) in [(-1, "작게"), (1, "크게")] {
                        body = body.child(self.button(label, label, cx, move |s, _, cx| {
                            s.local.preferences.font_size =
                                (s.local.preferences.font_size + delta).clamp(10, 28);
                            for t in &s.tabs {
                                t.terminal
                                    .update(cx, |t, _| t.set_font(s.local.preferences.font_size));
                            }
                            s.save();
                        }));
                    }
                    body = body
                        .child("⌘N 호스트 · ⇧⌘N 세션 · ⌘W 연결 종료 · ⌘1–9 연결 전환 · ⌘R 새로고침")
                        .child("신뢰한 서버");
                    for (endpoint, fingerprint) in &self.local.trust {
                        let e = endpoint.clone();
                        body = body
                            .child(format!("{endpoint}\n{fingerprint}"))
                            .child(self.button(
                                format!("forget-{endpoint}"),
                                "신뢰 삭제…",
                                cx,
                                move |s, _, _| {
                                    s.dialog = Some(Dialog::Delete {
                                        title: format!("{e} 신뢰 삭제"),
                                        operation: DeleteOp::Trust(e.clone()),
                                    })
                                },
                            ));
                    }
                }
            }
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x00000099))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(
                        div()
                            .w(px(520.))
                            .max_w_full()
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(bg))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(body)
                            .child(footer),
                    ),
            );
        }
        if let Some(error) = self.error.clone() {
            root = root.child(
                div()
                    .absolute()
                    .bottom_4()
                    .left_4()
                    .right_4()
                    .p_4()
                    .bg(rgb(0x682f38))
                    .text_color(rgb(0xffffff))
                    .rounded_md()
                    .flex()
                    .gap_3()
                    .child(div().flex_1().child(error))
                    .child(self.button("dismiss-error", "닫기", cx, |s, _, _| s.error = None)),
            );
        }
        root
    }
}
fn main() {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap(),
    );
    let path = PathBuf::from(std::env::var_os("DGL_CANDIDATE_DIR").unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").expect("HOME"))
            .join("Library/Application Support/dgl-rust-candidate-v1")
            .into_os_string()
    }))
    .join("settings.json");
    Application::new().run(move |cx: &mut App| {
        text_input::bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        let bounds = Bounds::centered(None, size(px(1240.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("dgl — Rust 후보".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, cx| cx.new(|cx| Desktop::new(runtime, path, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
