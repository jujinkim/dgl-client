//! Connection actor: all remote commands and input are serialized on one task.
use crate::{
    Error, Result,
    command::{Command, Operation},
    flow::{Generation, MAX_BATCH, OutputKind, OutputQueue, PaneTarget},
    model::{self, Credential, HostProfile, RemoteState, Size},
    protocol::{Event, Parser},
    ssh::{self, Ssh, Stream},
};
use crate::{i18n::t, tr};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc};
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
pub const MAX_RETRIES: u8 = 3;
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(3);
/// Matches tmux's own one-second subscription check.
const METADATA_POLL: Duration = Duration::from_secs(1);
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RetryState {
    pub attempt: u8,
    pub wait_seconds: Option<u64>,
    pub stopped: bool,
    pub connecting: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Connecting,
    TrustRequired,
    Authenticating,
    Selecting,
    Restoring,
    Ready,
    Reconnecting,
    MissingSession,
    Failed,
    Shell,
    Closed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub sequence: u64,
    pub handle: u64,
    pub phase: Phase,
    #[serde(default)]
    pub resizing: bool,
    #[serde(default)]
    pub checking: bool,
    pub generation: Generation,
    pub session: Option<String>,
    pub pane: Option<String>,
    pub window: Option<String>,
    /// Render identities are independent of the active input pane.
    pub render_targets: BTreeMap<String, PaneTarget>,
    pub remote: RemoteState,
    pub fingerprint: Option<String>,
    pub error: Option<Error>,
    #[serde(default)]
    pub retry: RetryState,
    pub diagnostics: Vec<ssh::Probe>,
    pub agents: crate::agent::Snapshot,
    pub agent_setup: Option<serde_json::Value>,
}
impl State {
    pub fn accepts_input(&self) -> bool {
        !self.checking
            && (matches!(self.phase, Phase::Ready | Phase::Shell)
                || (self.phase == Phase::Restoring && self.resizing && !self.retry.connecting))
    }
    pub fn accepts_pointer(&self) -> bool {
        self.accepts_input() && !self.resizing
    }
    /// tmux sessions can be listed and targeted (select, create, manage) without
    /// waiting for a transport attempt.
    pub fn browses_sessions(&self) -> bool {
        matches!(
            self.phase,
            Phase::Selecting | Phase::Ready | Phase::MissingSession
        )
    }
    /// The phases in which the connection runs its agent observer.
    pub fn observing_agents(&self) -> bool {
        matches!(
            self.phase,
            Phase::Ready | Phase::Restoring | Phase::Selecting
        )
    }
    pub fn input_target(&self) -> Option<PaneTarget> {
        self.pane.as_ref().map(|p| PaneTarget {
            connection: self.handle,
            pane_id: p.clone(),
            generation: self.generation.clone(),
        })
    }
    /// Output belongs to a pane renderer, independently of input focus; a plain shell
    /// has a single target that is also its input target.
    pub fn accepts_output(&self, target: &PaneTarget) -> bool {
        if target.pane_id == "shell" {
            self.input_target().as_ref() == Some(target)
        } else {
            self.render_targets.get(&target.pane_id) == Some(target)
                && matches!(self.phase, Phase::Ready | Phase::Restoring)
        }
    }
    pub fn retry_exhausted(&self) -> bool {
        self.resume_exhausted() && self.retry.attempt >= MAX_RETRIES
    }
    fn can_check(&self) -> bool {
        matches!(
            self.phase,
            Phase::Ready | Phase::Shell | Phase::Selecting | Phase::MissingSession
        ) || (self.phase == Phase::Restoring && !self.retry.connecting)
    }
    fn resume_exhausted(&self) -> bool {
        self.phase == Phase::Failed && self.error == Some(Error::Network) && !self.retry.stopped
    }
    /// A transport attempt (connect, authenticate, reconnect or reattach) is in flight.
    pub fn reconnecting(&self) -> bool {
        matches!(
            self.phase,
            Phase::Connecting | Phase::Authenticating | Phase::Reconnecting
        ) || (self.phase == Phase::Restoring && self.retry.connecting)
    }
    fn retry_failed(&self) -> bool {
        self.phase == Phase::Failed && (self.error == Some(Error::Network) || self.retry.stopped)
    }
    /// The connection is in a retry cycle, regardless of a pending connection check.
    pub fn retry_available(&self) -> bool {
        self.reconnecting() || self.retry_failed()
    }
    /// Admission rule; a pending connection check also accepts (and merges) retry actions.
    pub fn can_stop_retrying(&self) -> bool {
        self.checking || self.reconnecting()
    }
    pub fn can_retry_now(&self) -> bool {
        self.can_stop_retrying() || self.retry_failed()
    }
    /// Whether a UI shows retry controls: never while a connection check is pending.
    pub fn offers_retry(&self) -> bool {
        !self.checking && self.retry_available()
    }
    /// The binding's JSON form. Fields keep their declared order (no `serde_json::Value`
    /// round trip), because adapters compare serialized pane targets as strings.
    pub fn view(&self) -> StateView<'_> {
        StateView {
            state: self,
            derived: self.derived(),
        }
    }
    /// Retry predicates serialized next to the state so bindings never re-derive them.
    pub fn derived(&self) -> Derived {
        Derived {
            observing_agents: self.observing_agents(),
            reconnecting: self.reconnecting(),
            retry_available: self.retry_available(),
            retry_exhausted: self.retry_exhausted(),
            retry_message: self.retry_message(),
        }
    }
    pub fn retry_message(&self) -> Option<String> {
        if self.retry.stopped {
            Some(t("재시도 정지 · 접속 정보 유지 중").into())
        } else if self.phase == Phase::Failed && self.error == Some(Error::Network) {
            Some(t("자동 재시도 3회 종료 · 접속 정보 유지 중").into())
        } else if let Some(seconds) = self.retry.wait_seconds {
            Some(tr!(
                "자동 재시도 대기 · {0}/3회 완료 · 대기 간격 {1}초",
                self.retry.attempt,
                seconds
            ))
        } else if self.phase == Phase::Reconnecting {
            Some(tr!(
                "연결 재시도 중 · 자동 재시도 {0}/3회",
                self.retry.attempt
            ))
        } else {
            None
        }
    }
}
#[derive(Serialize)]
pub struct StateView<'a> {
    #[serde(flatten)]
    pub state: &'a State,
    pub derived: Derived,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Derived {
    pub observing_agents: bool,
    pub reconnecting: bool,
    pub retry_available: bool,
    pub retry_exhausted: bool,
    pub retry_message: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Completion {
    pub request_id: u64,
    pub outcome: CompletionOutcome,
    pub error: Option<Error>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionOutcome {
    Sent,
    Completed,
    Cancelled,
    StaleGeneration,
    TransportLost,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Action {
    CheckConnection,
    RetryNow,
    StopRetrying,
    SelectSession {
        id: String,
    },
    SelectTarget {
        session: String,
        window: Option<String>,
        pane: Option<String>,
    },
    SelectPane {
        id: String,
    },
    Manage {
        operation: Operation,
    },
    AgentSetup {
        operation: String,
        token: Option<String>,
    },
    Refresh,
    Snapshot,
    Resize {
        columns: u16,
        rows: u16,
    },
}
enum RequestKind {
    Input {
        target: PaneTarget,
        bytes: Vec<u8>,
        revision: u64,
        coordinates: bool,
    },
    Action(Action),
}
struct Request {
    id: u64,
    kind: RequestKind,
    permit: Option<OwnedSemaphorePermit>,
    owner: Weak<Shared>,
}
impl Drop for Request {
    fn drop(&mut self) {
        // Control-lane cancellation may drop an actor future at any await point.
        // Keep its result and permit until the client consumes the completion.
        if let Some(permit) = self.permit.take()
            && let Some(shared) = self.owner.upgrade()
        {
            shared.cancelled.lock().unwrap().remove(&self.id);
            shared.results.lock().unwrap().push_back((
                Completion {
                    request_id: self.id,
                    outcome: CompletionOutcome::Cancelled,
                    error: Some(Error::Cancelled),
                },
                Some(permit),
            ));
        }
    }
}
#[derive(Default)]
struct TitleVisibility {
    enabled: bool,
    revision: u64,
    last_start: Option<tokio::time::Instant>,
}
struct Shared {
    /// Copy-on-write snapshot: readers clone the `Arc`; an update copies only while a reader holds one.
    state: Mutex<Arc<State>>,
    output: Arc<OutputQueue>,
    cancel: AtomicBool,
    stop: Notify,
    done: AtomicBool,
    finished: Notify,
    results: Mutex<VecDeque<(Completion, Option<OwnedSemaphorePermit>)>>,
    cancelled: Mutex<HashSet<u64>>,
    requests: Mutex<HashSet<u64>>,
    input_revision: AtomicU64,
    check_request: AtomicU64,
    retry_request: AtomicU64,
    title_visibility: Mutex<TitleVisibility>,
    title_changed: Notify,
}
pub struct Connection {
    shared: Arc<Shared>,
    tx: mpsc::Sender<Request>,
    retry_tx: mpsc::Sender<Request>,
    check_tx: mpsc::Sender<Request>,
    slots: Arc<Semaphore>,
    controls: Arc<Semaphore>,
    next: AtomicU64,
}
impl Shared {
    fn update(&self, f: impl FnOnce(&mut State)) {
        let mut guard = self.state.lock().unwrap();
        let s = Arc::make_mut(&mut guard);
        f(s);
        if s.phase != Phase::Restoring || s.error.is_some() {
            s.resizing = false;
        }
        s.sequence = s.sequence.checked_add(1).expect("state sequence exhausted");
    }
    fn phase(&self, p: Phase) {
        self.update(|s| {
            if matches!(p, Phase::Ready | Phase::Selecting) {
                s.retry = RetryState::default();
            }
            s.phase = p;
            s.resizing = false;
        });
    }
    fn target(&self) -> Option<PaneTarget> {
        self.state.lock().unwrap().input_target()
    }
    fn title_scope(&self) -> Option<crate::title::Scope> {
        let visibility = self.title_visibility.lock().unwrap();
        if !visibility.enabled {
            return None;
        }
        let s = self.state.lock().unwrap();
        if s.phase != Phase::Ready || s.checking {
            return None;
        }
        Some(crate::title::Scope {
            visibility: visibility.revision,
            target: PaneTarget {
                connection: s.handle,
                pane_id: s.pane.clone()?,
                generation: s.generation.clone(),
            },
        })
    }
    fn apply_title(&self, scope: &crate::title::Scope, title: String) {
        // Serialize hiding with publication; a hide/return also invalidates old responses.
        let visibility = self.title_visibility.lock().unwrap();
        if !visibility.enabled || visibility.revision != scope.visibility {
            return;
        }
        let mut guard = self.state.lock().unwrap();
        let s = &**guard;
        if s.phase != Phase::Ready
            || s.checking
            || s.handle != scope.target.connection
            || s.pane.as_ref() != Some(&scope.target.pane_id)
            || s.generation.connection != scope.target.generation.connection
            || s.generation.pane != scope.target.generation.pane
        {
            return;
        }
        if let Some(index) = s
            .remote
            .panes
            .iter()
            .position(|p| p.id == scope.target.pane_id && p.title != title)
        {
            let s = Arc::make_mut(&mut guard);
            s.remote.panes[index].title = title;
            s.sequence = s.sequence.checked_add(1).expect("state sequence exhausted");
        }
    }
    fn complete(&self, mut r: Request, error: Option<Error>) {
        let _ = self
            .retry_request
            .compare_exchange(r.id, 0, Ordering::SeqCst, Ordering::SeqCst);
        self.cancelled.lock().unwrap().remove(&r.id);
        self.results.lock().unwrap().push_back((
            Completion {
                request_id: r.id,
                outcome: match error {
                    None if matches!(r.kind, RequestKind::Input { .. }) => CompletionOutcome::Sent,
                    None => CompletionOutcome::Completed,
                    Some(Error::Cancelled) => CompletionOutcome::Cancelled,
                    Some(Error::StaleGeneration) => CompletionOutcome::StaleGeneration,
                    Some(Error::Network) => CompletionOutcome::TransportLost,
                    Some(_) => CompletionOutcome::Failed,
                },
                error,
            },
            r.permit.take(),
        ));
    }
    async fn emit_to(
        &self,
        target: &PaneTarget,
        kind: OutputKind,
        size: Size,
        bytes: &[u8],
    ) -> Result<()> {
        let id = self.output.subscription();
        self.emit_to_subscription(id, target, kind, size, bytes)
            .await
    }
    async fn emit_to_subscription(
        &self,
        id: u64,
        target: &PaneTarget,
        kind: OutputKind,
        size: Size,
        bytes: &[u8],
    ) -> Result<()> {
        if id != 0 {
            match self.output.push(id, target, kind, size, bytes).await {
                Err(Error::StaleGeneration) => {}
                r => r?,
            }
        }
        Ok(())
    }
    async fn emit(&self, kind: OutputKind, size: Size, bytes: &[u8]) -> Result<()> {
        if let Some(target) = self.target() {
            let id = self.output.subscription();
            if id != 0 {
                match self.output.push(id, &target, kind, size, bytes).await {
                    Err(Error::StaleGeneration) => {}
                    r => r?,
                }
            }
        }
        Ok(())
    }
}
impl Connection {
    pub fn start(host: HostProfile, credential: Credential, trusted: Option<String>) -> Arc<Self> {
        let handle = NEXT_HANDLE
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| x.checked_add(1))
            .expect("connection handles exhausted");
        let shared = Arc::new(Shared {
            state: Mutex::new(Arc::new(State {
                sequence: 0,
                handle,
                phase: Phase::Connecting,
                resizing: false,
                checking: false,
                generation: Generation {
                    connection: 1,
                    pane: 0,
                    size: 0,
                },
                session: host.last_session.clone(),
                pane: None,
                window: None,
                render_targets: BTreeMap::new(),
                remote: Default::default(),
                fingerprint: trusted.clone(),
                error: None,
                retry: RetryState::default(),
                diagnostics: vec![],
                agents: Default::default(),
                agent_setup: None,
            })),
            output: Arc::new(OutputQueue::default()),
            cancel: AtomicBool::new(false),
            stop: Notify::new(),
            done: AtomicBool::new(false),
            finished: Notify::new(),
            results: Mutex::new(VecDeque::new()),
            cancelled: Mutex::new(HashSet::new()),
            requests: Mutex::new(HashSet::new()),
            input_revision: AtomicU64::new(0),
            check_request: AtomicU64::new(0),
            retry_request: AtomicU64::new(0),
            title_visibility: Mutex::new(TitleVisibility::default()),
            title_changed: Notify::new(),
        });
        let (tx, rx) = mpsc::channel(32);
        let (retry_tx, retry_rx) = mpsc::channel(32);
        let (check_tx, check_rx) = mpsc::channel(1);
        let result = Arc::new(Self {
            shared: shared.clone(),
            tx,
            retry_tx,
            check_tx,
            slots: Arc::new(Semaphore::new(33)),
            controls: Arc::new(Semaphore::new(3)),
            next: AtomicU64::new(1),
        });
        tokio::spawn(async move {
            let stopped = shared.stop.notified();
            tokio::pin!(stopped);
            stopped.as_mut().enable();
            if !shared.cancel.load(Ordering::SeqCst) {
                tokio::select! {biased;_=stopped=>{},_=run(shared.clone(),host,credential,trusted,rx,retry_rx,check_rx)=>{}}
            } else {
                drop(rx);
                drop(retry_rx);
                drop(check_rx);
            }
            shared.output.close();
            // Close cancels outstanding work without losing its completion record.
            // Release permits now; data and reserved control results remain drainable.
            {
                let requests = shared.requests.lock().unwrap();
                let mut results = shared.results.lock().unwrap();
                let completed: HashSet<_> = results.iter().map(|(r, _)| r.request_id).collect();
                for (_, permit) in results.iter_mut() {
                    permit.take();
                }
                for id in requests.iter().filter(|id| !completed.contains(id)) {
                    results.push_back((
                        Completion {
                            request_id: *id,
                            outcome: CompletionOutcome::Cancelled,
                            error: Some(Error::Cancelled),
                        },
                        None,
                    ));
                }
            }
            shared.cancelled.lock().unwrap().clear();
            shared.update(|s| {
                s.phase = Phase::Closed;
                s.checking = false;
                s.retry.wait_seconds = None;
                s.retry.stopped = false;
                s.retry.connecting = false;
                s.agents.invalidate();
                s.pane = None;
                s.window = None;
                s.generation.connection = s.generation.connection.saturating_add(1);
                s.render_targets.clear();
            });
            shared.done.store(true, Ordering::SeqCst);
            shared.finished.notify_waiters();
        });
        result
    }
    /// A shared snapshot; cloning it is a reference-count increment.
    pub fn state(&self) -> Arc<State> {
        self.shared.state.lock().unwrap().clone()
    }
    /// Changes whenever a new state snapshot is published.
    pub fn sequence(&self) -> u64 {
        self.shared.state.lock().unwrap().sequence
    }
    pub fn target(&self) -> Option<PaneTarget> {
        self.shared.target()
    }
    pub fn output(&self) -> Arc<OutputQueue> {
        self.shared.output.clone()
    }
    /// Transient UI visibility, independent of output subscriptions and saved state.
    pub fn set_title_updates_enabled(&self, enabled: bool) {
        let mut visibility = self.shared.title_visibility.lock().unwrap();
        if visibility.enabled == enabled {
            return;
        }
        visibility.enabled = enabled;
        visibility.revision = visibility
            .revision
            .checked_add(1)
            .expect("visibility revision exhausted");
        self.shared.title_changed.notify_one();
    }
    fn submit(&self, kind: RequestKind) -> Result<u64> {
        if self.shared.cancel.load(Ordering::SeqCst) || self.shared.done.load(Ordering::SeqCst) {
            return Err(Error::Closed);
        }
        let retry = matches!(&kind, RequestKind::Action(Action::RetryNow));
        // Serialize admission so repeated clicks cannot cancel/restart a handshake.
        let retry_guard = retry.then(|| self.shared.state.lock().unwrap());
        if retry {
            if retry_guard.as_ref().unwrap().checking {
                return Ok(self.shared.check_request.load(Ordering::SeqCst));
            }
            let pending = self.shared.retry_request.load(Ordering::SeqCst);
            if pending != 0 {
                return Ok(pending);
            }
        }
        let controls = matches!(
            &kind,
            RequestKind::Action(Action::CheckConnection | Action::RetryNow | Action::StopRetrying)
        );
        let permit = if controls {
            &self.controls
        } else {
            &self.slots
        }
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::QueueFull)?;
        let id = self
            .next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| x.checked_add(1))
            .map_err(|_| Error::Closed)?;
        self.shared.requests.lock().unwrap().insert(id);
        if retry {
            if retry_guard.as_ref().unwrap().retry.connecting {
                drop(retry_guard);
                self.shared.complete(
                    Request {
                        id,
                        kind,
                        permit: Some(permit),
                        owner: Arc::downgrade(&self.shared),
                    },
                    None,
                );
                return Ok(id);
            }
            self.shared.retry_request.store(id, Ordering::SeqCst);
        }
        drop(retry_guard);
        if matches!(&kind, RequestKind::Action(Action::CheckConnection)) {
            let mut state = self.shared.state.lock().unwrap();
            if state.checking {
                self.shared.requests.lock().unwrap().remove(&id);
                return Ok(self.shared.check_request.load(Ordering::SeqCst));
            }
            if !(state.can_check() || state.resume_exhausted()) {
                drop(state);
                self.shared.complete(
                    Request {
                        id,
                        kind,
                        permit: Some(permit),
                        owner: Arc::downgrade(&self.shared),
                    },
                    None,
                );
                return Ok(id);
            }
            // Block input at admission, before the actor or UI next polls state.
            let s = Arc::make_mut(&mut state);
            s.checking = true;
            self.shared.check_request.store(id, Ordering::SeqCst);
            self.shared.input_revision.fetch_add(1, Ordering::SeqCst);
            s.sequence += 1;
            drop(state);
            if let Err(error) = self.check_tx.try_send(Request {
                id,
                kind,
                permit: Some(permit),
                owner: Arc::downgrade(&self.shared),
            }) {
                error.into_inner().permit.take();
                self.shared.update(|s| s.checking = false);
                self.shared.requests.lock().unwrap().remove(&id);
                return Err(Error::QueueFull);
            }
            return Ok(id);
        }
        let tx = if matches!(
            &kind,
            RequestKind::Action(Action::RetryNow | Action::StopRetrying)
        ) {
            &self.retry_tx
        } else {
            &self.tx
        };
        if let Err(error) = tx.try_send(Request {
            id,
            kind,
            permit: Some(permit),
            owner: Arc::downgrade(&self.shared),
        }) {
            error.into_inner().permit.take();
            let _ = self.shared.retry_request.compare_exchange(
                id,
                0,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            self.shared.requests.lock().unwrap().remove(&id);
            return Err(Error::QueueFull);
        }
        Ok(id)
    }
    pub fn input(&self, target: PaneTarget, bytes: Vec<u8>) -> Result<u64> {
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(Error::InvalidArgument);
        }
        // Existing raw-input ABI: mouse reports retain their exact grid generation.
        let coordinates = bytes.starts_with(b"\x1b[M") || bytes.starts_with(b"\x1b[<");
        let revision = self.shared.input_revision.load(Ordering::SeqCst);
        let s = self.state();
        if s.phase == Phase::Closed {
            return Err(Error::Closed);
        }
        if !s.accepts_input() || (coordinates && !s.accepts_pointer()) {
            return Err(Error::NotReady);
        }
        if self.target().is_none_or(|current| {
            !current.same_input_destination(&target) || (coordinates && current != target)
        }) {
            return Err(Error::StaleGeneration);
        }
        self.submit(RequestKind::Input {
            target,
            bytes,
            revision,
            coordinates,
        })
    }
    pub fn action(&self, action: Action) -> Result<u64> {
        if self.shared.cancel.load(Ordering::SeqCst) || self.shared.done.load(Ordering::SeqCst) {
            return Err(Error::Closed);
        }
        if matches!(action, Action::CheckConnection) && self.state().checking {
            return Ok(self.shared.check_request.load(Ordering::SeqCst));
        }
        match &action {
            Action::RetryNow if !self.state().can_retry_now() => return Err(Error::NotReady),
            Action::StopRetrying if !self.state().can_stop_retrying() => {
                return Err(Error::NotReady);
            }
            Action::SelectSession { id } => {
                crate::command::target(id, '$')?;
            }
            Action::SelectTarget {
                session,
                window,
                pane,
            } => {
                crate::command::target(session, '$')?;
                if let Some(id) = window {
                    crate::command::target(id, '@')?;
                }
                if let Some(id) = pane {
                    crate::command::target(id, '%')?;
                }
            }
            Action::SelectPane { id } => {
                crate::command::target(id, '%')?;
            }
            Action::Manage { operation } => {
                Command::new(operation)?;
            }
            Action::AgentSetup { operation, token } => {
                if !["inspect", "install", "remove"].contains(&operation.as_str()) {
                    return Err(Error::InvalidArgument);
                }
                if operation != "inspect"
                    && !token
                        .as_ref()
                        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(Error::InvalidArgument);
                }
            }
            Action::Resize { columns, rows } => {
                Size::new(*columns, *rows)?;
            }
            _ => {}
        }
        if matches!(action, Action::AgentSetup { .. }) {
            self.shared.update(|s| s.agent_setup = None);
        }
        self.submit(RequestKind::Action(action))
    }
    pub fn cancel(&self, id: u64) -> Result<()> {
        if self.shared.cancel.load(Ordering::SeqCst) {
            return Err(Error::Closed);
        }
        if self.shared.requests.lock().unwrap().contains(&id) {
            self.shared.cancelled.lock().unwrap().insert(id);
        }
        Ok(())
    }
    pub fn completions(&self) -> Vec<Completion> {
        let mut requests = self.shared.requests.lock().unwrap();
        self.shared
            .results
            .lock()
            .unwrap()
            .drain(..)
            .map(|(r, _)| {
                requests.remove(&r.request_id);
                r
            })
            .collect()
    }
    pub fn close(&self) {
        if !self.shared.cancel.swap(true, Ordering::SeqCst) {
            self.shared.output.close();
            self.shared.stop.notify_waiters();
        }
    }
    pub async fn join(&self) {
        loop {
            let done = self.shared.finished.notified();
            tokio::pin!(done);
            done.as_mut().enable();
            if self.shared.done.load(Ordering::SeqCst) {
                return;
            }
            done.await;
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}
/// A capture is one byte stream, not one renderer round trip per cursor move/row.
/// Keep only one bounded batch while the output queue owns all pending payload.
struct SnapshotWriter {
    output: Arc<OutputQueue>,
    subscription: u64,
    target: PaneTarget,
    size: Size,
    bytes: Vec<u8>,
}
impl SnapshotWriter {
    fn new(output: Arc<OutputQueue>, subscription: u64, target: PaneTarget, size: Size) -> Self {
        Self {
            output,
            subscription,
            target,
            size,
            bytes: Vec::with_capacity(MAX_BATCH),
        }
    }
    async fn write(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let n = bytes.len().min(MAX_BATCH - self.bytes.len());
            self.bytes.extend_from_slice(&bytes[..n]);
            bytes = &bytes[n..];
            if self.bytes.len() == MAX_BATCH {
                self.flush().await?;
            }
        }
        Ok(())
    }
    async fn rows(&mut self, rows: Vec<Vec<u8>>) -> Result<()> {
        for (i, row) in rows.into_iter().enumerate() {
            self.write(format!("\x1b[{};1H", i + 1).as_bytes()).await?;
            self.write(&crate::protocol::decode(&row, true)?).await?;
        }
        Ok(())
    }
    async fn flush(&mut self) -> Result<()> {
        if !self.bytes.is_empty() && self.subscription != 0 {
            match self
                .output
                .push(
                    self.subscription,
                    &self.target,
                    OutputKind::ResetChunk,
                    self.size,
                    &self.bytes,
                )
                .await
            {
                // A capture remains bound to the subscriber present at its start.
                Err(Error::StaleGeneration) => {}
                result => result?,
            }
        }
        self.bytes.clear();
        Ok(())
    }
}

