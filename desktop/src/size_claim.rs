//! The tmux window follows the screen focused last (`Action::ClaimSize`).
//!
//! A focus gain claims every visible tab's window with `force`; user input claims
//! without it only after another client took the size (`State::displaced`). Claims
//! are best effort: a refusal or a failed completion is never reported to the user.
//! An automatic reconnect reattaches without taking the window, so a focus claim
//! refused while the connection was not attached is deferred until it is `Ready`,
//! for at most [`FOCUS_CLAIM_DEFER_LIMIT`].
use crate::workspace::TabId;
use dgl_core::Error;
use dgl_core::connection::{Action, Connection};
use dgl_core::model::HostProfile;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

/// How long a deferred focus claim stays valid. A later reconnect must not take the
/// window back from a screen the user moved to meanwhile; without the claim the
/// ignore-size reattach leaves this client `displaced`, so the next key claims it.
pub const FOCUS_CLAIM_DEFER_LIMIT: Duration = Duration::from_secs(15);

/// Claim requests in flight for one connection, shared by its viewport and panes,
/// each marked whether it is a focus claim (`force`).
#[derive(Clone, Default)]
pub struct Claims(Rc<RefCell<HashMap<u64, bool>>>);
impl Claims {
    /// Makes the connection's current tmux window follow this client's size.
    /// `Err(NotReady)` while the connection is not attached: see [`FocusClaims::defer`].
    pub fn force(&self, connection: &Connection) -> dgl_core::Result<u64> {
        let request = connection.action(Action::ClaimSize { force: true });
        self.record(&request, true);
        request
    }
    /// Called immediately before user input, never before an emulator's automatic
    /// reply. A cheap local check: no request is added between ordinary keys.
    pub fn before_input(&self, connection: &Connection) -> bool {
        input_claim_needed(connection.state().displaced, self.pending())
            && self.record(
                &connection.action(Action::ClaimSize { force: false }),
                false,
            )
    }
    fn record(&self, request: &dgl_core::Result<u64>, focus: bool) -> bool {
        // Detached, reconnecting or a plain shell has no window to claim.
        request
            .as_ref()
            .is_ok_and(|id| self.0.borrow_mut().insert(*id, focus).is_none())
    }
    #[cfg(test)]
    pub fn track(&self, id: u64, focus: bool) {
        self.0.borrow_mut().insert(id, focus);
    }
    pub fn pending(&self) -> bool {
        !self.0.borrow().is_empty()
    }
    /// `Some(focus)` when `id` was a claim; its outcome is consumed silently either way.
    pub fn complete(&self, id: u64) -> Option<bool> {
        self.0.borrow_mut().remove(&id)
    }
}

/// A claim already in flight clears `displaced` when it runs, so later keys of the
/// same burst must not queue another one between inputs.
pub fn input_claim_needed(displaced: bool, pending: bool) -> bool {
    displaced && !pending
}

/// The only failures expected of a pointer report queued behind a size claim in
/// flight: the claim changed the grid (`StaleGeneration`) or the screen is still
/// resizing (`NotReady`). Anything else (`QueueFull`, `Closed`, ...) is reported.
pub fn expected_report_error(error: Option<&Error>) -> bool {
    matches!(error, None | Some(Error::NotReady | Error::StaleGeneration))
}

/// A focus claim that failed because the transport was not there (refused while
/// detached, or lost or cancelled with the transport) is retried after reconnecting.
pub fn defers_focus_claim(error: &Error) -> bool {
    matches!(error, Error::NotReady | Error::Network | Error::Cancelled)
}

