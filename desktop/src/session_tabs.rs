use super::*;
use dgl_core::connection::State;

#[derive(Debug, PartialEq, Eq)]
enum Route {
    Existing(TabId),
    Bind(TabId),
    Create,
    Blocked,
}
fn route<'a>(
    tabs: impl IntoIterator<Item = (TabId, &'a str, Option<&'a str>)>,
    host: &str,
    session: &str,
    can_create: bool,
) -> Route {
    let mut unbound = None;
    for (id, tab_host, tab_session) in tabs {
        if tab_host != host {
            continue;
        }
        if tab_session == Some(session) {
            return Route::Existing(id);
        }
        if tab_session.is_none() {
            unbound = unbound.or(Some(id));
        }
    }
    if !can_create {
        Route::Blocked
    } else if let Some(id) = unbound {
        Route::Bind(id)
    } else {
        Route::Create
    }
}

pub(super) struct PendingTarget {
    pub session: String,
    pub window: Option<String>,
    pub pane: Option<String>,
    pub generation: u64,
    pub operation: Option<Operation>,
}
impl PendingTarget {
    pub fn action(&self) -> Action {
        Action::SelectTarget {
            session: self.session.clone(),
            window: self.window.clone(),
            pane: self.pane.clone(),
        }
    }
}
impl Tab {
    pub(super) fn update_session(&mut self, state: &State) {
        if self.session.is_none() && state.phase == Phase::Ready {
            self.session = state.session.clone();
        }
        if let Some(session) = state
            .remote
            .sessions
            .iter()
            .find(|s| Some(&s.id) == self.session.as_ref())
        {
            self.session_title = Some(hierarchy::session_tab_title(&state.remote, session));
        }
    }
    pub(super) fn label(&self) -> String {
        let alias = hierarchy::display_text(&self.host.alias);
        format!("{alias} · {}", self.title())
    }
    pub(super) fn title(&self) -> String {
        match self.session.as_deref() {
            Some(id) => hierarchy::display_text(self.session_title.as_deref().unwrap_or(id)),
            None => t("세션 선택").into(),
        }
    }
}
impl Desktop {
    pub(super) fn focus_active_terminal(&self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            let focus = tab.terminal.focus_handle(cx);
            for handle in cx.windows() {
                let _ = cx.update_window(handle, |_, w, _| w.focus(&focus));
            }
        }
    }
    pub(super) fn save_host_order(&mut self) {
        if let Err(e) = self
            .host_order
            .save(&self.path.with_file_name("host-order.json"))
        {
            self.error = Some(tr!("호스트 순서 저장 실패: {0}", e));
        }
    }
    pub(super) fn reconcile_hosts(&mut self) {
        if self.host_order.reconcile(&self.local.hosts) {
            self.save_host_order();
        }
        for tab in &mut self.tabs {
            if let Some(host) = self.local.hosts.iter().find(|h| h.id == tab.host.id) {
                tab.host.alias = host.alias.clone();
                tab.host.favorite = host.favorite;
            }
        }
    }
    pub(super) fn host_tab(&self, host: &str) -> Option<TabId> {
        self.last_host_tab
            .get(host)
            .and_then(|tabs| tabs.last())
            .copied()
            .filter(|id| self.tabs.iter().any(|t| t.id == *id && t.host.id == host))
            .or_else(|| {
                self.tabs
                    .iter()
                    .rev()
                    .find(|t| t.host.id == host)
                    .map(|t| t.id)
            })
    }
    pub(super) fn session_tab(&self, host: &str, session: &str) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|t| t.host.id == host && t.session.as_deref() == Some(session))
            .map(|t| t.id)
    }
    pub(super) fn activate_tab(&mut self, id: TabId) -> bool {
        let Some(group) = self.workspace.group_for(id) else {
            return false;
        };
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return false;
        };
        let history = self.last_host_tab.entry(tab.host.id.clone()).or_default();
        history.retain(|t| *t != id);
        history.push(id);
        self.workspace.select(group, id);
        true
    }
    pub(super) fn create_tab(
        &mut self,
        host: HostProfile,
        credential: Credential,
        trusted: Option<String>,
        cx: &mut Context<Self>,
    ) -> TabId {
        // Install the slot, start order and session identity in this synchronous UI
        // turn, before another selection can be dispatched.
        let id = self.workspace.add_tab();
        self.host_order.reconcile(&self.local.hosts);
        if self.host_order.connection_started(&host.id) {
            self.save_host_order();
        }
        let session = host.last_session.clone();
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
            id,
            host,
            connection,
            terminal,
            session,
            session_title: None,
            pending_target: None,
            refresh: None,
            last_sequence: 0,
        });
        self.activate_tab(id);
        id
    }

    /// A row is evidence from a specific connection generation. It cannot mutate a
    /// replacement connection. Activation remains available while either tab recovers.
    pub(super) fn select_session_tab(
        &mut self,
        source: &Arc<Connection>,
        generation: u64,
        session: &str,
        window: Option<String>,
        pane: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let source_tab = self
            .tabs
            .iter()
            .find(|t| Arc::ptr_eq(&t.connection, source))?;
        let state = source.state();
        if state.generation.connection != generation {
            return None;
        }
        let host = source_tab.host.clone();
        let browse = matches!(
            state.phase,
            Phase::Selecting | Phase::Ready | Phase::MissingSession
        );
        let route = route(
            self.tabs
                .iter()
                .map(|t| (t.id, t.host.id.as_str(), t.session.as_deref())),
            &host.id,
            session,
            browse && state.remote.sessions.iter().any(|s| s.id == session),
        );
        let existing = matches!(route, Route::Existing(_));
        let id = match route {
            Route::Blocked => return None,
            Route::Existing(id) => id,
            Route::Bind(id) => {
                let tab = self.tabs.iter_mut().find(|t| t.id == id).unwrap();
                // Reserve before submitting or waiting for a selection completion.
                tab.session = Some(session.into());
                let target_state = tab.connection.state();
                let target = PendingTarget {
                    session: session.into(),
                    window: window.clone(),
                    pane: pane.clone(),
                    generation: target_state.generation.connection,
                    operation: None,
                };
                if matches!(
                    target_state.phase,
                    Phase::Selecting | Phase::Ready | Phase::MissingSession
                ) {
                    if let Err(e) = tab.connection.action(target.action()) {
                        self.error = Some(tr!("대상 선택 실패: {0}", e));
                    }
                } else if matches!(
                    target_state.phase,
                    Phase::Connecting
                        | Phase::TrustRequired
                        | Phase::Authenticating
                        | Phase::Restoring
                ) {
                    tab.pending_target = Some(target);
                }
                tab.id
            }
            Route::Create => {
                let mut host = self
                    .local
                    .hosts
                    .iter()
                    .find(|h| h.id == host.id)
                    .cloned()
                    .unwrap_or(host);
                host.last_session = Some(session.into());
                host.last_pane = None;
                let id = self.start_tab(host, cx)?;
                if window.is_some() || pane.is_some() {
                    let tab = self.tabs.iter_mut().find(|t| t.id == id).unwrap();
                    tab.pending_target = Some(PendingTarget {
                        session: session.into(),
                        window: window.clone(),
                        pane: pane.clone(),
                        generation: tab.connection.state().generation.connection,
                        operation: None,
                    });
                }
                id
            }
        };
        // Re-selecting a session only activates it: preserve its window, pane and SSH.
        if existing && (window.is_some() || pane.is_some()) && browse {
            let tab = self.tabs.iter_mut().find(|t| t.id == id)?;
            let current = tab.connection.state();
            if matches!(
                current.phase,
                Phase::Selecting | Phase::Ready | Phase::MissingSession
            ) {
                if let Err(e) = tab.connection.action(Action::SelectTarget {
                    session: session.into(),
                    window,
                    pane,
                }) {
                    self.error = Some(tr!("대상 선택 실패: {0}", e));
                }
            }
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && let Some(remote) = state.remote.sessions.iter().find(|s| s.id == session)
        {
            tab.session_title = Some(hierarchy::session_tab_title(&state.remote, remote));
        }
        self.activate_tab(id);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::{Route, route};
    #[test]
    fn identity_reservation_reuses_sessions_across_hosts_and_blocks_creation_during_recovery() {
        let mut tabs = vec![(1, "a", None), (2, "b", Some("$1"))];
        assert_eq!(route(tabs.clone(), "a", "$1", true), Route::Bind(1));
        // Reservation happens before the actor receives selection, even if it fails.
        tabs[0].2 = Some("$1");
        for can_create in [false, true] {
            for _ in 0..8 {
                assert_eq!(
                    route(tabs.clone(), "a", "$1", can_create),
                    Route::Existing(1)
                );
                assert_eq!(
                    route(tabs.clone(), "b", "$1", can_create),
                    Route::Existing(2)
                );
            }
        }
        assert_eq!(route(tabs.clone(), "a", "$2", false), Route::Blocked);
        assert_eq!(route(tabs.clone(), "a", "$2", true), Route::Create);
        tabs.push((3, "a", Some("$2")));
        assert_eq!(route(tabs.clone(), "a", "$2", false), Route::Existing(3));
        tabs.remove(0);
        assert_eq!(route(tabs, "a", "$1", true), Route::Create);
    }
}
