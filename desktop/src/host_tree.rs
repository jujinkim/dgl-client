//! The sidebar lists every host with its own collapsible session tree. A host's
//! sessions come from one of its tabs' connections; an expanded host without a
//! listing tab opens a short-lived probe connection that only lists sessions.
use crate::workspace::TabId;
use dgl_core::connection::{Connection, State};
use dgl_core::model::HostProfile;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A probe nobody touched for this long hands its last listing to the cache and
/// closes, so a forgotten expanded host does not hold SSH open.
pub(crate) const PROBE_IDLE: Duration = Duration::from_secs(60);

/// The `expanded` key of a host row.
pub(crate) fn expand_key(host: &str) -> String {
    format!("host:{host}")
}

/// A tab-less connection that lists one host's sessions and never attaches.
pub(crate) struct Probe {
    pub host: HostProfile,
    pub connection: Option<Arc<Connection>>,
    /// The state the probe showed when it closed; rows stay visible but read-only.
    pub cached: Option<Arc<State>>,
    pub refresh: Option<u64>,
    pub created: Option<crate::session_tabs::CreatedSession>,
    pub used: Instant,
    pub last_sequence: u64,
}
impl Probe {
    pub fn new(host: HostProfile, connection: Arc<Connection>, now: Instant) -> Self {
        Self {
            host,
            connection: Some(connection),
            cached: None,
            refresh: None,
            created: None,
            used: now,
            last_sequence: 0,
        }
    }
    pub fn state(&self) -> Option<Arc<State>> {
        self.connection
            .as_ref()
            .map(|c| c.state())
            .or_else(|| self.cached.clone())
    }
}

/// Where a host's session tree comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Tab(TabId),
    Probe,
}

/// One tab of the host: its ID and whether its connection lists sessions now.
pub(crate) struct HostTab {
    pub id: TabId,
    pub browses: bool,
}

/// The connection that lists a host's sessions. A tab that browses wins (the active
/// one, then the most recently used, then any); a probe serves while no tab can; a
/// recovering tab still shows its cached rows when there is no probe.
pub(crate) fn pick_source(
    tabs: &[HostTab],
    active: Option<TabId>,
    history: &[TabId],
    probe: bool,
) -> Option<Source> {
    let preferred = || {
        active
            .into_iter()
            .chain(history.iter().rev().copied())
            .chain(tabs.iter().map(|t| t.id))
            .filter_map(|id| tabs.iter().find(|t| t.id == id))
    };
    if let Some(tab) = preferred().find(|t| t.browses) {
        return Some(Source::Tab(tab.id));
    }
    if probe {
        return Some(Source::Probe);
    }
    preferred().next().map(|t| Source::Tab(t.id))
}

/// Whether a probe's connection should close now. It serves only an expanded host
/// that no tab lists, and only while it is in use.
pub(crate) fn probe_expires(expanded: bool, tab_lists: bool, used: Instant, now: Instant) -> bool {
    !expanded || tab_lists || now.duration_since(used) >= PROBE_IDLE
}

/// A probe never attaches: the host's remembered target is dropped so the connection
/// stops at session selection.
pub(crate) fn probe_host(host: &HostProfile) -> HostProfile {
    let mut host = host.clone();
    host.last_session = None;
    host.last_pane = None;
    host
}

/// The rows a host's tree shows and the connection they act through. Without a
/// connection the rows are a closed probe's last listing: they open tabs only.
pub(crate) struct TreeSource {
    pub connection: Option<Arc<Connection>>,
    pub state: Arc<State>,
    pub refreshing: bool,
}

