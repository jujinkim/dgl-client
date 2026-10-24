#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod launcher;
mod modal;
mod pane_toolbar;
mod platform;
mod storage;
mod terminal;
mod text_input;
mod theme;
#[cfg(test)]
mod ui_tests;
#[cfg(target_os = "windows")]
mod windows_ime;
mod workspace;
mod workspace_ui;
use dgl_core::{
    command::Operation,
    connection::{Action, Connection, Phase},
    model::{AuthKind, Credential, HostProfile, LocalState},
    ssh,
};
use gpui::{prelude::*, *};
use std::collections::{HashMap, HashSet};
use std::{
    io::Read,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::Terminal;
use text_input::TextInput;
use workspace::{Axis, GroupId, Layout, TabId, Workspace};
actions!(dgl, [Quit]);
struct Tab {
    id: TabId,
    refresh: Option<u64>,
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
    workspace: Workspace,
    palette: theme::Palette,
    modal: modal::Modal,
    expanded: HashSet<String>,
    menu: Option<String>,
    launcher: launcher::LaunchMenu,
    group_bounds: HashMap<GroupId, Bounds<Pixels>>,
    split_bounds: HashMap<Vec<bool>, Bounds<Pixels>>,
    resizing: Option<Vec<bool>>,
    dragging_tab: Option<TabId>,
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
        let workspace = Workspace::new(Layout::load(&path.with_file_name("workspace-layout.json")));
        Self {
            runtime,
            path,
            local,
            tabs: vec![],
            workspace,
            palette: theme::Palette::new(true),
            modal: modal::Modal::default(),
            expanded: HashSet::new(),
            menu: None,
            launcher: launcher::LaunchMenu::default(),
            group_bounds: HashMap::new(),
            split_bounds: HashMap::new(),
            resizing: None,
            dragging_tab: None,
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
                if tab.refresh == Some(result.request_id) {
                    tab.refresh = None;
                    changed = true;
                }
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
                let id = self.workspace.add_tab();
                self.tabs.push(Tab {
                    id,
                    refresh: None,
                    host,
                    connection,
                    terminal,
                    last_sequence: 0,
                });
            }
            Err(e) => self.error = Some(format!("보호 저장소를 열지 못했습니다: {e}")),
        }
        cx.notify();
    }
    fn active_tab(&self) -> Option<&Tab> {
        let id = self.workspace.selected()?;
        self.tabs.iter().find(|t| t.id == id)
    }
    fn close_tab(&mut self, id: TabId) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            let tab = self.tabs.remove(index);
            self.workspace.close(id);
            tab.connection.close();
            self.runtime.spawn(async move {
                tab.connection.join().await;
            });
        }
    }
    fn reconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            let mut host = tab.host.clone();
            let state = tab.connection.state();
            host.last_session = state.session.or(host.last_session);
            host.last_pane = state.pane.or(host.last_pane);
            let id = tab.id;
            self.close_tab(id);
            self.open(host, cx);
        }
    }
    fn save_layout(&mut self) {
        if let Err(e) = self
            .workspace
            .layout
            .save(&self.path.with_file_name("workspace-layout.json"))
        {
            self.error = Some(format!("배치 저장 실패: {e}"));
        }
    }
    fn refresh(&mut self) {
        if let Some(id) = self.workspace.selected()
            && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && tab.refresh.is_none()
        {
            match tab.connection.action(Action::Refresh) {
                Ok(request) => tab.refresh = Some(request),
                Err(e) => self.error = Some(format!("조회 실패: {e}")),
            }
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
        let ssh_directory = platform::ssh_directory_placeholder();
        let labels = [
            "별칭",
            "호스트 / IP",
            "포트",
            "사용자 이름",
            "비밀번호 (비우면 기존 유지)",
            "키 암호",
            &ssh_directory,
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
        if let Some(tab) = self.active_tab() {
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
        let id = id.into();
        let palette = self.palette;
        div()
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .px_2()
            .py_1()
            .flex_shrink_0()
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(palette.selection)))
            .when(self.modal.building, |d| {
                d.track_focus(&self.modal.button(&id, cx))
            })
            .focus(move |s| {
                s.bg(rgb(palette.selection))
                    .border_color(rgb(palette.accent))
            })
            .border_1()
            .border_color(gpui::transparent_black())
            .child(label.into())
            .on_click(cx.listener(move |v, _, w, cx| {
                action(v, w, cx);
                cx.stop_propagation();
                cx.notify();
            }))
    }
    fn accept_name(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Prompt {
            connection,
            operation,
            input,
            ..
        }) = self.dialog.clone()
        else {
            return;
        };
        if input.read(cx).is_composing() {
            return;
        }
        let name = input.read(cx).content.to_string();
        let operation = match operation {
            PromptOp::Session => Operation::NewSession { name },
            PromptOp::Window(target) => Operation::NewWindow { target, name },
            PromptOp::RenameSession(target) => Operation::RenameSession { target, name },
            PromptOp::RenameWindow(target) => Operation::RenameWindow { target, name },
        };
        match connection.action(Action::Manage { operation }) {
            Ok(_) => self.dialog = None,
            Err(e) => self.error = Some(format!("이름을 확인하세요: {e}")),
        }
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
        self.palette = theme::Palette::new(dark);
        self.sync_launcher(window, cx);
        let bg = self.palette.bg;
        if self.dialog.is_none() {
            self.modal.restore(window);
            if window.focused(cx).is_none() {
                window.focus(&self.focus);
            }
        }
        for tab in &self.tabs {
            tab.terminal
                .update(cx, |t, _| t.enabled = self.dialog.is_none());
        }
        let mut root = self.render_workspace(window, cx);
        if let Some(dialog) = self.dialog.clone() {
            let (kind, inputs) = match &dialog {
                Dialog::Host {
                    fields, key_auth, ..
                } => (
                    if *key_auth {
                        "host-key"
                    } else {
                        "host-password"
                    },
                    fields
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !(*i == 4 && *key_auth || *i >= 5 && !*key_auth))
                        .map(|(_, f)| f.clone())
                        .collect::<Vec<_>>(),
                ),
                Dialog::Prompt { input, .. } => ("prompt", vec![input.clone()]),
                Dialog::Delete { .. } => ("delete", vec![]),
                Dialog::Settings => ("settings", vec![]),
            };
            for input in &inputs {
                input.update(cx, |i, _| i.palette = self.palette);
            }
            self.modal.begin(
                kind,
                inputs.iter().map(|i| i.focus_handle(cx)).collect(),
                window,
                cx,
            );
            let mut body = div()
                .id("dialog-body")
                .overflow_y_scroll()
                .max_h(px(
                    (f32::from(window.viewport_size().height) - 180.).clamp(100., 620.)
                ))
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
                        if i == 4 || i == 5 {
                            let input = f.clone();
                            body = body.child(self.button(
                                format!("show-secret-{i}"),
                                if f.read(cx).secret_visible() {
                                    "☑ 비밀번호 표시"
                                } else {
                                    "☐ 비밀번호 표시"
                                },
                                cx,
                                move |_, _, cx| {
                                    input
                                        .update(cx, |input, cx| input.toggle_secret_visibility(cx));
                                },
                            ));
                        }
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
                    let _ = (connection, operation);
                    footer = footer.child(
                        self.button("accept-name", "확인", cx, |s, _, cx| s.accept_name(cx)),
                    );
                }

                Dialog::Delete { title, .. } => {
                    body=body.child(div().text_xl().child(title)).child("원격 대상 삭제는 해당 프로세스를 종료합니다. 로컬 호스트 삭제는 저장된 연결 정보만 삭제합니다.");
                    footer =
                        footer.child(
                            self.button("confirm-delete", "삭제", cx, |s, _, _| s.confirm()),
                        );
                }
                Dialog::Settings => {
                    body = body
                        .child(div().text_xl().child("설정"))
                        .child(self.shortcut_settings(cx));
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
                        .child(platform::STORAGE_LABEL)
                        .child(platform::SHORTCUT_HELP)
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
                    .id("modal-overlay")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
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
                            .border_1()
                            .border_color(rgb(self.palette.line))
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
                    .flex()
                    .gap_3()
                    .child(div().flex_1().child(error))
                    .child(self.button("dismiss-error", "닫기", cx, |s, _, _| s.error = None)),
            );
        }
        if self.dialog.is_some() {
            self.modal.finish(window);
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
    let path = platform::data_dir()
        .expect("application data directory unavailable")
        .join("settings.json");
    Application::new()
        .with_assets(launcher::Assets)
        .run(move |cx: &mut App| {
            text_input::bind_keys(cx);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new(platform::QUIT_KEY, Quit, None)]);
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
                move |window, cx| {
                    #[cfg(target_os = "windows")]
                    windows_ime::install(window).expect("Windows IME handler unavailable");
                    #[cfg(not(target_os = "windows"))]
                    let _ = window;
                    cx.new(|cx| Desktop::new(runtime, path, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