/// Detects when the visible tabs should take their windows' size: the app window
/// turning active, the focused tab changing while it is active, or a wake from
/// sleep while it is active (the user is back at this machine). Also holds the
/// focus claims deferred until their connection is attached again.
#[derive(Default)]
pub struct FocusClaims {
    active: bool,
    focused: Option<TabId>,
    /// Tabs whose focus claim waits for `Ready`. Only a return to `Ready` sends it,
    /// so a claim that keeps failing while attached is never repeated every frame.
    deferred: HashMap<TabId, Deferred>,
}
struct Deferred {
    /// Its connection has been seen outside `Ready` since the claim failed.
    unready: bool,
    /// When the claim was first deferred; it expires [`FOCUS_CLAIM_DEFER_LIMIT`] later.
    since: Instant,
}
impl FocusClaims {
    pub fn update(&mut self, active: bool, focused: Option<TabId>, resumed: bool) -> bool {
        let activated = active && !self.active;
        let moved = focused != self.focused;
        self.active = active;
        self.focused = focused;
        let event = active && (activated || moved || resumed);
        // A newer focus event claims again and defers anew; an inactive app is not
        // the screen the user operates.
        if event || !active {
            self.deferred.clear();
        }
        event
    }
    /// Defers `tab`'s failed focus claim at `now`. `ready`: its connection is `Ready`
    /// now. Deferring an already deferred claim keeps its original age.
    pub fn defer(&mut self, tab: TabId, ready: bool, now: Instant) {
        if self.active {
            let deferred = self.deferred.entry(tab).or_insert(Deferred {
                unready: false,
                since: now,
            });
            deferred.unready |= !ready;
        }
    }
    /// The deferred claims to send now, in `visible` order. `ready` is `None` for a
    /// closed tab, otherwise whether its connection is `Ready`. A deferred claim is
    /// dropped once its tab is hidden or closed, or once it is older than
    /// [`FOCUS_CLAIM_DEFER_LIMIT`] at `now`.
    pub fn due(
        &mut self,
        visible: &[TabId],
        now: Instant,
        ready: impl Fn(TabId) -> Option<bool>,
    ) -> Vec<TabId> {
        if !self.active {
            self.deferred.clear();
        }
        self.deferred.retain(|id, deferred| {
            visible.contains(id)
                && now.saturating_duration_since(deferred.since) <= FOCUS_CLAIM_DEFER_LIMIT
        });
        let mut due = Vec::new();
        for id in visible {
            let Some(Deferred { unready, .. }) = self.deferred.get_mut(id) else {
                continue;
            };
            match ready(*id) {
                None => {
                    self.deferred.remove(id);
                }
                Some(false) => *unready = true,
                Some(true) if *unready => {
                    self.deferred.remove(id);
                    due.push(*id);
                }
                Some(true) => {}
            }
        }
        due
    }
}

/// One tmux window: the server, the user (one tmux server each) and its window id,
/// which is unique per server. Two host profiles may name the same server, even by
/// different hostnames (an alias and an IP), so neither the profile id nor the typed
/// hostname identifies it when the host key is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerWindow {
    server: Server,
    username: String,
    window: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Server {
    Local,
    /// The connection's host key fingerprint.
    Key(String),
    /// The lowercase hostname and port, before a host key is known.
    Address(String, u16),
}
impl ServerWindow {
    /// `fingerprint` and `window` come from the same connection state snapshot.
    pub fn new(
        host: &HostProfile,
        fingerprint: Option<&str>,
        window: Option<&str>,
    ) -> Option<Self> {
        let server = if host.id == crate::local_host::ID {
            Server::Local
        } else {
            match fingerprint.filter(|f| !f.is_empty()) {
                Some(key) => Server::Key(key.to_owned()),
                None => Server::Address(host.hostname.trim().to_ascii_lowercase(), host.port),
            }
        };
        Some(Self {
            server,
            username: host.username.clone(),
            window: window?.to_owned(),
        })
    }
}