impl crate::Desktop {
    fn host_tabs(&self, host: &str) -> Vec<HostTab> {
        self.tabs
            .iter()
            .filter(|t| t.host.id == host)
            .map(|t| HostTab {
                id: t.id,
                browses: t.connection.state().browses_sessions(),
            })
            .collect()
    }
    pub(crate) fn host_expanded(&self, host: &str) -> bool {
        self.expanded.contains(&expand_key(host))
    }
    pub(crate) fn tree_source(&self, host: &str) -> Option<TreeSource> {
        let tabs = self.host_tabs(host);
        let active = self
            .active_tab()
            .filter(|t| t.host.id == host)
            .map(|t| t.id);
        let history = self
            .last_host_tab
            .get(host)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let probe = self.probes.get(host);
        match pick_source(&tabs, active, history, probe.is_some())? {
            Source::Tab(id) => {
                let tab = self.tabs.iter().find(|t| t.id == id)?;
                Some(TreeSource {
                    connection: Some(tab.connection.clone()),
                    state: tab.connection.state(),
                    refreshing: tab.refresh.is_some(),
                })
            }
            Source::Probe => {
                let probe = probe?;
                Some(TreeSource {
                    connection: probe.connection.clone(),
                    state: probe.state()?,
                    refreshing: probe.refresh.is_some(),
                })
            }
        }
    }
    /// Expanding a host that no tab lists starts its probe; collapsing ends it.
    pub(crate) fn toggle_host(&mut self, host: &HostProfile, cx: &mut gpui::Context<Self>) {
        let key = expand_key(&host.id);
        if self.expanded.remove(&key) {
            if let Some(probe) = self.probes.remove(&host.id)
                && let Some(connection) = probe.connection
            {
                self.release(connection);
            }
            return;
        }
        self.expanded.insert(key);
        // A host's tab lists it, or shows its cached rows while it recovers.
        if self.host_tabs(&host.id).is_empty() {
            self.start_probe(host, cx);
        }
    }
    fn start_probe(&mut self, host: &HostProfile, cx: &mut gpui::Context<Self>) {
        if let Some(probe) = self.probes.remove(&host.id)
            && let Some(connection) = probe.connection
        {
            self.release(connection);
        }
        let Some((credential, trusted)) = self.credentials(host, cx) else {
            // The host editor opened for the missing credential; nothing to list.
            self.expanded.remove(&expand_key(&host.id));
            return;
        };
        let host = probe_host(host);
        let _enter = self.runtime.enter();
        let connection = match crate::local_host::Target::of(&host) {
            crate::local_host::Target::Local => {
                Connection::start_local(dgl_core::local::LocalConfig::default())
            }
            crate::local_host::Target::Ssh(_) => {
                Connection::start(host.clone(), credential, trusted)
            }
        };
        self.probes.insert(
            host.id.clone(),
            Probe::new(host, connection, Instant::now()),
        );
    }
    /// Any use of a probe's rows keeps it open for another idle period.
    pub(crate) fn touch_probe(&mut self, host: &str) {
        if let Some(probe) = self.probes.get_mut(host) {
            probe.used = Instant::now();
        }
    }
    /// Refreshes the connection listing `host`; a closed probe reconnects.
    pub(crate) fn refresh_host(&mut self, host: &HostProfile, cx: &mut gpui::Context<Self>) {
        let source = self.tree_source(&host.id);
        let tab = source
            .as_ref()
            .and_then(|s| s.connection.as_ref())
            .and_then(|c| self.tabs.iter().position(|t| Arc::ptr_eq(&t.connection, c)));
        if let Some(index) = tab {
            let tab = &mut self.tabs[index];
            if tab.refresh.is_none() {
                match tab.connection.action(dgl_core::connection::Action::Refresh) {
                    Ok(request) => tab.refresh = Some(request),
                    Err(e) => self
                        .snackbar
                        .show_snackbar(dgl_core::tr!("app.fetch_failed", e)),
                }
            }
            return;
        }
        match self.probes.get_mut(&host.id) {
            Some(probe) if probe.connection.is_some() => {
                probe.used = Instant::now();
                let connection = probe.connection.clone().unwrap();
                if probe.refresh.is_none() {
                    match connection.action(dgl_core::connection::Action::Refresh) {
                        Ok(request) => probe.refresh = Some(request),
                        Err(e) => self
                            .snackbar
                            .show_snackbar(dgl_core::tr!("app.fetch_failed", e)),
                    }
                }
            }
            _ => self.start_probe(host, cx),
        }
    }
    /// Drains probe completions and closes idle probes. Returns whether anything shown
    /// changed, and the sessions created through a probe, to open as tabs.
    pub(crate) fn tick_probes(&mut self) -> (bool, Vec<(HostProfile, Arc<State>, String)>) {
        let now = Instant::now();
        let mut changed = false;
        let mut created = Vec::new();
        let hosts: Vec<String> = self.probes.keys().cloned().collect();
        for host in hosts {
            let exists =
                host == crate::local_host::ID || self.local.hosts.iter().any(|h| h.id == host);
            let expanded = self.host_expanded(&host);
            let tab_lists = self.host_tabs(&host).iter().any(|t| t.browses);
            let probe = self.probes.get_mut(&host).unwrap();
            if let Some(connection) = probe.connection.clone() {
                for result in connection.completions() {
                    if probe.refresh == Some(result.request_id) {
                        probe.refresh = None;
                        changed = true;
                    }
                    if let Some(c) = probe.created.take_if(|c| c.request == result.request_id)
                        && result.error.is_none()
                    {
                        let state = connection.state();
                        if let Some(session) = c.resolve(&state.remote.sessions) {
                            created.push((probe.host.clone(), state, session));
                        }
                    }
                }
                let state = connection.state();
                if state.sequence != probe.last_sequence {
                    probe.last_sequence = state.sequence;
                    changed = true;
                }
            }
            if !exists || !expanded || tab_lists {
                let probe = self.probes.remove(&host).unwrap();
                if let Some(connection) = probe.connection {
                    self.release(connection);
                }
                changed = true;
            } else if probe.connection.is_some()
                && probe_expires(expanded, tab_lists, probe.used, now)
            {
                let connection = probe.connection.take().unwrap();
                probe.cached = Some(connection.state());
                probe.refresh = None;
                probe.created = None;
                self.release(connection);
                changed = true;
            }
        }
        (changed, created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tab(id: TabId, browses: bool) -> HostTab {
        HostTab { id, browses }
    }
    #[test]
    fn a_browsing_tab_lists_before_a_probe_and_the_active_one_first() {
        let tabs = [tab(1, true), tab(2, true), tab(3, false)];
        assert_eq!(
            pick_source(&tabs, Some(2), &[1], true),
            Some(Source::Tab(2))
        );
        // The active tab recovers: the most recently used browsing tab serves.
        assert_eq!(
            pick_source(&tabs, Some(3), &[2, 1], true),
            Some(Source::Tab(1))
        );
        assert_eq!(pick_source(&tabs, None, &[], false), Some(Source::Tab(1)));
    }
    #[test]
    fn a_probe_serves_only_while_no_tab_can_list() {
        let tabs = [tab(3, false), tab(4, false)];
        assert_eq!(pick_source(&tabs, Some(4), &[3], true), Some(Source::Probe));
        // Without a probe the recovering tab keeps its cached rows on screen.
        assert_eq!(
            pick_source(&tabs, Some(4), &[3], false),
            Some(Source::Tab(4))
        );
        assert_eq!(pick_source(&tabs, None, &[3], false), Some(Source::Tab(3)));
        assert_eq!(pick_source(&[], None, &[], true), Some(Source::Probe));
        assert_eq!(pick_source(&[], None, &[], false), None);
    }
    #[test]
    fn probes_close_on_collapse_a_listing_tab_or_idleness() {
        let start = Instant::now();
        assert!(!probe_expires(true, false, start, start));
        assert!(!probe_expires(
            true,
            false,
            start,
            start + PROBE_IDLE - Duration::from_millis(1)
        ));
        assert!(probe_expires(true, false, start, start + PROBE_IDLE));
        assert!(probe_expires(false, false, start, start));
        assert!(probe_expires(true, true, start, start));
    }
    #[test]
    fn probes_drop_the_remembered_target() {
        let host = HostProfile {
            id: "a".into(),
            last_session: Some("$1".into()),
            last_pane: Some("%1".into()),
            ..sample()
        };
        let probe = probe_host(&host);
        assert_eq!((probe.last_session, probe.last_pane), (None, None));
        assert_eq!(probe.id, "a");
    }
    fn sample() -> HostProfile {
        HostProfile {
            id: String::new(),
            alias: String::new(),
            hostname: String::new(),
            port: 22,
            username: String::new(),
            auth: dgl_core::model::AuthKind::Password,
            credential_ref: String::new(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        }
    }
}