/// Replaces the format subscription on tmux before 3.7, which crashes on it (see
/// `command::safe_subscriptions`): the same format is read once a second instead.
struct MetadataPoll {
    due: tokio::time::Instant,
    last: Vec<Vec<u8>>,
}
struct Control {
    stream: Stream,
    parser: Parser,
    events: VecDeque<Event>,
    shared: Arc<Shared>,
    size: Size,
    screens: BTreeMap<String, (PaneTarget, Size, String)>,
    capturing: Option<String>,
    dirty: bool,
    paused: HashSet<String>,
    titles: crate::title::Updates,
    metadata: Option<MetadataPoll>,
}
impl Control {
    async fn next(&mut self) -> Result<Event> {
        loop {
            if let Some(e) = self.events.pop_front() {
                return Ok(e);
            }
            let msg = self.stream.wait().await.ok_or(Error::Network)?;
            match msg {
                russh::ChannelMsg::Data { data } => self.events.extend(self.parser.feed(&data)?),
                russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Close => {
                    return Err(Error::MissingSession);
                }
                _ => {}
            }
        }
    }
    async fn unsolicited(&mut self, e: Event) -> Result<()> {
        match e {
            Event::Output { pane, bytes } => {
                self.titles.reconcile(self.shared.title_scope());
                self.titles.feed(&pane, &bytes);
                if self.capturing.as_ref() != Some(&pane)
                    && let Some((target, size, _)) = self.screens.get(&pane)
                {
                    self.shared
                        .emit_to(target, OutputKind::Live, *size, &bytes)
                        .await?;
                }
            }
            Event::Notification { name, arguments } => {
                if name == "%exit" {
                    return Err(Error::MissingSession);
                }
                if name == "%pause"
                    && let Some(pane) = arguments.split_whitespace().next()
                {
                    self.paused.insert(pane.to_owned());
                }
                if [
                    "%session-changed",
                    "%sessions-changed",
                    "%session-renamed",
                    "%window-add",
                    "%window-close",
                    "%window-renamed",
                    "%unlinked-window-add",
                    "%unlinked-window-close",
                    "%unlinked-window-renamed",
                    "%layout-change",
                    "%window-pane-changed",
                    "%session-window-changed",
                    "%pane-mode-changed",
                    "%subscription-changed",
                ]
                .contains(&name.as_str())
                {
                    self.dirty = true;
                }
            }
            Event::Response { .. } => return Err(Error::Protocol),
        }
        Ok(())
    }
    async fn response(&mut self) -> Result<Vec<Vec<u8>>> {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let e = self.next().await?;
                match e {
                    Event::Response { failed, lines, .. } => {
                        return if failed {
                            Err(Error::InvalidArgument)
                        } else {
                            Ok(lines)
                        };
                    }
                    _ => self.unsolicited(e).await?,
                }
            }
        })
        .await
        .map_err(|_| Error::Network)?
    }
    async fn command(&mut self, op: Operation) -> Result<Vec<Vec<u8>>> {
        self.commands(&[op]).await?.pop().unwrap()
    }
    /// Writes the commands at once and reads their replies in order: one round trip.
    /// Each is its own line, so a failed command (`Err` in its reply) does not stop
    /// the ones after it the way a `;` command list would.
    async fn commands(&mut self, ops: &[Operation]) -> Result<Vec<Result<Vec<Vec<u8>>>>> {
        let mut wire = String::new();
        for op in ops {
            wire.push_str(&Command::new(op)?.wire);
            wire.push('\n');
        }
        self.stream
            .data(wire.as_bytes())
            .await
            .map_err(|_| Error::Network)?;
        let mut replies = Vec::with_capacity(ops.len());
        for _ in ops {
            replies.push(match self.response().await {
                Err(Error::InvalidArgument) => Err(Error::InvalidArgument),
                reply => Ok(reply?),
            });
        }
        Ok(replies)
    }
    async fn refresh(&mut self) -> Result<()> {
        self.refresh_after(&[]).await.map(drop)
    }
    /// Runs `ops`, then lists every session, in one round trip. The listing is applied
    /// even if an operation failed; the first failure is returned after it.
    async fn refresh_after(&mut self, ops: &[Operation]) -> Result<Vec<Vec<Vec<u8>>>> {
        // Clear before reading: notifications received during the queries still
        // require another pass, otherwise a concurrent title change can be lost.
        self.dirty = false;
        // Title output after this point gets its own query even if the listing covers it.
        self.titles.covered_by_refresh();
        let mut all = ops.to_vec();
        all.extend([Operation::Sessions, Operation::Windows, Operation::Panes]);
        let mut replies = self.commands(&all).await?;
        let lists = replies.split_off(ops.len());
        let [sessions, windows, panes] = <[_; 3]>::try_from(lists).unwrap();
        let remote = model::remote(&sessions?, &windows?, &panes?)?;
        self.shared.update(|s| {
            s.remote = remote;
            if s.session
                .as_ref()
                .is_some_and(|id| !s.remote.sessions.iter().any(|v| &v.id == id))
            {
                s.phase = Phase::MissingSession;
                s.pane = None;
                s.window = None;
                s.render_targets.clear();
            } else {
                // Window/pane selection is shared with ordinary clients in this session.
                let window = s
                    .remote
                    .windows
                    .iter()
                    .find(|w| Some(&w.session) == s.session.as_ref() && w.active);
                let pane = window
                    .and_then(|w| s.remote.panes.iter().find(|p| p.window == w.id && p.active));
                let next = pane.map(|p| p.id.clone());
                if s.pane != next {
                    s.pane = next;
                    s.generation.pane = s.generation.pane.saturating_add(1);
                }
                s.window = window.map(|w| w.id.clone());
            }
        });
        replies.into_iter().collect()
    }
    async fn refresh_title(&mut self) -> Result<()> {
        self.titles.reconcile(self.shared.title_scope());
        let Some(scope) = self.titles.begin() else {
            return Ok(());
        };
        self.shared.title_visibility.lock().unwrap().last_start = Some(tokio::time::Instant::now());
        let result = self
            .command(Operation::PaneTitle {
                target: scope.target.pane_id.clone(),
            })
            .await;
        self.titles.finish();
        match result {
            Ok(rows) => self.shared.apply_title(&scope, model::pane_title(&rows)?),
            // A deleted pane is resolved by the normal metadata/screen path.
            Err(Error::InvalidArgument) => self.dirty = true,
            Err(error) => return Err(error),
        }
        Ok(())
    }
    async fn poll_metadata(&mut self) -> Result<()> {
        let value = self.command(Operation::MetadataPoll).await;
        let poll = self.metadata.as_mut().unwrap();
        poll.due = tokio::time::Instant::now() + METADATA_POLL;
        match value {
            Ok(value) if value == poll.last => {}
            Ok(value) => {
                poll.last = value;
                self.dirty = true;
            }
            Err(Error::InvalidArgument) => self.dirty = true,
            Err(error) => return Err(error),
        }
        Ok(())
    }
    async fn refresh_screen_if_needed(&mut self, resizing: bool) -> Result<()> {
        self.refresh_screen_after(&[], resizing).await.map(drop)
    }
    /// `refresh_after`, then captures the panes it changed. A failed operation still
    /// leaves the screens in step with the listing before its error is returned.
    async fn refresh_screen_after(
        &mut self,
        ops: &[Operation],
        resizing: bool,
    ) -> Result<Vec<Vec<Vec<u8>>>> {
        let before = self.shared.target();
        let result = self.refresh_after(ops).await;
        if result
            .as_ref()
            .is_err_and(|error| *error != Error::InvalidArgument)
        {
            return result;
        }
        let same = before
            .as_ref()
            .zip(self.shared.target().as_ref())
            .is_some_and(|(a, b)| a.same_input_destination(b));
        self.sync_screens(resizing && same, false).await?;
        result
    }
    async fn select_pane(&mut self, id: String) -> Result<()> {
        let state = self.shared.state.lock().unwrap().clone();
        let p = state
            .remote
            .panes
            .iter()
            .find(|p| p.id == id)
            .ok_or(Error::InvalidArgument)?;
        let window = state
            .remote
            .windows
            .iter()
            .find(|w| w.id == p.window && Some(&w.session) == state.session.as_ref())
            .ok_or(Error::InvalidArgument)?;
        let ops = [
            Operation::SelectWindow {
                target: window.id.clone(),
            },
            Operation::SelectPane { target: id },
        ];
        self.refresh_screen_after(&ops, false).await.map(drop)
    }
    async fn sync_screens(&mut self, resizing: bool, force: bool) -> Result<()> {
        let state = self.shared.state.lock().unwrap().clone();
        let visible = state
            .remote
            .windows
            .iter()
            .find(|w| {
                Some(&w.id) == state.window.as_ref() && Some(&w.session) == state.session.as_ref()
            })
            .map(|w| {
                state
                    .remote
                    .panes
                    .iter()
                    .filter(|p| p.window == w.id && (!w.zoomed || p.active))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.screens
            .retain(|id, _| visible.iter().any(|p| &p.id == id));
        self.shared.update(|s| {
            s.render_targets
                .retain(|id, _| visible.iter().any(|p| &p.id == id))
        });
        for p in visible {
            let size = Size {
                columns: p.width,
                rows: p.height,
            };
            if force
                || self.paused.contains(&p.id)
                || self
                    .screens
                    .get(&p.id)
                    .is_none_or(|(_, old, mode)| *old != size || *mode != p.mode)
            {
                self.snapshot_pane(p.id.clone(), resizing).await?;
                if let Some(screen) = self.screens.get_mut(&p.id) {
                    screen.2 = p.mode;
                }
            }
        }
        self.paused.retain(|id| self.screens.contains_key(id));
        if self.shared.state.lock().unwrap().pane.is_some() {
            self.shared.phase(Phase::Ready);
        }
        Ok(())
    }
    async fn snapshot_pane(&mut self, id: String, resizing: bool) -> Result<()> {
        // Every part of this capture belongs to the consumer present at its start.
        let subscription = self.shared.output.subscription();
        self.capturing = Some(id.clone());
        self.shared.update(|s| {
            s.phase = Phase::Restoring;
            s.resizing = resizing;
        });
        let commands = [
            Operation::Pause { target: id.clone() },
            Operation::Modes { target: id.clone() },
            Operation::CaptureMain { target: id.clone() },
            Operation::Capture { target: id.clone() },
            Operation::Pending { target: id.clone() },
            Operation::Resume { target: id.clone() },
        ];
        let wire = commands
            .iter()
            .map(Command::new)
            .collect::<Result<Vec<_>>>()?
            .iter()
            .map(|c| c.wire.clone())
            .collect::<Vec<_>>()
            .join(" ; ")
            + "\n";
        self.stream
            .data(wire.as_bytes())
            .await
            .map_err(|_| Error::Network)?;
        self.response().await?;
        let mode = self.response().await?;
        let fields = mode
            .first()
            .ok_or(Error::Protocol)?
            .split(|b| *b == b'|')
            .map(|v| {
                if v.is_empty() {
                    Ok(0)
                } else {
                    std::str::from_utf8(v)
                        .map_err(|_| Error::Protocol)?
                        .parse::<u16>()
                        .map_err(|_| Error::Protocol)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        if fields.len() != 17 {
            return Err(Error::Protocol);
        }
        if fields[15] == 0 || fields[16] == 0 {
            return Err(Error::Protocol);
        }
        self.size = Size {
            columns: fields[15],
            rows: fields[16],
        };
        let target = {
            let s = self.shared.state.lock().unwrap();
            PaneTarget {
                connection: s.handle,
                pane_id: id.clone(),
                generation: Generation {
                    connection: s.generation.connection,
                    pane: 0,
                    size: s.generation.size.checked_add(1).expect("size exhausted"),
                },
            }
        };
        self.screens
            .insert(id.clone(), (target.clone(), self.size, String::new()));
        self.shared.update(|s| {
            s.render_targets.insert(id.clone(), target.clone());
            s.generation.size = s.generation.size.checked_add(1).expect("size exhausted");
            // The capture may observe a newer layout than the preceding list query.
            // Keep that baseline so its queued layout notification cannot recapture it.
            if let Some(pane) = s.remote.panes.iter_mut().find(|p| p.id == id) {
                pane.width = self.size.columns;
                pane.height = self.size.rows;
            }
        });
        self.shared
            .emit_to_subscription(subscription, &target, OutputKind::Resize, self.size, &[])
            .await?;
        self.shared
            .emit_to_subscription(
                subscription,
                &target,
                OutputKind::ResetBegin,
                self.size,
                b"\x1bc",
            )
            .await?;
        let main = self.response().await?;
        let mut writer = SnapshotWriter::new(
            self.shared.output.clone(),
            subscription,
            target.clone(),
            self.size,
        );
        if fields[2] == 1 {
            writer.rows(main).await?;
            writer.write(b"\x1b[?1049h").await?;
        }
        let screen = self.response().await?;
        writer.rows(screen).await?;
        let mut modes = format!(
            "\x1b[0m\x1b[{};{}r\x1b[{};{}H",
            fields[12] + 1,
            fields[13] + 1,
            fields[1] + 1,
            fields[0] + 1
        );
        for (code, index) in [
            (25, 3),
            (1, 4),
            (1003, 6),
            (1002, 7),
            (1000, 8),
            (1005, 9),
            (1006, 10),
            (2004, 11),
        ] {
            modes.push_str(&format!(
                "\x1b[?{code}{}",
                if fields[index] == 1 { 'h' } else { 'l' }
            ));
        }
        modes.push_str(if fields[5] == 1 { "\x1b=" } else { "\x1b>" });
        modes.push_str(&format!(
            "\x1b[4{}",
            if fields[14] == 1 { 'h' } else { 'l' }
        ));
        writer.write(modes.as_bytes()).await?;
        for row in self.response().await? {
            writer.write(&crate::protocol::decode(&row, true)?).await?;
        }
        self.response().await?;
        writer.flush().await?;
        self.shared
            .emit_to_subscription(subscription, &target, OutputKind::ResetEnd, self.size, &[])
            .await?;
        self.capturing = None;
        self.paused.remove(&id);
        Ok(())
    }
}
async fn list(ssh: &Ssh) -> Result<RemoteState> {
    let (code, bytes) = ssh.execute_tmux(&Command::inventory()).await?;
    let rows: Vec<_> = bytes
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .map(<[u8]>::to_vec)
        .collect();
    if code != 0 {
        // Without a server there is no session to list; a later listing failure is an error.
        return if rows.is_empty() {
            Ok(RemoteState::default())
        } else {
            Err(Error::InvalidArgument)
        };
    }
    model::inventory(&rows)
}
fn resolve_target(
    remote: &RemoteState,
    session: &str,
    window: Option<&str>,
    pane: Option<&str>,
) -> Result<String> {
    if !remote.sessions.iter().any(|s| s.id == session) {
        return Err(Error::InvalidArgument);
    }
    let windows: Vec<_> = remote
        .windows
        .iter()
        .filter(|w| w.session == session && window.is_none_or(|id| id == w.id))
        .collect();
    let candidates: Vec<_> = remote
        .panes
        .iter()
        .filter(|p| windows.iter().any(|w| w.id == p.window) && pane.is_none_or(|id| id == p.id))
        .collect();
    candidates
        .iter()
        .find(|p| {
            p.active
                && windows
                    .iter()
                    .any(|w| w.id == p.window && (window.is_some() || w.active))
        })
        .or_else(|| candidates.first())
        .map(|p| p.id.clone())
        .ok_or(Error::InvalidArgument)
}
async fn attach(
    ssh: &Ssh,
    shared: Arc<Shared>,
    id: String,
    size: Size,
    remembered: Option<String>,
) -> Result<Control> {
    let stream = ssh.control(&id).await?;
    shared.update(|s| {
        s.phase = Phase::Restoring;
        s.resizing = false;
        s.error = None;
        s.session = Some(id.clone());
        s.pane = None;
        s.window = None;
        s.render_targets.clear();
    });
    let mut c = Control {
        stream,
        parser: Parser::default(),
        events: VecDeque::new(),
        shared,
        size,
        screens: BTreeMap::new(),
        capturing: None,
        dirty: false,
        paused: HashSet::new(),
        titles: Default::default(),
        metadata: None,
    };
    c.titles
        .set_last_start(c.shared.title_visibility.lock().unwrap().last_start);
    c.response().await?;
    let subscribe = ssh
        .tmux_version
        .as_deref()
        .is_some_and(crate::command::safe_subscriptions);
    let setup = [
        Operation::FlowControl,
        // A polled baseline is read before the listing, so later changes are not missed.
        if subscribe {
            Operation::MetadataSubscription
        } else {
            Operation::MetadataPoll
        },
        Operation::ClientSize {
            columns: size.columns,
            rows: size.rows,
        },
    ];
    let mut replies = c.refresh_after(&setup).await?;
    if !subscribe {
        c.metadata = Some(MetadataPoll {
            due: tokio::time::Instant::now() + METADATA_POLL,
            last: replies.swap_remove(1),
        });
    }
    let state = c.shared.state.lock().unwrap().clone();
    let windows: Vec<_> = state
        .remote
        .windows
        .iter()
        .filter(|w| w.session == id)
        .collect();
    let pane = state
        .remote
        .panes
        .iter()
        .find(|p| Some(&p.id) == remembered.as_ref() && windows.iter().any(|w| w.id == p.window))
        .or_else(|| {
            state
                .remote
                .panes
                .iter()
                .find(|p| p.active && windows.iter().any(|w| w.active && w.id == p.window))
        })
        .or_else(|| {
            state
                .remote
                .panes
                .iter()
                .find(|p| windows.iter().any(|w| w.id == p.window))
        })
        .map(|p| p.id.clone());
    if let Some(p) = pane {
        c.select_pane(p).await?;
    } else {
        c.shared.phase(Phase::Selecting)
    }
    Ok(c)
}
// Retry controls have a separate queue so a backoff or an in-flight SSH handshake
// never blocks Stop/Retry behind ordinary terminal requests.
async fn next_retry_control(shared: &Shared, rx: &mut mpsc::Receiver<Request>) -> Request {
    loop {
        let Some(request) = rx.recv().await else {
            return std::future::pending().await;
        };
        let state = shared.state.lock().unwrap().clone();
        let allowed = match request.kind {
            RequestKind::Action(Action::RetryNow) => state.can_retry_now(),
            RequestKind::Action(Action::StopRetrying) => state.can_stop_retrying(),
            _ => false,
        };
        let error = if shared.cancelled.lock().unwrap().contains(&request.id) {
            Some(Error::Cancelled)
        } else if !allowed {
            Some(Error::NotReady)
        } else {
            None
        };
        if let Some(error) = error {
            shared.complete(request, Some(error));
        } else {
            return request;
        }
    }
}
fn apply_retry_control(shared: &Shared, request: Request) -> bool {
    let retry = matches!(request.kind, RequestKind::Action(Action::RetryNow));
    shared.update(|s| {
        s.checking = false;
        if retry {
            s.retry = RetryState {
                attempt: 1,
                connecting: true,
                ..RetryState::default()
            };
            s.phase = Phase::Reconnecting;
            s.error = None;
        } else {
            s.retry.stopped = true;
            s.retry.wait_seconds = None;
            s.retry.connecting = false;
            s.phase = Phase::Failed;
        }
    });
    shared.complete(request, None);
    retry
}
fn schedule_retry(shared: &Shared, error: Error) -> Option<Duration> {
    let mut delay = None;
    shared.update(|s| {
        s.retry.wait_seconds = None;
        s.retry.connecting = false;
        if error == Error::Network && s.retry.attempt < MAX_RETRIES {
            let seconds = if s.retry.attempt == 0 { 0 } else { 1 };
            s.phase = Phase::Reconnecting;
            s.retry.wait_seconds = Some(seconds);
            delay = Some(Duration::from_secs(seconds));
        } else {
            s.phase = match error {
                Error::HostKeyUnknown => Phase::TrustRequired,
                Error::MissingSession => Phase::MissingSession,
                _ => Phase::Failed,
            };
        }
    });
    delay
}
async fn wait_for_retry(
    shared: &Shared,
    rx: &mut mpsc::Receiver<Request>,
    checks: &mut mpsc::Receiver<Request>,
    mut delay: Option<Duration>,
) {
    loop {
        let timer = async {
            if let Some(delay) = delay {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            biased;
            Some(request) = checks.recv() => {
                let state = shared.state.lock().unwrap().clone();
                let cancelled = shared.cancelled.lock().unwrap().contains(&request.id);
                shared.update(|s| s.checking = false);
                if !cancelled && state.resume_exhausted() {
                    shared.update(|s| s.retry = RetryState::default());
                    delay = schedule_retry(shared, Error::Network);
                }
                shared.complete(request, if cancelled { Some(Error::Cancelled) } else { None });
            }
            request = next_retry_control(shared, rx) => {
                if apply_retry_control(shared, request) { return; }
                delay = None;
            }
            _ = timer => {
                shared.update(|s| {
                    s.retry.attempt += 1;
                    s.retry.wait_seconds = None;
                });
                return;
            }
        }
    }
}
async fn run(
    shared: Arc<Shared>,
    host: HostProfile,
    credential: Credential,
    trusted: Option<String>,
    mut rx: mpsc::Receiver<Request>,
    mut retry_rx: mpsc::Receiver<Request>,
    mut check_rx: mpsc::Receiver<Request>,
) {
    let mut first = true;
    let mut size = Size::default();
    let mut remembered = host.last_pane.clone();
    loop {
        let observed = Arc::new(Mutex::new(None));
        shared.update(|s| {
            s.phase = if first {
                Phase::Connecting
            } else {
                Phase::Reconnecting
            };
            s.retry.connecting = true;
        });
        first = false;
        let connection = async {
            let mut ssh =
                ssh::connect(&host, &credential, trusted.clone(), observed.clone()).await?;
            shared.phase(Phase::Authenticating);
            let tmux = ssh.discover().await?;
            shared.update(|s| {
                s.error = None;
                s.diagnostics = ssh.diagnostics.clone();
            });
            let (checked_tx, mut checked_rx) = mpsc::channel(1);
            // The observer uses a separate exec channel and runs even if output delivery
            // is backpressured or no terminal is subscribed. Dropping this future cancels it.
            let result = tokio::select! {
                result = connected(&ssh, shared.clone(), &mut rx, &mut checked_rx, tmux, &mut size, &mut remembered) => result,
                result = check_connections(&ssh, shared.clone(), &mut check_rx, checked_tx) => result,
                _ = observe_agents(&ssh, shared.clone(), tmux) => unreachable!(),
            };
            if result == Err(Error::Network) {
                ssh.abort();
            }
            ssh.close().await;
            result
        };
        let result = tokio::select! {
            biased;
            request = next_retry_control(&shared, &mut retry_rx) => Err(request),
            result = connection => Ok(result),
        };
        while let Ok(r) = rx.try_recv() {
            shared.complete(r, Some(Error::Cancelled));
        }
        while let Ok(r) = check_rx.try_recv() {
            shared.complete(r, Some(Error::Cancelled));
        }
        shared.update(|s| {
            s.checking = false;
            s.agents.invalidate();
            if let Some(fingerprint) = observed.lock().unwrap().clone() {
                s.fingerprint = Some(fingerprint);
            }
            s.generation.connection = s
                .generation
                .connection
                .checked_add(1)
                .expect("connection generation exhausted");
            s.pane = None;
            s.window = None;
            s.render_targets.clear();
        });
        let delay = match result {
            Err(request) => {
                if apply_retry_control(&shared, request) {
                    continue;
                }
                None
            }
            Ok(result) => {
                let error = result.err().unwrap_or(Error::Closed);
                shared.update(|s| s.error = Some(error));
                schedule_retry(&shared, error)
            }
        };
        // The actor retains host, credentials, trust, session, pane and size while
        // A foreground check can restart a network-exhausted cycle, never a user stop.
        wait_for_retry(&shared, &mut retry_rx, &mut check_rx, delay).await;
    }
}
// This future is polled alongside the terminal actor: neither a tmux response nor
// a full output queue can delay the independent exec probe or its deadline.
async fn check_connections(
    ssh: &Ssh,
    shared: Arc<Shared>,
    rx: &mut mpsc::Receiver<Request>,
    tx: mpsc::Sender<Request>,
) -> Result<()> {
    loop {
        let Some(request) = rx.recv().await else {
            return std::future::pending().await;
        };
        if shared.cancelled.lock().unwrap().contains(&request.id) {
            shared.update(|s| s.checking = false);
            shared.complete(request, Some(Error::Cancelled));
            continue;
        }
        if !matches!(
            tokio::time::timeout(CHECK_TIMEOUT, ssh.execute(":")).await,
            Ok(Ok((0, _)))
        ) {
            shared.complete(request, Some(Error::Network));
            return Err(Error::Network);
        }
        tx.send(request).await.map_err(|_| Error::Closed)?;
    }
}
/// How long a helper that never answered (no Python, a failed start) is left to the
/// basic fallback. One that had answered is restarted on the next poll.
const OBSERVER_RETRY: Duration = Duration::from_secs(30);
async fn observe_agents(ssh: &Ssh, shared: Arc<Shared>, tmux: bool) {
    let mut cursor = 0u64;
    let mut epoch = String::new();
    let mut scope = (String::new(), String::new());
    let mut observation_floor = 0;
    let mut observer: Option<crate::agent::Observer> = None;
    let mut answered = false;
    let mut retry_at: Option<tokio::time::Instant> = None;
    loop {
        let state = shared.state.lock().unwrap().clone();
        if tmux
            && state.observing_agents()
            && let Some(session) = state.session.clone()
        {
            if scope.0 != session {
                cursor = 0;
                epoch.clear();
            }
            if observer.is_none() && retry_at.is_none_or(|at| tokio::time::Instant::now() >= at) {
                observer = tokio::time::timeout(
                    Duration::from_secs(8),
                    crate::agent::Observer::start(ssh),
                )
                .await
                .ok()
                .and_then(|r| r.ok());
                answered = false;
                if observer.is_none() {
                    retry_at = Some(tokio::time::Instant::now() + OBSERVER_RETRY);
                }
            }
            let mut snapshot = None;
            if let Some(o) = observer.as_mut() {
                match tokio::time::timeout(Duration::from_secs(8), o.poll(&session, cursor, &epoch))
                    .await
                {
                    // An `{"error"}` reply keeps the helper; only this poll falls back.
                    Ok(Ok(v)) => {
                        answered = true;
                        snapshot = Some(v)
                            .filter(|v| v.get("server").is_some())
                            .and_then(|v| serde_json::from_value::<crate::agent::Snapshot>(v).ok())
                            .filter(|s| s.validate().is_ok());
                    }
                    // Missing Python, an exited helper, or a reply out of step: the basic
                    // fallback serves until the next start attempt.
                    _ => {
                        if let Some(o) = observer.take() {
                            o.close().await;
                        }
                        retry_at =
                            (!answered).then(|| tokio::time::Instant::now() + OBSERVER_RETRY);
                    }
                }
            }
            if snapshot.is_none() {
                snapshot = tokio::time::timeout(
                    Duration::from_secs(2),
                    crate::agent::basic_snapshot(ssh, &session),
                )
                .await
                .ok()
                .and_then(|r| r.ok());
            }
            match snapshot {
                Some(mut snapshot) => {
                    if epoch != snapshot.epoch
                        || scope != (session.clone(), snapshot.server.clone())
                    {
                        observation_floor = 0;
                        snapshot.events.clear();
                    }
                    if snapshot.gap {
                        observation_floor = snapshot.cursor;
                        snapshot.events.clear();
                    }
                    snapshot.discard_observations_through(observation_floor);
                    cursor = snapshot.cursor;
                    epoch = snapshot.epoch.clone();
                    scope = (session.clone(), snapshot.server.clone());
                    snapshot.valid = true;
                    snapshot.observed_at = crate::agent::now_ms();
                    shared.update(|s| {
                        if s.session.as_ref() == Some(&session) {
                            s.agents = snapshot;
                        }
                    });
                }
                None => {
                    shared.update(|s| s.agents.invalidate());
                    cursor = 0;
                    epoch.clear();
                }
            }
        } else {
            shared.update(|s| s.agents.invalidate());
            cursor = 0;
            epoch.clear();
            if let Some(o) = observer.take() {
                o.close().await;
            }
        }
        // One sequential poll per connection, independent of the terminal actor.
        let wait = tokio::time::sleep(Duration::from_secs(2));
        if let Some(o) = observer.as_mut() {
            tokio::select! {
                _ = wait => {}
                _ = o.drain() => {
                    observer.take().unwrap().close().await;
                    retry_at = (!answered).then(|| tokio::time::Instant::now() + OBSERVER_RETRY);
                }
            }
        } else {
            wait.await;
        }
    }
}

/// Keys typed while one is in flight go out together instead of a round trip each.
const INPUT_PIPELINE: usize = 16;
/// Checks an input request before any of it is sent.
fn admit_input(shared: &Shared, r: &Request) -> Result<()> {
    let RequestKind::Input {
        target,
        revision,
        coordinates,
        ..
    } = &r.kind
    else {
        unreachable!()
    };
    if *revision != shared.input_revision.load(Ordering::SeqCst) {
        return Err(Error::Cancelled);
    }
    if shared.target().is_none_or(|current| {
        !current.same_input_destination(target) || (*coordinates && &current != target)
    }) {
        return Err(Error::StaleGeneration);
    }
    let state = shared.state.lock().unwrap().clone();
    if !state.accepts_input() || (*coordinates && !state.accepts_pointer()) {
        return Err(Error::NotReady);
    }
    if shared.cancelled.lock().unwrap().contains(&r.id) {
        return Err(Error::Cancelled);
    }
    Ok(())
}
/// Checks a partly sent input request before its next chunks.
fn input_continues(shared: &Shared, r: &Request) -> Result<()> {
    let RequestKind::Input {
        target,
        revision,
        coordinates,
        ..
    } = &r.kind
    else {
        unreachable!()
    };
    if !shared.state.lock().unwrap().accepts_input()
        || *revision != shared.input_revision.load(Ordering::SeqCst)
    {
        return Err(Error::NotReady);
    }
    if shared.target().is_none_or(|current| {
        !current.same_input_destination(target) || (*coordinates && &current != target)
    }) {
        return Err(Error::StaleGeneration);
    }
    if shared.cancelled.lock().unwrap().contains(&r.id) {
        return Err(Error::Cancelled);
    }
    Ok(())
}
fn same_input(a: &Request, b: &Request) -> bool {
    match (&a.kind, &b.kind) {
        (
            RequestKind::Input {
                target,
                revision,
                coordinates,
                ..
            },
            RequestKind::Input {
                target: t,
                revision: r,
                coordinates: c,
                ..
            },
        ) => target == t && revision == r && coordinates == c,
        _ => false,
    }
}
/// What remains of a batch after `error` ended it: the requests none of which was written.
struct Unsent {
    requests: Vec<Request>,
    error: Option<Error>,
}
/// Sends the batch's 256-byte `send-keys` chunks in order, up to `INPUT_PIPELINE` per
/// write, checking the target before each write. A request stops at its first failed
/// chunk and is never resent; chunks already written behind it in the same write still run.
/// Each request completes as soon as its last reply is read, so a later write that is
/// dropped or fails cannot report delivered input as lost.
async fn send_input(c: &mut Control, shared: &Shared, batch: Vec<Request>) -> Unsent {
    let mut items: Vec<_> = batch.into_iter().map(|r| Some((r, 0, None))).collect();
    let finish = |items: &mut Vec<Option<(Request, usize, Option<Error>)>>| {
        for item in items.iter_mut() {
            let done = item.as_ref().is_some_and(|(r, sent, error)| {
                let RequestKind::Input { bytes, .. } = &r.kind else {
                    unreachable!()
                };
                error.is_some() || *sent == bytes.len()
            });
            if done {
                let (r, _, error) = item.take().unwrap();
                shared.complete(r, error);
            }
        }
    };
    let error = loop {
        let mut ops = Vec::new();
        let mut owners = Vec::new();
        for (i, item) in items.iter_mut().enumerate() {
            let Some((r, sent, error)) = item else {
                continue;
            };
            if ops.len() == INPUT_PIPELINE {
                break;
            }
            let RequestKind::Input { target, bytes, .. } = &r.kind else {
                unreachable!()
            };
            let check = if *sent == 0 {
                admit_input(shared, r)
            } else {
                input_continues(shared, r)
            };
            if let Err(e) = check {
                *error = Some(e);
                continue;
            }
            for chunk in bytes[*sent..].chunks(256).take(INPUT_PIPELINE - ops.len()) {
                ops.push(Operation::Send {
                    target: target.pane_id.clone(),
                    bytes: chunk.to_vec(),
                });
                owners.push(i);
                *sent += chunk.len();
            }
        }
        if ops.is_empty() {
            finish(&mut items);
            break None;
        }
        match c.commands(&ops).await {
            Ok(replies) => {
                for (i, reply) in owners.into_iter().zip(replies) {
                    if let (Err(e), Some((_, _, error))) = (reply, &mut items[i]) {
                        error.get_or_insert(e);
                    }
                }
                finish(&mut items);
            }
            Err(e) => break Some(e),
        }
    };
    let mut unsent = Unsent {
        requests: Vec::new(),
        error,
    };
    for (r, sent, e) in items.into_iter().flatten() {
        match error {
            Some(_) if e.is_none() && sent == 0 => unsent.requests.push(r),
            // Delivery of the failed write, or of what was left, is unknown.
            _ => shared.complete(r, e.or(error)),
        }
    }
    unsent
}

/// What a connection drives: tmux, with a control client while a session is attached,
/// or a login shell on a host without tmux.
// One value per connection, so the variant sizes do not matter.
#[allow(clippy::large_enum_variant)]
enum Remote {
    Tmux(Option<Control>),
    Shell(Stream),
}
enum Incoming {
    Checked(Request),
    Request(Option<Request>),
    Event(Result<Event>),
    Shell(Option<russh::ChannelMsg>),
    Refresh,
    Title,
    Metadata,
    Visibility,
}
/// One SSH connection serving requests in order until it fails or the client goes away.
struct Actor<'a> {
    ssh: &'a Ssh,
    shared: Arc<Shared>,
    remote: Remote,
    size: &'a mut Size,
    remembered: &'a mut Option<String>,
    /// Requests taken from `rx` while merging input, served before newer ones.
    backlog: VecDeque<Request>,
}
async fn connected(
    ssh: &Ssh,
    shared: Arc<Shared>,
    rx: &mut mpsc::Receiver<Request>,
    checks: &mut mpsc::Receiver<Request>,
    tmux: bool,
    size: &mut Size,
    remembered: &mut Option<String>,
) -> Result<()> {
    let remote = if tmux {
        Remote::Tmux(None)
    } else {
        Remote::Shell(ssh.shell(*size).await?)
    };
    let mut actor = Actor {
        ssh,
        shared,
        remote,
        size,
        remembered,
        backlog: VecDeque::new(),
    };
    actor.start().await?;
    // A successfully restored connection gets a fresh budget for the next outage.
    actor.shared.update(|s| s.retry = RetryState::default());
    loop {
        actor.serve_changes().await?;
        match actor.next(rx, checks).await {
            Incoming::Request(None) => return Ok(()),
            Incoming::Request(Some(r)) => actor.request(r, rx).await?,
            Incoming::Checked(check) => actor.checked(check).await?,
            Incoming::Shell(message) => actor.shell_output(message).await?,
            Incoming::Refresh | Incoming::Visibility => {}
            incoming @ (Incoming::Title | Incoming::Metadata | Incoming::Event(_)) => {
                let Remote::Tmux(Some(c)) = &mut actor.remote else {
                    unreachable!("only an attached control client produces these")
                };
                let result = match incoming {
                    Incoming::Title => c.refresh_title().await,
                    Incoming::Metadata => c.poll_metadata().await,
                    Incoming::Event(event) => match event {
                        Ok(event) => c.unsolicited(event).await,
                        Err(error) => Err(error),
                    },
                    _ => unreachable!(),
                };
                actor.recover(result).await?;
            }
        }
    }
}
impl Actor<'_> {
    async fn start(&mut self) -> Result<()> {
        match &mut self.remote {
            Remote::Tmux(control) => {
                let remote = list(self.ssh).await?;
                let session = self.shared.state.lock().unwrap().session.clone();
                self.shared.update(|s| s.remote = remote.clone());
                match session {
                    Some(id) if remote.sessions.iter().any(|s| s.id == id) => {
                        *control = Some(
                            attach(self.ssh, self.shared.clone(), id, *self.size, None).await?,
                        );
                    }
                    Some(_) => self.shared.phase(Phase::MissingSession),
                    None => self.shared.phase(Phase::Selecting),
                }
            }
            Remote::Shell(_) => {
                self.shared.update(|s| {
                    s.phase = Phase::Shell;
                    s.retry = RetryState::default();
                    s.pane = Some("shell".into());
                    s.generation.pane += 1;
                });
                let size = *self.size;
                self.shared.emit(OutputKind::Resize, size, &[]).await?;
                self.shared
                    .emit(OutputKind::ResetBegin, size, b"\x1bc")
                    .await?;
                self.shared.emit(OutputKind::ResetEnd, size, &[]).await?;
            }
        }
        Ok(())
    }
    /// Commands also consume notifications while waiting for their response.
    /// Serve those changes before waiting for another unsolicited event.
    async fn serve_changes(&mut self) -> Result<()> {
        let Remote::Tmux(Some(c)) = &mut self.remote else {
            return Ok(());
        };
        if !c.dirty && c.paused.is_empty() {
            return Ok(());
        }
        let result = async {
            if c.dirty {
                c.refresh_screen_if_needed(false).await?;
            }
            if !c.paused.is_empty() {
                c.sync_screens(false, false).await?;
            }
            Ok(())
        }
        .await;
        self.recover(result).await
    }
    async fn next(
        &mut self,
        rx: &mut mpsc::Receiver<Request>,
        checks: &mut mpsc::Receiver<Request>,
    ) -> Incoming {
        if let Some(r) = self.backlog.pop_front() {
            return Incoming::Request(Some(r));
        }
        match &mut self.remote {
            Remote::Tmux(Some(c)) => {
                c.titles.reconcile(self.shared.title_scope());
                let title_deadline = c.titles.deadline();
                let metadata_due = c.metadata.as_ref().map(|m| m.due);
                let pending_refresh = c.dirty || !c.paused.is_empty();
                // A slow link may receive another change during every refresh.
                // Keep pending user requests eligible instead of starving input.
                tokio::select! {
                    Some(check) = checks.recv() => Incoming::Checked(check),
                    r = rx.recv() => Incoming::Request(r),
                    e = c.next() => Incoming::Event(e),
                    _ = self.shared.title_changed.notified() => Incoming::Visibility,
                    _ = async {
                        match title_deadline {
                            Some(at) => tokio::time::sleep_until(at).await,
                            None => std::future::pending().await,
                        }
                    } => Incoming::Title,
                    _ = async {
                        match metadata_due {
                            Some(at) => tokio::time::sleep_until(at).await,
                            None => std::future::pending().await,
                        }
                    } => Incoming::Metadata,
                    _ = std::future::ready(()), if pending_refresh => Incoming::Refresh,
                }
            }
            Remote::Shell(sh) => tokio::select! {
                Some(check) = checks.recv() => Incoming::Checked(check),
                r = rx.recv() => Incoming::Request(r),
                m = sh.wait() => Incoming::Shell(m),
            },
            Remote::Tmux(None) => tokio::select! {
                Some(check) = checks.recv() => Incoming::Checked(check),
                r = rx.recv() => Incoming::Request(r),
            },
        }
    }
    /// A vanished tmux session leaves the connection up but detached.
    async fn recover(&mut self, result: Result<()>) -> Result<()> {
        if result == Err(Error::MissingSession) {
            self.missing_session().await
        } else {
            result
        }
    }
    /// After a request was completed with `error`: a vanished session detaches, a lost or
    /// confused transport ends the connection, and anything else was the request's own.
    async fn after_request(&mut self, error: Option<Error>) -> Result<()> {
        match error {
            Some(Error::MissingSession) => self.missing_session().await,
            Some(error @ (Error::Network | Error::Protocol)) => Err(error),
            _ => Ok(()),
        }
    }
    async fn missing_session(&mut self) -> Result<()> {
        if let Remote::Tmux(control) = &mut self.remote
            && let Some(c) = control.take()
        {
            let _ = c.stream.close().await;
        }
        let remote = list(self.ssh).await?;
        self.shared.update(|s| {
            s.remote = remote;
            s.window = None;
            s.render_targets.clear();
            s.phase = Phase::MissingSession;
            s.error = Some(Error::MissingSession);
            s.pane = None;
            s.generation.pane = s.generation.pane.saturating_add(1);
        });
        Ok(())
    }
    /// A check that reached the server; the connection's generation cannot have changed
    /// since, as it only changes after this actor ends.
    async fn checked(&mut self, check: Request) -> Result<()> {
        let result = if self.shared.cancelled.lock().unwrap().contains(&check.id) {
            Err(Error::Cancelled)
        } else {
            match &mut self.remote {
                // A healthy resume keeps the existing renderer and stream.
                // Only changed/paused panes need a fresh capture.
                Remote::Tmux(Some(c)) => c.refresh_screen_if_needed(false).await,
                Remote::Tmux(None) => list(self.ssh)
                    .await
                    .map(|remote| self.shared.update(|s| s.remote = remote)),
                Remote::Shell(_) => Ok(()),
            }
        };
        let error = result.err();
        self.shared.complete(check, error);
        self.after_request(error).await?;
        self.shared.update(|s| s.checking = false);
        Ok(())
    }
    async fn shell_output(&mut self, message: Option<russh::ChannelMsg>) -> Result<()> {
        match message {
            Some(russh::ChannelMsg::Data { data }) => {
                self.shared.emit(OutputKind::Live, *self.size, &data).await
            }
            None | Some(russh::ChannelMsg::Close | russh::ChannelMsg::ExitStatus { .. }) => {
                Err(Error::Network)
            }
            Some(_) => Ok(()),
        }
    }
    async fn request(&mut self, r: Request, rx: &mut mpsc::Receiver<Request>) -> Result<()> {
        if self.shared.cancelled.lock().unwrap().contains(&r.id) {
            self.shared.complete(r, Some(Error::Cancelled));
            return Ok(());
        }
        if let (RequestKind::Input { .. }, Remote::Tmux(Some(c))) = (&r.kind, &mut self.remote) {
            let mut batch = vec![r];
            while batch.len() < INPUT_PIPELINE {
                match rx.try_recv() {
                    Ok(next) if same_input(&batch[0], &next) => batch.push(next),
                    Ok(next) => {
                        self.backlog.push_back(next);
                        break;
                    }
                    Err(_) => break,
                }
            }
            let unsent = send_input(c, &self.shared, batch).await;
            for r in unsent.requests.into_iter().rev() {
                self.backlog.push_front(r);
            }
            return self.after_request(unsent.error).await;
        }
        let result = match &r.kind {
            RequestKind::Input { bytes, .. } => self.shell_input(&r, bytes).await,
            RequestKind::Action(action) => self.action(action).await,
        };
        let error = result.err();
        if error.is_some() {
            self.shared.update(|s| s.resizing = false);
        }
        self.shared.complete(r, error);
        self.after_request(error).await
    }
    /// tmux input goes through `send_input`; this is the shell's, or none at all.
    async fn shell_input(&mut self, r: &Request, bytes: &[u8]) -> Result<()> {
        admit_input(&self.shared, r)?;
        let Remote::Shell(sh) = &self.remote else {
            return Err(Error::NotReady);
        };
        sh.data(bytes).await.map_err(|_| Error::Network)
    }
    async fn action(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::RetryNow | Action::StopRetrying | Action::CheckConnection => {
                Err(Error::NotReady)
            }
            Action::SelectSession { id } => self.attach_session(id.clone(), None).await,
            Action::SelectTarget {
                session,
                window,
                pane,
            } => {
                self.select_target(session, window.as_deref(), pane.as_deref())
                    .await
            }
            Action::SelectPane { id } => {
                let Remote::Tmux(Some(c)) = &mut self.remote else {
                    return Err(Error::NotReady);
                };
                c.select_pane(id.clone()).await?;
                *self.remembered = Some(id.clone());
                Ok(())
            }
            Action::Manage { operation } => self.manage(operation).await,
            Action::AgentSetup { operation, token } => {
                let mut args = vec![operation.as_str()];
                if let Some(token) = token {
                    args.extend(["--token", token]);
                }
                let result = crate::agent::setup(self.ssh, &args).await.unwrap_or_else(|_| serde_json::json!({"error": t("연동 검사 실패: Python 3·tmux·CLI 설치 및 연결을 확인하세요.")}));
                self.shared.update(|s| s.agent_setup = Some(result));
                Ok(())
            }
            Action::Refresh => match &mut self.remote {
                Remote::Tmux(Some(c)) => c.refresh_screen_if_needed(false).await,
                Remote::Tmux(None) => {
                    let remote = list(self.ssh).await?;
                    self.shared.update(|s| s.remote = remote);
                    Ok(())
                }
                Remote::Shell(_) => Ok(()),
            },
            Action::Snapshot => match &mut self.remote {
                Remote::Tmux(Some(c)) => c.sync_screens(false, true).await,
                _ => Ok(()),
            },
            Action::Resize { columns, rows } => self.resize(*columns, *rows).await,
        }
    }
    /// Replaces any control client with one attached to `session`.
    async fn attach_session(&mut self, session: String, pane: Option<String>) -> Result<()> {
        let Remote::Tmux(control) = &mut self.remote else {
            return Err(Error::NotReady);
        };
        if let Some(c) = control.take() {
            let _ = c.stream.close().await;
        }
        *control = Some(attach(self.ssh, self.shared.clone(), session, *self.size, pane).await?);
        Ok(())
    }
    async fn select_target(
        &mut self,
        session: &str,
        window: Option<&str>,
        pane: Option<&str>,
    ) -> Result<()> {
        let Remote::Tmux(control) = &mut self.remote else {
            return Err(Error::NotReady);
        };
        let selected = if let Some(c) = control.as_mut() {
            // The attached client already lists every session.
            c.refresh().await?;
            let state = self.shared.state.lock().unwrap().clone();
            resolve_target(&state.remote, session, window, pane)
                // The refresh may have moved the active pane.
                .inspect_err(|_| c.dirty = true)?
        } else {
            resolve_target(&list(self.ssh).await?, session, window, pane)?
        };
        let current = self.shared.state.lock().unwrap().session.clone();
        if let Some(c) = control
            .as_mut()
            .filter(|_| current.as_deref() == Some(session))
        {
            c.select_pane(selected.clone()).await?;
        } else {
            self.attach_session(session.to_owned(), Some(selected.clone()))
                .await?;
        }
        *self.remembered = Some(selected);
        Ok(())
    }
    async fn manage(&mut self, operation: &Operation) -> Result<()> {
        let c = match &mut self.remote {
            Remote::Tmux(Some(c)) => c,
            // Without an attached session only session-independent operations run.
            Remote::Tmux(None) => {
                if !matches!(
                    operation,
                    Operation::NewSession { .. }
                        | Operation::NewWindow { .. }
                        | Operation::RenameSession { .. }
                        | Operation::RenameWindow { .. }
                        | Operation::RenamePane { .. }
                        | Operation::KillSession { .. }
                        | Operation::KillWindow { .. }
                        | Operation::KillPane { .. }
                ) {
                    return Err(Error::NotReady);
                }
                let (code, _) = self.ssh.execute_tmux(&Command::new(operation)?).await?;
                if code != 0 {
                    return Err(Error::InvalidArgument);
                }
                let remote = list(self.ssh).await?;
                self.shared.update(|s| s.remote = remote);
                return Ok(());
            }
            Remote::Shell(_) => return Err(Error::NotReady),
        };
        // Directional selection starts at this connection's displayed pane.
        // Other connections keep their own PaneTarget, even in the same tmux window.
        let selected_window = match operation {
            Operation::FocusPane { target, .. } | Operation::Split { target, .. } => {
                let state = self.shared.state.lock().unwrap().clone();
                if state.pane.as_ref() != Some(target) {
                    return Err(Error::StaleGeneration);
                }
                Some(
                    state
                        .remote
                        .panes
                        .iter()
                        .find(|p| &p.id == target)
                        .ok_or(Error::InvalidArgument)?
                        .window
                        .clone(),
                )
            }
            Operation::SelectWindow { target } => Some(target.clone()),
            _ => None,
        };
        if let Operation::SelectPane { target } = operation {
            return c.select_pane(target.clone()).await;
        }
        c.refresh_screen_after(std::slice::from_ref(operation), false)
            .await?;
        if let Some(window) = selected_window {
            let remote = self.shared.state.lock().unwrap().remote.clone();
            let pane = remote
                .panes
                .iter()
                .find(|p| p.window == window && p.active)
                .ok_or(Error::InvalidArgument)?
                .id
                .clone();
            c.select_pane(pane.clone()).await?;
            *self.remembered = Some(pane);
        }
        Ok(())
    }
    async fn resize(&mut self, columns: u16, rows: u16) -> Result<()> {
        let next = Size::new(columns, rows)?;
        if next == *self.size {
            return Ok(());
        }
        // Retain the requested viewport even if transport fails below;
        // the next attachment must start at the latest dimensions.
        *self.size = next;
        match &mut self.remote {
            Remote::Shell(sh) => {
                sh.window_change(columns.into(), rows.into(), 0, 0)
                    .await
                    .map_err(|_| Error::Network)?;
                self.shared.update(|s| s.generation.size += 1);
                self.shared.emit(OutputKind::Resize, next, &[]).await
            }
            Remote::Tmux(Some(c)) => c
                .refresh_screen_after(&[Operation::ClientSize { columns, rows }], true)
                .await
                .map(drop),
            Remote::Tmux(None) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot_writer() -> (Arc<OutputQueue>, SnapshotWriter) {
        let output = Arc::new(OutputQueue::default());
        let id = output.subscribe().unwrap();
        let target = PaneTarget {
            connection: 1,
            pane_id: "%1".into(),
            generation: Generation {
                connection: 1,
                pane: 0,
                size: 1,
            },
        };
        let writer = SnapshotWriter::new(output.clone(), id, target, Size::default());
        (output, writer)
    }
    #[tokio::test]
    async fn snapshot_batches_preserve_main_alternate_modes_and_pending_bytes() {
        let (output, mut writer) = snapshot_writer();
        let mut expected = Vec::new();
        for alternate in [false, true] {
            if alternate {
                writer.write(b"\x1b[?1049h").await.unwrap();
                expected.extend_from_slice(b"\x1b[?1049h");
            }
            let mut rows = Vec::new();
            for i in 1..=40 {
                rows.push("한글😀\\\\shell or TUI".as_bytes().to_vec());
                expected.extend_from_slice(format!("\x1b[{i};1H한글😀\\shell or TUI").as_bytes());
            }
            writer.rows(rows).await.unwrap();
        }
        let suffix = b"\x1b[0m\x1b[1;40r\x1b[5;10H\x1b[?2004h\x1b[38;2;";
        writer.write(suffix).await.unwrap();
        expected.extend_from_slice(suffix);
        assert_eq!(
            output.metrics(),
            (0, 0),
            "Small rows share one renderer write"
        );
        writer.flush().await.unwrap();
        let event = output.poll(writer.subscription).unwrap().unwrap();
        assert_eq!(event.kind, OutputKind::ResetChunk);
        assert_eq!(event.bytes, expected);
        assert!(output.poll(writer.subscription).unwrap().is_none());
        assert_eq!(
            output.metrics(),
            (expected.len(), 1),
            "Payload stays charged until renderer ACK"
        );
        output.ack(writer.subscription, event.sequence).unwrap();
        assert_eq!(output.metrics(), (0, 0));
    }
    #[tokio::test]
    async fn snapshot_batches_split_at_limit_without_losing_utf8_or_escape_bytes() {
        let (output, mut writer) = snapshot_writer();
        let mut bytes = vec![b'a'; MAX_BATCH - 1];
        bytes.extend_from_slice("한글😀\x1b[?1049h".as_bytes());
        bytes.extend_from_slice(&vec![b'b'; MAX_BATCH]);
        writer.write(&bytes).await.unwrap();
        writer.flush().await.unwrap();
        writer.flush().await.unwrap();
        let mut restored = Vec::new();
        let mut writes = 0;
        while let Some(event) = output.poll(writer.subscription).unwrap() {
            assert!(!event.bytes.is_empty() && event.bytes.len() <= MAX_BATCH);
            restored.extend_from_slice(&event.bytes);
            output.ack(writer.subscription, event.sequence).unwrap();
            writes += 1;
        }
        assert_eq!(restored, bytes);
        assert_eq!(writes, bytes.len().div_ceil(MAX_BATCH));
        assert_eq!(writer.bytes.capacity(), MAX_BATCH);
        assert_eq!(output.metrics(), (0, 0));
    }
    #[tokio::test]
    async fn snapshot_batch_backpressure_is_released_by_unsubscribe_without_leaking_to_successor() {
        let (output, mut writer) = snapshot_writer();
        let id = writer.subscription;
        for _ in 0..crate::flow::MAX_OUTPUT / MAX_BATCH {
            writer.write(&vec![b'x'; MAX_BATCH]).await.unwrap();
        }
        let task = tokio::spawn(async move { writer.write(&vec![b'y'; MAX_BATCH]).await });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        output.unsubscribe(id);
        let next = output.subscribe().unwrap();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(output.poll(next).unwrap().is_none());
        assert_eq!(output.metrics(), (0, 0));
    }
    fn host(port: u16) -> HostProfile {
        HostProfile {
            id: "retry-fixture".into(),
            alias: "retry-fixture".into(),
            hostname: "127.0.0.1".into(),
            port,
            username: "fixture".into(),
            auth: model::AuthKind::Password,
            credential_ref: "fixture".into(),
            favorite: false,
            last_connected: 0,
            last_session: Some("$12".into()),
            last_pane: Some("%34".into()),
        }
    }
    #[tokio::test]
    async fn session_browsing_is_exactly_the_settled_tmux_phases() {
        let c = Connection::start(host(1), Credential::default(), None);
        let mut s = (*c.state()).clone();
        c.close();
        use Phase::*;
        for phase in [
            Connecting,
            TrustRequired,
            Authenticating,
            Selecting,
            Restoring,
            Ready,
            Reconnecting,
            MissingSession,
            Failed,
            Shell,
            Closed,
        ] {
            let expected = matches!(phase, Selecting | Ready | MissingSession);
            s.phase = phase.clone();
            assert_eq!(s.browses_sessions(), expected, "{phase:?}");
        }
    }
    #[tokio::test]
    async fn retry_controls_hide_during_a_check_that_still_admits_retry() {
        let c = Connection::start(host(1), Credential::default(), None);
        let mut s = (*c.state()).clone();
        c.close();
        s.phase = Phase::Failed;
        s.error = Some(Error::Network);
        s.retry.attempt = MAX_RETRIES;
        let failed = s.derived();
        assert!(failed.retry_available && failed.retry_exhausted && !failed.reconnecting);
        assert!(s.offers_retry() && failed.retry_message.is_some());
        // A check after exhausted retries still counts as a retry cycle for menus and guards.
        s.checking = true;
        assert!(s.can_retry_now() && s.can_stop_retrying());
        assert!(s.derived().retry_available && !s.offers_retry() && !s.accepts_input());
        s.checking = false;
        s.phase = Phase::Restoring;
        s.error = None;
        s.retry.connecting = true;
        let reattach = s.derived();
        assert!(reattach.reconnecting && reattach.retry_available && !reattach.retry_exhausted);
        s.retry.connecting = false;
        s.resizing = true;
        assert!(s.accepts_input() && !s.accepts_pointer() && !s.derived().retry_available);
        s.render_targets.insert(
            "%34".into(),
            crate::flow::PaneTarget {
                connection: s.handle,
                pane_id: "%34".into(),
                generation: s.generation.clone(),
            },
        );
        // Same bytes as the plain state plus `derived`: target strings must match output events.
        let plain = serde_json::to_string(&s).unwrap();
        let view = serde_json::to_string(&s.view()).unwrap();
        assert!(view.starts_with(plain.strip_suffix('}').unwrap()));
        let target = serde_json::to_string(&s.render_targets["%34"]).unwrap();
        assert!(view.contains(&target));
        // Output follows render targets in Ready/Restoring, not input focus.
        let rendered = s.render_targets["%34"].clone();
        s.phase = Phase::Ready;
        s.pane = Some("%35".into());
        assert!(s.accepts_output(&rendered) && s.observing_agents());
        let mut stale = rendered.clone();
        stale.generation.size += 1;
        assert!(!s.accepts_output(&stale));
        s.phase = Phase::Reconnecting;
        assert!(!s.accepts_output(&rendered) && !s.observing_agents());
        s.phase = Phase::Shell;
        s.pane = Some("shell".into());
        let shell = s.input_target().unwrap();
        assert!(s.accepts_output(&shell));
    }
    #[tokio::test]
    async fn title_publication_rejects_stale_responses_and_only_changes_title() {
        let c = Connection::start(host(1), Credential::default(), None);
        assert!(c.shared.title_scope().is_none()); // disabled by default
        c.shared.update(|s| {
            s.phase = Phase::Ready;
            s.pane = Some("%34".into());
            s.window = Some("@2".into());
            s.remote.panes = model::panes(&[b"%34|@2|80|24|1||sh|old|0|0".to_vec()]).unwrap();
        });
        let before = serde_json::to_value(&*c.state()).unwrap();
        c.set_title_updates_enabled(true);
        assert_eq!(serde_json::to_value(&*c.state()).unwrap(), before);
        let scope = c.shared.title_scope().unwrap();
        c.shared.apply_title(&scope, "old".into());
        assert_eq!(serde_json::to_value(&*c.state()).unwrap(), before);
        c.shared.apply_title(&scope, "한글 🚀".into());
        let mut expected = before.clone();
        expected["sequence"] = (c.state().sequence).into();
        expected["remote"]["panes"][0]["title"] = "한글 🚀".into();
        assert_eq!(serde_json::to_value(&*c.state()).unwrap(), expected);
        assert_eq!(c.state().sequence, before["sequence"].as_u64().unwrap() + 1);
        c.set_title_updates_enabled(false);
        c.set_title_updates_enabled(true);
        let hidden = c.state().sequence;
        c.shared.apply_title(&scope, "stale hide/return".into());
        assert_eq!(c.state().sequence, hidden);
        for change_connection in [false, true] {
            let old = c.shared.title_scope().unwrap();
            c.shared.update(|s| {
                if change_connection {
                    s.generation.connection += 1;
                } else {
                    s.generation.pane += 1;
                }
            });
            let sequence = c.state().sequence;
            c.shared.apply_title(&old, "stale destination".into());
            assert_eq!(c.state().sequence, sequence);
        }
        let fresh = c.shared.title_scope().unwrap();
        c.shared.update(|s| s.generation.size += 1);
        c.shared.apply_title(&fresh, String::new());
        assert!(c.state().remote.panes[0].title.is_empty());
        c.shared.update(|s| s.phase = Phase::Reconnecting);
        let sequence = c.state().sequence;
        c.shared.apply_title(&fresh, "stale phase".into());
        assert_eq!(c.state().sequence, sequence);
        c.close();
        c.join().await;
    }
    // Yield to real loopback I/O without advancing the paused retry clock.
    async fn until(mut predicate: impl FnMut() -> bool) {
        for _ in 0..100_000 {
            if predicate() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("connection did not reach the expected state");
    }
    #[tokio::test(start_paused = true)]
    async fn immediate_then_two_one_second_retries_preserve_user_stop() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicU64::new(0));
        let count = accepted.clone();
        let server = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                drop(socket);
            }
        });
        let c = Connection::start(
            host(port),
            Credential::default(),
            Some("saved-fingerprint".into()),
        );
        let handle = c.state().handle;
        until(|| c.state().retry.wait_seconds == Some(1)).await;
        assert_eq!(
            accepted.load(Ordering::SeqCst),
            2,
            "first retry needs no clock advance"
        );
        for attempt in 1..3 {
            let seconds = 1;
            assert_eq!(c.state().retry.attempt as usize, attempt);
            assert_eq!(c.state().retry.wait_seconds, Some(seconds));
            tokio::time::advance(Duration::from_millis(999)).await;
            tokio::task::yield_now().await;
            assert_eq!(c.state().retry.attempt as usize, attempt);
            tokio::time::advance(Duration::from_millis(1)).await;
            until(|| {
                let s = c.state();
                s.retry.attempt as usize == attempt + 1
                    && (s.retry.wait_seconds.is_some() || s.phase == Phase::Failed)
            })
            .await;
        }
        assert_eq!(accepted.load(Ordering::SeqCst), 4); // first attempt + three retries
        assert_eq!(c.state().phase, Phase::Failed);
        assert!(c.state().can_retry_now());
        assert!(!c.state().can_stop_retrying());
        tokio::time::advance(Duration::from_secs(3600)).await;
        tokio::task::yield_now().await;
        assert_eq!(accepted.load(Ordering::SeqCst), 4);

        // Foreground recovery starts immediately, coalescing duplicate resumes.
        let retry = c.action(Action::CheckConnection).unwrap();
        c.action(Action::CheckConnection).unwrap();
        until(|| accepted.load(Ordering::SeqCst) == 5 && c.state().retry.wait_seconds == Some(1))
            .await;
        assert_eq!(c.state().retry.attempt, 1);
        assert!(
            c.completions()
                .iter()
                .any(|r| r.request_id == retry && r.error.is_none())
        );
        // Manual retry also skips an existing delay, without advancing the clock.
        let manual = c.action(Action::RetryNow).unwrap();
        assert_eq!(c.action(Action::RetryNow).unwrap(), manual);
        until(|| accepted.load(Ordering::SeqCst) == 6 && c.state().retry.wait_seconds == Some(1))
            .await;
        let stop = c.action(Action::StopRetrying).unwrap();
        until(|| c.state().retry.stopped).await;
        assert!(
            c.completions()
                .iter()
                .any(|r| r.request_id == stop && r.error.is_none())
        );
        assert_eq!(c.state().phase, Phase::Failed);
        c.action(Action::CheckConnection).unwrap();
        tokio::time::advance(Duration::from_secs(3600)).await;
        tokio::task::yield_now().await;
        assert!(c.state().retry.stopped);
        assert_eq!(accepted.load(Ordering::SeqCst), 6);
        assert_eq!(c.state().handle, handle);
        assert_eq!(c.state().session.as_deref(), Some("$12"));
        assert_eq!(c.state().fingerprint.as_deref(), Some("saved-fingerprint"));
        assert!(c.state().can_retry_now());
        c.action(Action::RetryNow).unwrap();
        until(|| accepted.load(Ordering::SeqCst) == 7 && c.state().retry.wait_seconds == Some(1))
            .await;
        assert!(!c.state().retry.stopped);
        c.close();
        c.join().await;
        server.abort();
    }
    #[tokio::test(start_paused = true)]
    async fn stop_cancels_in_flight_handshake_without_waiting_for_timeout() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let c = Connection::start(
            host(listener.local_addr().unwrap().port()),
            Credential::default(),
            None,
        );
        let closed = Arc::new(AtomicBool::new(false));
        let observed = closed.clone();
        let accepted = Arc::new(AtomicBool::new(false));
        let ready = accepted.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            ready.store(true, Ordering::SeqCst);
            let _ = socket.read_to_end(&mut Vec::new()).await;
            observed.store(true, Ordering::SeqCst);
        });
        until(|| accepted.load(Ordering::SeqCst)).await;
        let before = tokio::time::Instant::now();
        let retry = c.action(Action::RetryNow).unwrap();
        assert!(
            c.completions()
                .iter()
                .any(|r| r.request_id == retry && r.error.is_none())
        );
        assert!(
            !closed.load(Ordering::SeqCst),
            "retry must reuse an in-flight handshake"
        );
        c.action(Action::StopRetrying).unwrap();
        until(|| c.state().retry.stopped && closed.load(Ordering::SeqCst)).await;
        assert_eq!(tokio::time::Instant::now(), before);
        assert!(c.state().can_retry_now());
        assert_eq!(c.state().session.as_deref(), Some("$12"));
        c.close();
        c.join().await;
        server.await.unwrap();
    }
    #[tokio::test]
    async fn checks_coalesce_with_reserved_capacity_and_never_restart_connecting_or_closed() {
        let c = Connection::start(host(1), Credential::default(), None);
        let ignored = c.action(Action::CheckConnection).unwrap();
        assert!(!c.state().checking);
        assert!(
            c.completions()
                .iter()
                .any(|r| r.request_id == ignored && r.error.is_none())
        );
        c.shared.update(|s| {
            s.phase = Phase::Ready;
            s.pane = Some("%1".into());
            s.retry.attempt = MAX_RETRIES;
        });
        c.shared.phase(Phase::Ready);
        assert_eq!(c.state().retry.attempt, 0);
        let mut legacy = serde_json::to_value(&*c.state()).unwrap();
        legacy.as_object_mut().unwrap().remove("checking");
        assert!(!serde_json::from_value::<State>(legacy).unwrap().checking);
        // Fill the ordinary queue; control admission must still succeed immediately.
        for _ in 0..32 {
            c.input(c.target().unwrap(), vec![1]).unwrap();
        }
        let check = c.action(Action::CheckConnection).unwrap();
        for _ in 0..10 {
            assert_eq!(c.action(Action::CheckConnection).unwrap(), check);
        }
        assert_eq!(c.action(Action::RetryNow).unwrap(), check);
        assert!(c.state().checking);
        assert_eq!(c.input(c.target().unwrap(), vec![1]), Err(Error::NotReady));
        c.close();
        c.join().await;
        let results = c.completions();
        assert_eq!(results.len(), 33);
        assert!(results.iter().all(|r| r.error == Some(Error::Cancelled)));
        assert_eq!(c.action(Action::CheckConnection), Err(Error::Closed));
    }
    #[tokio::test]
    async fn size_only_restore_accepts_bounded_input_and_rejects_recovery_or_changed_target() {
        let c = Connection::start(host(1), Credential::default(), None);
        c.shared.update(|s| {
            s.phase = Phase::Restoring;
            s.resizing = true;
            s.retry.connecting = false;
            s.pane = Some("%1".into());
        });
        let target = c.target().unwrap();
        assert!(c.state().accepts_input());
        assert!(!c.state().accepts_pointer());
        c.shared.update(|s| s.generation.size += 1);
        c.input(target.clone(), "한글abc\u{1b}[A".as_bytes().to_vec())
            .unwrap();
        assert_eq!(
            c.input(c.target().unwrap(), b"\x1b[<0;1;1M".to_vec()),
            Err(Error::NotReady)
        );
        c.shared.phase(Phase::Ready);
        assert_eq!(
            c.input(target.clone(), b"\x1b[<0;1;1M".to_vec()),
            Err(Error::StaleGeneration)
        );
        c.shared.update(|s| {
            s.phase = Phase::Restoring;
            s.resizing = true;
        });
        c.shared.update(|s| s.generation.pane += 1);
        assert_eq!(c.input(target, vec![1]), Err(Error::StaleGeneration));
        c.shared.update(|s| s.resizing = false);
        assert_eq!(c.input(c.target().unwrap(), vec![1]), Err(Error::NotReady));
        c.shared.update(|s| {
            s.resizing = true;
            s.checking = true;
        });
        assert_eq!(c.input(c.target().unwrap(), vec![1]), Err(Error::NotReady));
        c.close();
        c.join().await;
        assert!(
            c.completions()
                .iter()
                .all(|r| r.error == Some(Error::Cancelled))
        );
    }
    #[tokio::test]
    async fn bounded_requests_cancel_and_close_before_connect() {
        let host = HostProfile {
            id: "fixture".into(),
            alias: "fixture".into(),
            hostname: "127.0.0.1".into(),
            port: 1,
            username: "fixture".into(),
            auth: model::AuthKind::Password,
            credential_ref: "fixture".into(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        };
        let c = Connection::start(host, Credential::default(), None);
        c.shared.update(|s| {
            s.phase = Phase::Ready;
            s.pane = Some("%1".into());
            s.generation.pane = 1;
        });
        let target = c.target().unwrap();
        let ids = (0..32)
            .map(|_| c.input(target.clone(), vec![0; 65536]).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(c.input(target.clone(), vec![1]), Err(Error::QueueFull));
        assert_eq!(c.input(target.clone(), vec![]), Err(Error::InvalidArgument));
        c.cancel(ids[0]).unwrap();
        c.cancel(ids[0]).unwrap();
        c.shared.update(|s| s.pane = Some("%2".into()));
        assert_eq!(
            c.input(target.clone(), vec![1]),
            Err(Error::StaleGeneration)
        );
        c.shared.phase(Phase::Reconnecting);
        assert_eq!(c.input(c.target().unwrap(), vec![1]), Err(Error::NotReady));
        c.close();
        c.close();
        tokio::time::timeout(Duration::from_secs(1), c.join())
            .await
            .unwrap();
        assert_eq!(c.state().phase, Phase::Closed);
        assert_eq!(c.output().metrics(), (0, 0));
        assert_eq!(c.input(target, vec![1]), Err(Error::Closed));
        let completed = c.completions();
        assert_eq!(completed.len(), 32);
        assert!(
            completed
                .iter()
                .all(|r| r.outcome == CompletionOutcome::Cancelled)
        );
        assert!(c.shared.requests.lock().unwrap().is_empty());
        assert!(c.completions().is_empty());
    }
}
