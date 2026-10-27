#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod accent_ui;
mod agent_ui;
mod appearance;
mod cloud_sync;
mod composer;
mod file_picker;
mod hierarchy;
mod host_dialog;
mod host_order;
mod i18n;
mod key_file;
mod launcher;
mod modal;
mod native_tooltips;
mod notification_panel;
mod notifications;
mod pane_toolbar;
mod platform;
mod resume;
mod session_tabs;
mod settings;
mod snackbar;
mod storage;
mod terminal;
mod text_input;
mod theme;
mod ui_glyph;
mod ui_icons;
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
use dgl_core::{i18n::t, tr};
use gpui::{prelude::*, *};
use native_tooltips::NativeTooltip;
use snackbar::{SnackbarDuration, SnackbarState};
use std::collections::{HashMap, HashSet};
use std::{cell::RefCell, rc::Rc};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use terminal::WindowTerminal as Terminal;
use text_input::TextInput;
use ui_glyph::ButtonLabel;
use ui_icons::UiIcon;
use workspace::{Axis, GroupId, Layout, TabId, Workspace};
actions!(dgl, [Quit]);
struct Tab {
    id: TabId,
    refresh: Option<u64>,
    host: HostProfile,
    connection: Arc<Connection>,
    terminal: Entity<Terminal>,
    last_sequence: u64,
    session: Option<String>,
    session_title: Option<String>,
    pending_target: Option<session_tabs::PendingTarget>,
}
#[derive(Clone)]
enum PromptOp {
    Session,
    Window(String),
    RenameSession(String),
    RenameWindow(String),
    RenamePane(String),
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
        feedback: host_dialog::Feedback,
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
    Shortcuts,
    Agents(Arc<Connection>, String, Option<String>),
    Notifications,
    Compose(TabId),
}
struct Desktop {
    runtime: Arc<tokio::runtime::Runtime>,
    path: PathBuf,
    local: LocalState,
    tabs: Vec<Tab>,
    workspace: Workspace,
    host_order: host_order::HostOrder,
    last_host_tab: HashMap<String, Vec<TabId>>,
    palette: theme::Palette,
    appearance: Option<appearance::Appearance>,
    accent_input: Entity<TextInput>,
    accent_error: bool,
    modal: modal::Modal,
    expanded: HashSet<String>,
    menu: Option<String>,
    launcher: launcher::LaunchMenu,
    group_bounds: HashMap<GroupId, Bounds<Pixels>>,
    shortcut_target: Option<(GroupId, Option<TabId>)>,
    settings: settings::SettingsState,
    split_bounds: HashMap<Vec<bool>, Bounds<Pixels>>,
    resizing: Option<Vec<bool>>,
    dragging_tab: Option<TabId>,
    dragging_host: Option<String>,
    host_drop: Option<Option<String>>,
    dialog: Option<Dialog>,
    composers: HashMap<TabId, composer::Composer>,
    snackbar: SnackbarState,
    focus: FocusHandle,
    inbox: dgl_core::agent::Inbox,
    resume: resume::ResumeDetector,
    agent_tick: u64,
    notification_status: String,
    notification_anchor: Rc<RefCell<Bounds<Pixels>>>,
    cloud_sync: cloud_sync::CloudSync,
    #[cfg(debug_assertions)]
    notification_probe_at: Option<u64>,
}
impl Desktop {
    fn new(runtime: Arc<tokio::runtime::Runtime>, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let (local, error) = match LocalState::read(&path) {
            Ok(s) => (s, None),
            Err(e) => (
                LocalState::default(),
                Some(tr!(
                    "설정을 읽지 못했습니다: {0}. 원본 파일을 보존했습니다.",
                    e
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
        notifications::initialize();
        if local.preferences.agent_notifications.os {
            notifications::authorize();
        }
        #[cfg(debug_assertions)]
        let notification_probe_at = (std::env::var("DGL_NOTIFICATION_PROBE").as_deref() == Ok("1"))
            .then(|| {
                notifications::authorize();
                dgl_core::agent::now_ms() + 20_000
            });
        let workspace = Workspace::new(Layout::load(&path.with_file_name("workspace-layout.json")));
        let host_order =
            host_order::HostOrder::load(&path.with_file_name("host-order.json"), &local.hosts);
        let mut cloud_sync = cloud_sync::CloudSync::load(&path.with_file_name("cloud-sync.json"));
        if error.is_some() {
            cloud_sync.block_invalid_local();
        }
        let accent_value = local
            .preferences
            .accent
            .custom_color
            .as_deref()
            .unwrap_or("#2468C9");
        let accent_input = cx.new(|cx| TextInput::new(accent_value, "#RRGGBB", false, cx));
        cx.observe(&accent_input, |s, _, cx| {
            s.accent_error = false;
            cx.notify();
        })
        .detach();
        let mut snackbar = SnackbarState::default();
        if let Some(error) = error {
            snackbar.show_snackbar_with_duration(error, SnackbarDuration::Long);
        }
        Self {
            runtime,
            path,
            local,
            tabs: vec![],
            workspace,
            host_order,
            last_host_tab: HashMap::new(),
            palette: theme::Palette::new(true),
            appearance: None,
            accent_input,
            accent_error: false,
            modal: modal::Modal::default(),
            expanded: HashSet::new(),
            menu: None,
            launcher: launcher::LaunchMenu::default(),
            group_bounds: HashMap::new(),
            shortcut_target: None,
            settings: settings::SettingsState::default(),
            split_bounds: HashMap::new(),
            resizing: None,
            dragging_tab: None,
            dragging_host: None,
            host_drop: None,
            dialog: None,
            composers: HashMap::new(),
            snackbar,
            focus: cx.focus_handle(),
            inbox: Default::default(),
            resume: Default::default(),
            agent_tick: 0,
            notification_status: String::new(),
            notification_anchor: Default::default(),
            cloud_sync,
            #[cfg(debug_assertions)]
            notification_probe_at,
        }
    }
    fn sync_title_updates(&self, foreground: bool) {
        for tab in &self.tabs {
            tab.connection.set_title_updates_enabled(
                self.workspace.title_updates_enabled(tab.id, foreground),
            );
        }
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        let foreground = cx.windows().into_iter().any(|h| {
            cx.update_window(h, |_, w, _| w.is_window_active())
                .unwrap_or(false)
        });
        self.sync_title_updates(foreground);
        if self.resume.update(foreground, SystemTime::now()) {
            let mut checked = HashSet::new();
            for tab in &self.tabs {
                if checked.insert(tab.connection.state().handle) {
                    let _ = tab.connection.action(Action::CheckConnection);
                }
            }
        }
        #[cfg(debug_assertions)]
        if self
            .notification_probe_at
            .is_some_and(|at| dgl_core::agent::now_ms() >= at)
        {
            self.notification_probe_at = None;
            let notice = dgl_core::agent::Notice {
                endpoint: "native-probe".into(),
                read: false,
                event: dgl_core::agent::AgentEvent {
                    id: "native-probe".into(),
                    seq: 1,
                    server: "probe".into(),
                    run: "probe".into(),
                    actor: "parent".into(),
                    session: "$0".into(),
                    window: "@0".into(),
                    pane: "%0".into(),
                    agent: dgl_core::agent::Agent::Codex,
                    kind: dgl_core::agent::Kind::Response,
                    event: "Stop (native notification probe)".into(),
                    at: dgl_core::agent::now_ms(),
                },
            };
            self.inbox.recent.push_front(notice.clone());
            eprintln!(
                "DGL_NOTIFICATION_PROBE_REQUESTED={}",
                notifications::show(&notice)
            );
            cx.notify();
        }
        self.tick_shortcut_drag(cx);
        self.tick_agents(cx);
        self.tick_cloud_sync(cx);
        let mut changed = false;
        let mut save_hosts = false;
        for tab in &mut self.tabs {
            tab.terminal.update(cx, |t, cx| t.tick(cx));
            let state = tab.connection.state();
            tab.update_session(&state);
            if let Some(target) = tab.pending_target.take() {
                if target.generation == state.generation.connection {
                    if matches!(
                        state.phase,
                        Phase::Selecting | Phase::Ready | Phase::MissingSession
                    ) {
                        match tab.connection.action(target.action()) {
                            Ok(_) => {
                                if let Some(operation) = target.operation
                                    && let Err(e) =
                                        tab.connection.action(Action::Manage { operation })
                                {
                                    self.snackbar.show_snackbar(tr!("요청 실패: {0}", e));
                                }
                            }
                            Err(e) => self.snackbar.show_snackbar(tr!("대상 선택 실패: {0}", e)),
                        }
                    } else if matches!(
                        state.phase,
                        Phase::Connecting
                            | Phase::TrustRequired
                            | Phase::Authenticating
                            | Phase::Restoring
                    ) {
                        tab.pending_target = Some(target);
                    }
                }
            }
            if state.sequence != tab.last_sequence {
                tab.last_sequence = state.sequence;
                changed = true;
                if state.phase == Phase::Ready
                    && let Some(host) = self.local.hosts.iter_mut().find(|h| h.id == tab.host.id)
                {
                    if host.last_session != state.session || host.last_pane != state.pane {
                        save_hosts = true;
                    }
                    host.last_session = state.session.clone();
                    host.last_pane = state.pane.clone();
                    host.last_connected = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                }
            }
            for result in tab.connection.completions() {
                if composer::complete(
                    &mut self.composers,
                    &mut self.dialog,
                    tab,
                    self.workspace.selected() == Some(tab.id),
                    &result,
                    cx,
                ) {
                    changed = true;
                    continue;
                }
                if tab.refresh == Some(result.request_id) {
                    tab.refresh = None;
                    changed = true;
                }
                let input = tab.terminal.update(cx, |t, cx| {
                    t.complete(result.request_id, result.error.is_none(), cx)
                });
                if let Some(e) = result.error
                    && (input
                        || !matches!(
                            e,
                            dgl_core::Error::Cancelled | dgl_core::Error::StaleGeneration
                        ))
                {
                    if input {
                        tab.terminal.update(cx, |t, _| t.reject_input(e));
                    } else {
                        self.snackbar.show_snackbar_with_duration(
                            tr!("요청 실패: {0}", e),
                            SnackbarDuration::Normal,
                        );
                    }
                    changed = true;
                }
            }
        }
        for tab in &self.tabs {
            if let Some(error) = tab.terminal.update(cx, |t, _| t.notice.take()) {
                self.snackbar
                    .show_snackbar_with_duration(error.message, error.duration);
                changed = true;
            }
        }
        changed |= self.snackbar.tick(std::time::Instant::now());
        if changed {
            if save_hosts && let Err(e) = self.local.save(&self.path) {
                self.snackbar.show_snackbar_with_duration(
                    tr!("설정 저장 실패: {0}", e),
                    SnackbarDuration::Long,
                );
            }
            cx.notify();
        }
    }
    fn save(&mut self) -> bool {
        self.reconcile_hosts();
        match self.local.save(&self.path) {
            Ok(()) => true,
            Err(e) => {
                self.snackbar.show_snackbar_with_duration(
                    tr!("설정을 저장하지 못했습니다: {0}", e),
                    SnackbarDuration::Long,
                );
                false
            }
        }
    }
    fn open(&mut self, host: HostProfile, cx: &mut Context<Self>) {
        if let Some(id) = self.host_tab(&host.id) {
            self.activate_tab(id);
            return;
        }
        self.start_tab(host, cx);
    }
    fn start_tab(&mut self, host: HostProfile, cx: &mut Context<Self>) -> Option<TabId> {
        if matches!(storage::read_optional(&host.credential_ref), Ok(None)) {
            self.edit(Some(host), cx);
            self.snackbar.show_snackbar_with_duration(
                t("이 기기에서 사용할 비밀번호 또는 개인키를 등록하세요."),
                SnackbarDuration::Long,
            );
            cx.notify();
            return None;
        }
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
        let id = match result {
            Ok((credential, trusted)) => Some(self.create_tab(host, credential, trusted, cx)),
            Err(e) => {
                self.snackbar.show_snackbar_with_duration(
                    tr!("보호 저장소를 열지 못했습니다: {0}", e),
                    SnackbarDuration::Long,
                );
                None
            }
        };
        cx.notify();
        id
    }
    fn active_tab(&self) -> Option<&Tab> {
        let id = self.workspace.selected()?;
        self.tabs.iter().find(|t| t.id == id)
    }
    fn close_tab(&mut self, id: TabId) {
        self.composers.remove(&id);
        if matches!(self.dialog, Some(Dialog::Compose(tab)) if tab == id) {
            self.dialog = None;
        }
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            let tab = self.tabs.remove(index);
            self.workspace.close(id);
            if let Some(history) = self.last_host_tab.get_mut(&tab.host.id) {
                history.retain(|t| *t != id);
            }
            tab.connection.close();
            self.runtime.spawn(async move {
                tab.connection.join().await;
            });
            if let Some(selected) = self.workspace.selected() {
                self.activate_tab(selected);
            }
        }
    }
    fn reconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            let mut host = self
                .local
                .hosts
                .iter()
                .find(|h| h.id == tab.host.id)
                .cloned()
                .unwrap_or_else(|| tab.host.clone());
            let state = tab.connection.state();
            host.last_session = tab.session.clone();
            host.last_pane = if state.session == tab.session {
                state.pane.or(host.last_pane)
            } else {
                None
            };
            let session_title = tab.session_title.clone();
            let id = tab.id;
            // Keep the slot and its group, even when a manual reconnect replaces SSH.
            let group = self.workspace.group_for(id).unwrap();
            let before = self.workspace.groups[&group]
                .tabs
                .iter()
                .position(|t| *t == id)
                .and_then(|index| self.workspace.groups[&group].tabs.get(index + 1).copied());
            if let Some(new) = self.start_tab(host, cx) {
                let mut replacement = self.tabs.pop().unwrap();
                debug_assert_eq!(replacement.id, new);
                self.workspace.close(new);
                for history in self.last_host_tab.values_mut() {
                    history.retain(|t| *t != new);
                }
                self.close_tab(id);
                replacement.id = id;
                replacement.session_title = session_title;
                self.tabs.push(replacement);
                // Reinsert the original ID in exactly the same group and position.
                self.workspace.groups.get_mut(&group).unwrap().tabs.push(id);
                self.workspace.move_tab(id, group, before);
                self.activate_tab(id);
            }
        }
    }
    fn save_layout(&mut self) {
        if let Err(e) = self
            .workspace
            .layout
            .save(&self.path.with_file_name("workspace-layout.json"))
        {
            self.snackbar
                .show_snackbar_with_duration(tr!("배치 저장 실패: {0}", e), SnackbarDuration::Long);
        }
    }
    fn refresh(&mut self) {
        if let Some(id) = self.workspace.selected()
            && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && tab.refresh.is_none()
        {
            match tab.connection.action(Action::Refresh) {
                Ok(request) => tab.refresh = Some(request),
                Err(e) => self.snackbar.show_snackbar(tr!("조회 실패: {0}", e)),
            }
        }
    }
    fn prompt(&mut self, title: &str, operation: PromptOp, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            self.dialog = Some(Dialog::Prompt {
                title: title.into(),
                connection: tab.connection.clone(),
                operation,
                input: cx.new(|cx| TextInput::new("", t("이름"), false, cx)),
            });
        }
    }
    fn submit(&mut self, connection: &Arc<Connection>, action: Action) {
        if let Err(e) = connection.action(action) {
            self.snackbar.show_snackbar(tr!("요청 실패: {0}", e));
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
                self.snackbar.show_snackbar(tr!("삭제 실패: {0}", e))
            } else {
                self.dialog = None;
            }
        }
    }
    fn button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<ButtonLabel>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let button = self.button_base(id, cx, action);
        match label.into() {
            ButtonLabel::Text(text) => button.child(text),
            ButtonLabel::Icon(icon) => button
                .flex()
                .items_center()
                .justify_center()
                .native_tooltip(icon.description())
                .child(
                    div()
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .child(icon.render(self.palette.fg)),
                ),
            ButtonLabel::IconText(icon, text) => button
                .flex()
                .items_center()
                .gap_1()
                .child(icon.render(
                    if matches!(
                        icon,
                        UiIcon::RadioSelected | UiIcon::CheckboxChecked | UiIcon::Star
                    ) {
                        self.palette.accent
                    } else {
                        self.palette.fg
                    },
                ))
                .child(text),
        }
    }
    fn button_base(
        &self,
        id: impl Into<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        self.styled_button(id, cx, action, false)
    }
    fn styled_button(
        &self,
        id: impl Into<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        bottom: bool,
    ) -> Stateful<Div> {
        let id = id.into();
        let palette = self.palette;
        let focus = self.modal.building.then(|| self.modal.button(&id, cx));
        let reveal = focus
            .clone()
            .filter(|_| id != "cancel-dialog" && id != "save-host" && id != "close-notifications");
        div()
            .when_some(reveal, |d, focus| {
                d.on_children_prepainted(self.modal.reveal_on_focus(focus))
            })
            .id(ElementId::Name(id.clone()))
            .debug_selector(|| id.to_string())
            .px_2()
            .py_1()
            .flex_shrink_0()
            .cursor_pointer()
            .hover(move |s| {
                let s = s.bg(rgb(palette.selection)).text_color(rgb(palette.fg));
                if bottom {
                    s.border_color(rgb(palette.selection))
                } else {
                    s
                }
            })
            .when_some(focus, |d, focus| d.track_focus(&focus))
            .when(bottom, |d| d.focusable())
            .focus(move |s| {
                s.bg(rgb(palette.selection))
                    .text_color(rgb(palette.fg))
                    .border_color(rgb(if bottom {
                        palette.selection
                    } else {
                        palette.accent
                    }))
            })
            .border_1()
            .border_color(if bottom {
                rgb(palette.bg).into()
            } else {
                gpui::transparent_black()
            })
            .when(bottom, |d| d.bg(rgb(palette.bg)))
            .on_click(cx.listener(move |v, _, w, cx| {
                action(v, w, cx);
                cx.stop_propagation();
                cx.notify();
            }))
    }
    fn input_button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        self.styled_button(id, cx, action, true).child(label.into())
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
            PromptOp::RenamePane(target) => Operation::RenamePane { target, name },
        };
        match connection.action(Action::Manage { operation }) {
            Ok(_) => self.dialog = None,
            Err(e) => self
                .snackbar
                .show_snackbar(tr!("이름을 확인하세요: {0}", e)),
        }
    }

    fn theme(&mut self, value: &str, cx: &mut Context<Self>) {
        self.local.preferences.theme = value.into();
        self.save();
        cx.notify();
    }
    fn language(&mut self, value: &str, cx: &mut Context<Self>) {
        self.local.preferences.language = value.into();
        dgl_core::i18n::configure(value, &i18n::system_locale());
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
        let dark = self.appearance.as_ref().map_or(dark, |a| a.dark);
        self.palette = theme::Palette::with_accent(
            dark,
            &self.local.preferences.accent,
            self.appearance.as_ref().and_then(|a| a.accent),
        );
        self.sync_title_updates(window.is_window_active());
        self.sync_launcher(window, cx);
        let bg = self.palette.bg;
        if self.dialog.is_none() {
            self.modal.restore(window);
            if window.focused(cx).is_none() {
                window.focus(&self.focus);
            }
        }
        let shortcut_target = Some((self.workspace.active, self.workspace.selected()));
        let clear_modifiers = self.shortcut_target != shortcut_target
            || self.dialog.is_some()
            || self.launcher.opened.is_some()
            || !window.is_window_active();
        self.shortcut_target = shortcut_target;
        for tab in &self.tabs {
            tab.terminal.update(cx, |t, cx| {
                t.enabled = self.dialog.is_none();
                if clear_modifiers {
                    t.clear_modifiers(cx);
                }
            });
        }
        let mut root = self.render_workspace(window, cx);
        if let Some(dialog) = self.dialog.clone() {
            let (kind, inputs) = match &dialog {
                Dialog::Host {
                    fields, key_auth, ..
                } => (
                    "host",
                    if *key_auth {
                        vec![0, 1, 2, 3, 6, 5]
                    } else {
                        vec![0, 1, 2, 3, 4]
                    }
                    .into_iter()
                    .map(|i| fields[i].clone())
                    .collect::<Vec<_>>(),
                ),
                Dialog::Prompt { input, .. } => ("prompt", vec![input.clone()]),
                Dialog::Delete { .. } => ("delete", vec![]),
                Dialog::Settings => ("settings", vec![]),
                Dialog::Shortcuts => ("shortcuts", vec![]),
                Dialog::Agents(..) => ("agents", vec![]),
                Dialog::Notifications => ("notifications", vec![]),
                Dialog::Compose(id) => ("compose", vec![self.composers[id].input.clone()]),
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
            if matches!(dialog, Dialog::Notifications) {
                root = root.child(self.render_notification_panel(window, cx));
            } else {
                let mut body = div()
                    .id("dialog-body")
                    .debug_selector(|| "dialog-body".into())
                    .track_scroll(&self.modal.scroll)
                    .overflow_y_scroll()
                    .max_h(px(
                        (f32::from(window.viewport_size().height) - 180.).clamp(100., 620.)
                    ))
                    .flex()
                    .flex_col()
                    .gap_3();
                let mut footer = div().flex().gap_2().justify_end().child(self.button(
                    "cancel-dialog",
                    if matches!(&dialog, Dialog::Shortcuts) {
                        t("뒤로")
                    } else if matches!(&dialog, Dialog::Compose(_)) {
                        t("취소")
                    } else {
                        t("취소 / 닫기")
                    },
                    cx,
                    |s, _, cx| s.dismiss_dialog(cx),
                ));
                match dialog {
                    dialog @ Dialog::Host { .. } => {
                        body = body.child(self.render_host_dialog(dialog, cx));
                        footer = footer.child(self.button(
                            "save-host",
                            t("저장"),
                            cx,
                            |s, _, cx| s.save_host(cx),
                        ));
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
                        footer = footer.child(self.button(
                            "accept-name",
                            t("확인"),
                            cx,
                            |s, _, cx| s.accept_name(cx),
                        ));
                    }

                    Dialog::Delete { title, operation } => {
                        let warning = match operation {
                            DeleteOp::Host(_) => {
                                t("저장된 연결 정보를 삭제합니다. 원격 작업은 유지됩니다.")
                            }
                            DeleteOp::Remote(..) => t("삭제하면 해당 원격 프로세스가 종료됩니다."),
                            DeleteOp::Trust(_) => t("다음 연결 시 서버 키를 다시 확인해야 합니다."),
                        };
                        body = body.child(div().text_xl().child(title)).child(warning);
                        footer = footer.child(self.button(
                            "confirm-delete",
                            t("삭제"),
                            cx,
                            |s, _, _| s.confirm(),
                        ));
                    }
                    Dialog::Agents(connection, alias, pane) => {
                        body =
                            body.child(self.agent_details(connection, alias, pane.as_deref(), cx));
                    }
                    Dialog::Notifications => unreachable!(),
                    Dialog::Compose(id) => {
                        body = body.child(self.render_composer(id, cx));
                        let composer = &self.composers[&id];
                        let enabled = composer.pending.is_none()
                            && !composer.input.read(cx).content.is_empty();
                        footer = footer.child(
                            self.button(
                                "send-composer",
                                if composer.pending.is_some() {
                                    t("전송 중…")
                                } else {
                                    t("전송")
                                },
                                cx,
                                |s, _, cx| s.send_composer(cx),
                            )
                            .when(!enabled, |d| d.opacity(0.35).cursor_default()),
                        );
                    }
                    Dialog::Settings => {
                        body = body.child(self.settings_main(cx));
                    }
                    Dialog::Shortcuts => {
                        body = body.child(self.shortcut_settings(cx));
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
                                .w(px(
                                    if matches!(
                                        self.dialog,
                                        Some(Dialog::Settings | Dialog::Shortcuts)
                                    ) {
                                        720.
                                    } else {
                                        520.
                                    },
                                ))
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
        }
        if let Some(snackbar) = self.snackbar.current.clone() {
            let id = snackbar.id;
            let remaining = snackbar.remaining_fraction(std::time::Instant::now());
            root = root.child(
                div()
                    .id("snackbar")
                    .debug_selector(|| "snackbar".into())
                    .absolute()
                    .bottom_4()
                    .left_4()
                    .right_4()
                    .bg(rgb(0x682f38))
                    .text_color(rgb(0xffffff))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().child(snackbar.request.message))
                            .child(self.button(
                                "dismiss-error",
                                t("닫기"),
                                cx,
                                move |s, _, _| s.snackbar.dismiss(id),
                            )),
                    )
                    .when_some(remaining, |bar, fraction| {
                        bar.child(
                            div()
                                .id("snackbar-gauge-track")
                                .debug_selector(|| "snackbar-gauge-track".into())
                                .w_full()
                                .h(px(3.))
                                .bg(rgba(0xffffff33))
                                .child(
                                    div()
                                        .id("snackbar-gauge")
                                        .debug_selector(|| "snackbar-gauge".into())
                                        .h_full()
                                        .w(relative(fraction))
                                        .bg(rgb(0xffffff)),
                                ),
                        )
                    }),
            );
        }
        if self.dialog.is_some() {
            if self.settings.restore && matches!(self.dialog, Some(Dialog::Settings)) {
                self.settings.restore = false;
                self.modal.scroll.set_offset(self.settings.main_offset());
                self.modal.restore_button("edit-shortcuts", window);
            }
            self.modal.finish(window);
        }
        native_tooltips::root(root)
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
    let language = LocalState::read(&path)
        .unwrap_or_default()
        .preferences
        .language;
    dgl_core::i18n::configure(&language, &i18n::system_locale());
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
                        title: Some("DaGaLi".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    #[cfg(target_os = "windows")]
                    windows_ime::install(window).expect("Windows IME handler unavailable");
                    #[cfg(not(target_os = "windows"))]
                    let _ = window;
                    cx.new(|cx| {
                        let mut desktop = Desktop::new(runtime, path, cx);
                        desktop.init_appearance(window, cx);
                        desktop
                    })
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