/// Visible tabs in claim order, the focused tab last. Claims on different
/// connections run on independent actors, so issue order does not decide which
/// lands last: a non-focused tab showing the focused tab's tmux window (`window`)
/// does not claim at all. Hidden tabs never claim.
pub fn claim_order<K: PartialEq>(
    visible: &[TabId],
    focused: Option<TabId>,
    window: impl Fn(TabId) -> Option<K>,
) -> Vec<TabId> {
    let focused_window = focused.and_then(&window);
    let mut order: Vec<TabId> = Vec::with_capacity(visible.len());
    for id in visible {
        if Some(*id) != focused
            && !order.contains(id)
            && (focused_window.is_none() || window(*id) != focused_window)
        {
            order.push(*id);
        }
    }
    order.extend(focused.filter(|id| visible.contains(id)));
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use dgl_core::model::{AuthKind, Credential};

    fn host(id: &str, hostname: &str, port: u16, username: &str) -> HostProfile {
        HostProfile {
            id: id.into(),
            alias: id.into(),
            hostname: hostname.into(),
            port,
            username: username.into(),
            auth: AuthKind::Password,
            credential_ref: id.into(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        }
    }

    #[test]
    fn input_claims_only_after_a_takeover_and_once_per_burst() {
        assert!(input_claim_needed(true, false));
        assert!(!input_claim_needed(false, false));
        assert!(!input_claim_needed(true, true));
        assert!(!input_claim_needed(false, true));
    }

    #[test]
    fn claims_record_only_submitted_requests_and_complete_once() {
        let claims = Claims::default();
        let shared = claims.clone();
        assert!(!claims.record(&Err(Error::NotReady), true));
        assert!(!claims.pending());
        assert!(claims.record(&Ok(7), true));
        assert!(claims.record(&Ok(9), false));
        assert!(shared.pending(), "viewport and panes share one set");
        assert_eq!(claims.complete(8), None);
        assert_eq!(shared.complete(7), Some(true), "a focus claim");
        assert_eq!(claims.complete(7), None);
        assert_eq!(claims.complete(9), Some(false), "an input claim");
        assert!(!claims.pending());
    }

    #[test]
    fn claims_without_an_attached_window_are_refused_silently() {
        // Unpolled runtime: the connection stays in its first phase.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = runtime.enter();
        let connection = Connection::start(
            host("fixture", "127.0.0.1", 1, "test"),
            Credential::default(),
            None,
        );
        let claims = Claims::default();
        assert!(!connection.state().displaced);
        assert!(!claims.before_input(&connection));
        assert_eq!(
            claims.force(&connection),
            Err(Error::NotReady),
            "not attached: deferred by the caller"
        );
        assert!(!claims.pending());
        connection.close();
        runtime.block_on(connection.join());
    }

    #[test]
    fn only_a_resize_race_is_silenced_for_a_report_queued_behind_a_claim() {
        assert!(expected_report_error(None));
        assert!(expected_report_error(Some(&Error::NotReady)));
        assert!(expected_report_error(Some(&Error::StaleGeneration)));
        for error in [
            Error::QueueFull,
            Error::Closed,
            Error::Network,
            Error::Cancelled,
            Error::InvalidArgument,
            Error::Protocol,
        ] {
            assert!(!expected_report_error(Some(&error)), "{error:?}");
        }
    }

    #[test]
    fn only_a_missing_transport_defers_a_failed_focus_claim() {
        for error in [Error::NotReady, Error::Network, Error::Cancelled] {
            assert!(defers_focus_claim(&error), "{error:?}");
        }
        for error in [
            Error::Protocol,
            Error::QueueFull,
            Error::Closed,
            Error::StaleGeneration,
            Error::InvalidArgument,
        ] {
            assert!(!defers_focus_claim(&error), "{error:?}");
        }
    }

    #[test]
    fn focus_claims_on_activation_edges_focus_moves_and_wakes_only() {
        let mut f = FocusClaims::default();
        assert!(
            !f.update(false, Some(1), false),
            "inactive windows never claim"
        );
        assert!(f.update(true, Some(1), false), "inactive -> active");
        assert!(!f.update(true, Some(1), false), "not every frame");
        assert!(
            f.update(true, Some(2), false),
            "another tab or group took focus"
        );
        assert!(f.update(true, None, false), "focus moved to an empty group");
        assert!(!f.update(true, None, false));
        assert!(
            !f.update(false, Some(3), false),
            "focus moves in the background"
        );
        assert!(!f.update(false, Some(3), false));
        assert!(f.update(true, Some(3), false), "reactivation claims again");
        assert!(f.update(true, Some(3), true), "a wake while active");
        assert!(!f.update(false, Some(3), true), "a wake in the background");
    }

    #[test]
    fn a_refused_focus_claim_is_sent_once_its_connection_is_ready_again() {
        let mut f = FocusClaims::default();
        let t0 = Instant::now();
        assert!(f.update(true, Some(1), false));
        f.defer(1, false, t0);
        let visible = [1, 2];
        assert!(
            f.due(&visible, t0, |_| Some(false)).is_empty(),
            "still connecting"
        );
        assert_eq!(f.due(&visible, t0, |_| Some(true)), vec![1]);
        assert!(f.due(&visible, t0, |_| Some(false)).is_empty(), "sent once");
        assert!(f.due(&visible, t0, |_| Some(true)).is_empty());

        // A claim lost while attached waits for a reconnect, never repeating per frame.
        f.defer(2, true, t0);
        assert!(f.due(&visible, t0, |_| Some(true)).is_empty());
        assert!(f.due(&visible, t0, |_| Some(false)).is_empty());
        assert_eq!(f.due(&visible, t0, |_| Some(true)), vec![2]);

        // Several deferred tabs come due in visible order.
        f.defer(2, false, t0);
        f.defer(1, false, t0);
        assert_eq!(f.due(&visible, t0, |_| Some(true)), vec![1, 2]);
    }

    #[test]
    fn deferred_focus_claims_drop_when_the_tab_or_app_stops_being_the_focus_target() {
        let mut f = FocusClaims::default();
        let t0 = Instant::now();
        assert!(f.update(true, Some(1), false));
        f.defer(1, false, t0);
        assert!(f.due(&[2], t0, |_| Some(true)).is_empty(), "hidden");
        assert!(
            f.due(&[1], t0, |_| Some(true)).is_empty(),
            "hidden once: dropped"
        );

        f.defer(1, false, t0);
        assert!(!f.update(false, Some(1), false));
        assert!(
            f.due(&[1], t0, |_| Some(true)).is_empty(),
            "app deactivated"
        );
        f.defer(1, false, t0);
        assert!(
            f.due(&[1], t0, |_| Some(true)).is_empty(),
            "inactive: never deferred"
        );

        assert!(f.update(true, Some(1), false));
        f.defer(1, false, t0);
        assert!(f.update(true, Some(2), false), "a newer focus change");
        assert!(f.due(&[1, 2], t0, |_| Some(true)).is_empty());

        f.defer(2, false, t0);
        assert!(f.due(&[2], t0, |_| None).is_empty(), "closed tab");
        assert!(f.due(&[2], t0, |_| Some(true)).is_empty());

        f.defer(2, false, t0);
        assert!(!f.update(true, Some(2), false));
        assert_eq!(
            f.due(&[2], t0, |_| Some(true)),
            vec![2],
            "no focus event: kept"
        );
    }

    #[test]
    fn a_deferred_focus_claim_expires_so_a_late_reconnect_does_not_steal_the_window() {
        let mut f = FocusClaims::default();
        let t0 = Instant::now();
        assert!(f.update(true, Some(1), false));
        f.defer(1, false, t0);
        let limit = t0 + FOCUS_CLAIM_DEFER_LIMIT;
        assert!(f.due(&[1], limit, |_| Some(false)).is_empty());
        assert_eq!(
            f.due(&[1], limit, |_| Some(true)),
            vec![1],
            "still valid at the limit"
        );

        f.defer(1, false, t0);
        f.defer(1, false, limit - Duration::from_secs(1));
        assert!(
            f.due(&[1], limit + Duration::from_millis(1), |_| Some(true))
                .is_empty(),
            "a reconnect after the limit; re-deferring kept the first age"
        );
        assert!(
            f.due(&[1], limit, |_| Some(true)).is_empty(),
            "an expired claim is dropped, not kept for later"
        );

        // Each claim ages on its own.
        f.defer(1, false, t0);
        f.defer(2, false, t0 + Duration::from_secs(10));
        assert_eq!(
            f.due(&[1, 2], t0 + Duration::from_secs(20), |_| Some(true)),
            vec![2]
        );
    }

    #[test]
    fn the_focused_visible_tab_claims_last() {
        let none = |_| None::<()>;
        assert_eq!(claim_order(&[1, 2, 3], Some(2), none), vec![1, 3, 2]);
        assert_eq!(claim_order(&[1, 2, 3], Some(3), none), vec![1, 2, 3]);
        assert_eq!(claim_order(&[1, 2], None, none), vec![1, 2]);
        assert_eq!(
            claim_order(&[1, 2], Some(9), none),
            vec![1, 2],
            "a hidden tab never claims"
        );
        assert_eq!(claim_order(&[2, 1, 2], Some(1), none), vec![2, 1]);
        assert!(claim_order(&[], Some(1), none).is_empty());
    }

    #[test]
    fn a_tab_showing_the_focused_tabs_tmux_window_does_not_claim() {
        let a = host("a", "Example.org", 22, "me");
        let alias = host("b", "example.org ", 22, "me");
        let other_user = host("c", "example.org", 22, "you");
        let other_port = host("d", "example.org", 2222, "me");
        let windows: HashMap<TabId, Option<ServerWindow>> = [
            (1, ServerWindow::new(&a, None, Some("@1"))),
            (2, ServerWindow::new(&alias, None, Some("@1"))),
            (3, ServerWindow::new(&a, None, Some("@2"))),
            (4, ServerWindow::new(&other_user, None, Some("@1"))),
            (5, ServerWindow::new(&other_port, None, Some("@1"))),
            (6, ServerWindow::new(&a, None, None)),
            (7, ServerWindow::new(&a, None, None)),
        ]
        .into();
        let window = |id| windows[&id].clone();
        assert_eq!(
            claim_order(&[1, 2, 3, 4, 5, 6], Some(1), window),
            vec![3, 4, 5, 6, 1],
            "another profile of the same server and window is skipped"
        );
        assert_eq!(claim_order(&[1, 2, 3], Some(2), window), vec![3, 2]);
        assert_eq!(
            claim_order(&[1, 7], Some(6), window),
            vec![1, 7],
            "a focused tab without a window skips nothing"
        );
        assert_eq!(
            claim_order(&[2], Some(1), window),
            Vec::<TabId>::new(),
            "deferred claims are filtered against the focused tab too"
        );
    }

    #[test]
    fn the_host_key_identifies_a_server_reached_by_different_hostnames() {
        let alias = host("a", "dev.example.org", 22, "me");
        let ip = host("b", "192.0.2.7", 2222, "me");
        let key = Some("SHA256:server");
        let windows: HashMap<TabId, Option<ServerWindow>> = [
            (1, ServerWindow::new(&alias, key, Some("@1"))),
            (2, ServerWindow::new(&ip, key, Some("@1"))),
            (3, ServerWindow::new(&ip, key, Some("@2"))),
            (
                4,
                ServerWindow::new(&host("c", "192.0.2.7", 22, "you"), key, Some("@1")),
            ),
            (5, ServerWindow::new(&ip, Some("SHA256:other"), Some("@1"))),
            (6, ServerWindow::new(&ip, None, Some("@1"))),
            (7, ServerWindow::new(&alias, None, Some("@1"))),
            (8, ServerWindow::new(&alias, Some(""), Some("@1"))),
        ]
        .into();
        let window = |id| windows[&id].clone();
        assert_eq!(
            claim_order(&[1, 2, 3, 4, 5, 6], Some(1), window),
            vec![3, 4, 5, 6, 1],
            "same key, user and window: skipped despite another hostname and port"
        );
        assert_eq!(
            claim_order(&[7, 8, 6], Some(7), window),
            vec![6, 7],
            "without a key the hostname and port decide; an empty key is no key"
        );
    }
}
