//! Connection actor: all remote commands and input are serialized on one task.
use crate::{
    Error, Result,
    command::{Command, Operation},
    flow::{Generation, OutputKind, OutputQueue, PaneTarget},
    model::{self, Credential, HostProfile, RemoteState, Size},
    protocol::{Event, Parser},
    ssh::{self, Ssh, Stream},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc};
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
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
    pub generation: Generation,
    pub session: Option<String>,
    pub pane: Option<String>,
    pub remote: RemoteState,
    pub fingerprint: Option<String>,
    pub error: Option<Error>,
    pub diagnostics: Vec<ssh::Probe>,
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
    SelectSession { id: String },
    SelectPane { id: String },
    Manage { operation: Operation },
    Refresh,
    Snapshot,
    Resize { columns: u16, rows: u16 },
}
enum RequestKind {
    Input { target: PaneTarget, bytes: Vec<u8> },
    Action(Action),
}
struct Request {
    id: u64,
    kind: RequestKind,
    permit: OwnedSemaphorePermit,
}
struct Shared {
    state: Mutex<State>,
    output: Arc<OutputQueue>,
    cancel: AtomicBool,
    stop: Notify,
    done: AtomicBool,
    finished: Notify,
    results: Mutex<VecDeque<(Completion, Option<OwnedSemaphorePermit>)>>,
    cancelled: Mutex<HashSet<u64>>,
    requests: Mutex<HashSet<u64>>,
}
pub struct Connection {
    shared: Arc<Shared>,
    tx: mpsc::Sender<Request>,
    slots: Arc<Semaphore>,
    next: AtomicU64,
}
impl Shared {
    fn update(&self, f: impl FnOnce(&mut State)) {
        let mut s = self.state.lock().unwrap();
        f(&mut s);
        s.sequence = s.sequence.checked_add(1).expect("state sequence exhausted");
    }
    fn phase(&self, p: Phase) {
        self.update(|s| s.phase = p);
    }
    fn target(&self) -> Option<PaneTarget> {
        let s = self.state.lock().unwrap();
        s.pane.as_ref().map(|p| PaneTarget {
            connection: s.handle,
            pane_id: p.clone(),
            generation: s.generation.clone(),
        })
    }
    fn complete(&self, r: Request, error: Option<Error>) {
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
            Some(r.permit),
        ));
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
            state: Mutex::new(State {
                sequence: 0,
                handle,
                phase: Phase::Connecting,
                generation: Generation {
                    connection: 1,
                    pane: 0,
                    size: 0,
                },
                session: host.last_session.clone(),
                pane: None,
                remote: Default::default(),
                fingerprint: None,
                error: None,
                diagnostics: vec![],
            }),
            output: Arc::new(OutputQueue::default()),
            cancel: AtomicBool::new(false),
            stop: Notify::new(),
            done: AtomicBool::new(false),
            finished: Notify::new(),
            results: Mutex::new(VecDeque::new()),
            cancelled: Mutex::new(HashSet::new()),
            requests: Mutex::new(HashSet::new()),
        });
        let (tx, rx) = mpsc::channel(32);
        let result = Arc::new(Self {
            shared: shared.clone(),
            tx,
            slots: Arc::new(Semaphore::new(33)),
            next: AtomicU64::new(1),
        });
        tokio::spawn(async move {
            let stopped = shared.stop.notified();
            tokio::pin!(stopped);
            stopped.as_mut().enable();
            if !shared.cancel.load(Ordering::SeqCst) {
                tokio::select! {biased;_=stopped=>{},_=run(shared.clone(),host,credential,trusted,rx)=>{}}
            }
            shared.output.close();
            // Close cancels outstanding work without losing its completion record.
            // Release permits now; the at-most-33 metadata records remain drainable.
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
                s.pane = None;
                s.generation.connection = s.generation.connection.saturating_add(1);
            });
            shared.done.store(true, Ordering::SeqCst);
            shared.finished.notify_waiters();
        });
        result
    }
    pub fn state(&self) -> State {
        self.shared.state.lock().unwrap().clone()
    }
    pub fn target(&self) -> Option<PaneTarget> {
        self.shared.target()
    }
    pub fn output(&self) -> Arc<OutputQueue> {
        self.shared.output.clone()
    }
    fn submit(&self, kind: RequestKind) -> Result<u64> {
        if self.shared.cancel.load(Ordering::SeqCst) || self.shared.done.load(Ordering::SeqCst) {
            return Err(Error::Closed);
        }
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::QueueFull)?;
        let id = self
            .next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| x.checked_add(1))
            .map_err(|_| Error::Closed)?;
        self.shared.requests.lock().unwrap().insert(id);
        if self.tx.try_send(Request { id, kind, permit }).is_err() {
            self.shared.requests.lock().unwrap().remove(&id);
            return Err(Error::QueueFull);
        }
        Ok(id)
    }
    pub fn input(&self, target: PaneTarget, bytes: Vec<u8>) -> Result<u64> {
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(Error::InvalidArgument);
        }
        let s = self.state();
        if s.phase == Phase::Closed {
            return Err(Error::Closed);
        }
        if !matches!(s.phase, Phase::Ready | Phase::Shell) {
            return Err(Error::NotReady);
        }
        if self.target().as_ref() != Some(&target) {
            return Err(Error::StaleGeneration);
        }
        self.submit(RequestKind::Input { target, bytes })
    }
    pub fn action(&self, action: Action) -> Result<u64> {
        match &action {
            Action::SelectSession { id } => {
                crate::command::target(id, '$')?;
            }
            Action::SelectPane { id } => {
                crate::command::target(id, '%')?;
            }
            Action::Manage { operation } => {
                Command::new(operation)?;
            }
            Action::Resize { columns, rows } => {
                Size::new(*columns, *rows)?;
            }
            _ => {}
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
struct Control {
    stream: Stream,
    parser: Parser,
    events: VecDeque<Event>,
    shared: Arc<Shared>,
    size: Size,
    live: bool,
    dirty: bool,
    paused: bool,
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
                if self.live && self.shared.target().is_some_and(|t| t.pane_id == pane) {
                    self.shared
                        .emit(OutputKind::Live, self.size, &bytes)
                        .await?;
                }
            }
            Event::Notification { name, .. } => {
                if name == "%exit" {
                    return Err(Error::MissingSession);
                }
                if name == "%pause" {
                    self.paused = true;
                    self.live = false;
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
        let command = Command::new(&op)?;
        self.stream
            .data(format!("{}\n", command.wire).as_bytes())
            .await
            .map_err(|_| Error::Network)?;
        self.response().await
    }
    async fn refresh(&mut self) -> Result<()> {
        let sessions = model::sessions(&self.command(Operation::Sessions).await?)?;
        let windows = model::windows(&self.command(Operation::Windows).await?)?;
        let mut panes = model::panes(&self.command(Operation::Panes).await?)?;
        let mut seen = HashSet::new();
        panes.retain(|p| seen.insert(p.id.clone()));
        self.shared.update(|s| {
            s.remote = RemoteState {
                sessions,
                windows,
                panes,
            };
            if s.session
                .as_ref()
                .is_some_and(|id| !s.remote.sessions.iter().any(|v| &v.id == id))
            {
                s.phase = Phase::MissingSession;
                s.pane = None;
            } else if s
                .pane
                .as_ref()
                .is_some_and(|id| !s.remote.panes.iter().any(|v| &v.id == id))
            {
                s.phase = Phase::Selecting;
                s.pane = None;
                s.generation.pane = s.generation.pane.saturating_add(1);
            }
        });
        self.dirty = false;
        Ok(())
    }
    async fn select_pane(&mut self, id: String) -> Result<()> {
        let state = self.shared.state.lock().unwrap().clone();
        let p = state
            .remote
            .panes
            .iter()
            .find(|p| p.id == id)
            .ok_or(Error::InvalidArgument)?;
        if !state
            .remote
            .windows
            .iter()
            .any(|w| w.id == p.window && Some(&w.session) == state.session.as_ref())
        {
            return Err(Error::InvalidArgument);
        }
        self.live = false;
        self.shared.update(|s| {
            s.phase = Phase::Restoring;
            s.pane = Some(id.clone());
            s.generation.pane = s
                .generation
                .pane
                .checked_add(1)
                .expect("generation exhausted");
        });
        self.command(Operation::SelectWindow {
            target: p.window.clone(),
        })
        .await?;
        self.command(Operation::SelectPane { target: id }).await?;
        self.snapshot().await
    }
    async fn snapshot(&mut self) -> Result<()> {
        let Some(t) = self.shared.target() else {
            return Ok(());
        };
        self.live = false;
        self.shared.phase(Phase::Restoring);
        let id = t.pane_id;
        let commands = [
            Operation::Pause { target: id.clone() },
            Operation::Modes { target: id.clone() },
            Operation::CaptureMain { target: id.clone() },
            Operation::Capture { target: id.clone() },
            Operation::Pending { target: id.clone() },
            Operation::Resume { target: id },
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
        self.size = Size::new(fields[15], fields[16]).map_err(|_| Error::Protocol)?;
        self.shared.update(|s| {
            s.generation.size = s.generation.size.checked_add(1).expect("size exhausted")
        });
        self.shared.emit(OutputKind::Resize, self.size, &[]).await?;
        self.shared
            .emit(OutputKind::ResetBegin, self.size, b"\x1bc")
            .await?;
        let main = self.response().await?;
        if fields[2] == 1 {
            self.draw(main).await?;
            self.shared
                .emit(OutputKind::ResetChunk, self.size, b"\x1b[?1049h")
                .await?;
        }
        let screen = self.response().await?;
        self.draw(screen).await?;
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
        self.shared
            .emit(OutputKind::ResetChunk, self.size, modes.as_bytes())
            .await?;
        for row in self.response().await? {
            self.shared
                .emit(
                    OutputKind::ResetChunk,
                    self.size,
                    &crate::protocol::decode(&row, true)?,
                )
                .await?;
        }
        self.response().await?;
        self.shared
            .emit(OutputKind::ResetEnd, self.size, &[])
            .await?;
        self.live = true;
        self.paused = false;
        self.shared.phase(Phase::Ready);
        Ok(())
    }
    async fn draw(&self, rows: Vec<Vec<u8>>) -> Result<()> {
        for (i, row) in rows.into_iter().enumerate() {
            self.shared
                .emit(
                    OutputKind::ResetChunk,
                    self.size,
                    format!("\x1b[{};1H", i + 1).as_bytes(),
                )
                .await?;
            self.shared
                .emit(
                    OutputKind::ResetChunk,
                    self.size,
                    &crate::protocol::decode(&row, true)?,
                )
                .await?;
        }
        Ok(())
    }
}
async fn list(ssh: &Ssh) -> Result<RemoteState> {
    let (code, bytes) = ssh
        .execute_tmux(&Command::new(&Operation::Sessions)?)
        .await?;
    if code != 0 {
        return Ok(RemoteState::default());
    }
    Ok(RemoteState {
        sessions: model::sessions(
            &bytes
                .split(|b| *b == b'\n')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_vec())
                .collect::<Vec<_>>(),
        )?,
        ..Default::default()
    })
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
        s.error = None;
        s.session = Some(id.clone());
        s.pane = None;
    });
    let mut c = Control {
        stream,
        parser: Parser::default(),
        events: VecDeque::new(),
        shared,
        size,
        live: false,
        dirty: false,
        paused: false,
    };
    c.response().await?;
    c.command(Operation::FlowControl).await?;
    c.command(Operation::ClientSize {
        columns: size.columns,
        rows: size.rows,
    })
    .await?;
    c.refresh().await?;
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
async fn run(
    shared: Arc<Shared>,
    host: HostProfile,
    credential: Credential,
    trusted: Option<String>,
    mut rx: mpsc::Receiver<Request>,
) {
    let mut attempt = 0;
    let mut size = Size::default();
    let mut remembered = host.last_pane.clone();
    loop {
        let observed = Arc::new(Mutex::new(None));
        shared.phase(if attempt == 0 {
            Phase::Connecting
        } else {
            Phase::Reconnecting
        });
        let result = async {
            let mut ssh =
                ssh::connect(&host, &credential, trusted.clone(), observed.clone()).await?;
            shared.phase(Phase::Authenticating);
            let tmux = ssh.discover().await?;
            shared.update(|s| {
                s.error = None;
                s.diagnostics = ssh.diagnostics.clone();
            });
            let result = connected(
                &ssh,
                shared.clone(),
                &mut rx,
                tmux,
                &mut size,
                &mut remembered,
            )
            .await;
            ssh.close().await;
            result
        }
        .await;
        let error = result.err().unwrap_or(Error::Closed);
        while let Ok(r) = rx.try_recv() {
            shared.complete(r, Some(Error::Cancelled));
        }
        shared.update(|s| {
            s.error = Some(error);
            s.fingerprint = observed.lock().unwrap().clone();
            s.generation.connection = s
                .generation
                .connection
                .checked_add(1)
                .expect("connection generation exhausted");
            s.pane = None;
        });
        if error == Error::Network && attempt < 7 {
            attempt += 1;
            shared.phase(Phase::Reconnecting);
            tokio::time::sleep(Duration::from_secs((1u64 << attempt.min(5)).min(30))).await;
            continue;
        }
        shared.phase(match error {
            Error::HostKeyUnknown => Phase::TrustRequired,
            Error::MissingSession => Phase::MissingSession,
            _ => Phase::Failed,
        });
        // Keep failure/trust visible until the UI explicitly closes this opaque handle.
        shared.stop.notified().await;
        return;
    }
}
async fn missing_session(ssh: &Ssh, shared: &Shared, control: &mut Option<Control>) -> Result<()> {
    if let Some(c) = control.take() {
        let _ = c.stream.close().await;
    }
    let remote = list(ssh).await?;
    shared.update(|s| {
        s.remote = remote;
        s.phase = Phase::MissingSession;
        s.error = Some(Error::MissingSession);
        s.pane = None;
        s.generation.pane = s.generation.pane.saturating_add(1);
    });
    Ok(())
}
async fn connected(
    ssh: &Ssh,
    shared: Arc<Shared>,
    rx: &mut mpsc::Receiver<Request>,
    tmux: bool,
    size: &mut Size,
    remembered: &mut Option<String>,
) -> Result<()> {
    let mut control = None;
    let mut shell = None;
    if tmux {
        let remote = list(ssh).await?;
        let session = shared.state.lock().unwrap().session.clone();
        shared.update(|s| s.remote = remote.clone());
        if let Some(id) = session {
            if remote.sessions.iter().any(|s| s.id == id) {
                control = Some(attach(ssh, shared.clone(), id, *size, remembered.clone()).await?);
            } else {
                shared.phase(Phase::MissingSession)
            }
        } else {
            shared.phase(Phase::Selecting)
        }
    } else {
        shell = Some(ssh.shell(*size).await?);
        shared.update(|s| {
            s.phase = Phase::Shell;
            s.pane = Some("shell".into());
            s.generation.pane += 1;
        });
        shared.emit(OutputKind::Resize, *size, &[]).await?;
        shared.emit(OutputKind::ResetBegin, *size, b"\x1bc").await?;
        shared.emit(OutputKind::ResetEnd, *size, &[]).await?;
    }
    loop {
        enum Incoming {
            Request(Option<Request>),
            Event(Result<Event>),
            Shell(Option<russh::ChannelMsg>),
        }
        let incoming = if let Some(c) = control.as_mut() {
            tokio::select! {r=rx.recv()=>Incoming::Request(r),e=c.next()=>Incoming::Event(e)}
        } else if let Some(sh) = shell.as_mut() {
            tokio::select! {r=rx.recv()=>Incoming::Request(r),m=sh.wait()=>Incoming::Shell(m)}
        } else {
            Incoming::Request(rx.recv().await)
        };
        match incoming {
            Incoming::Event(e) => {
                let result = async {
                    let c = control.as_mut().unwrap();
                    c.unsolicited(e?).await?;
                    if c.dirty {
                        let old = c.shared.state.lock().unwrap().remote.clone();
                        c.refresh().await?;
                        let now = c.shared.state.lock().unwrap().clone();
                        if now.remote != old && now.pane.is_some() {
                            c.snapshot().await?;
                        }
                    }
                    if c.paused {
                        c.snapshot().await?;
                    }
                    Ok(())
                }
                .await;
                if result == Err(Error::MissingSession) {
                    missing_session(ssh, &shared, &mut control).await?;
                } else {
                    result?;
                }
            }
            Incoming::Shell(Some(russh::ChannelMsg::Data { data })) => {
                shared.emit(OutputKind::Live, *size, &data).await?
            }
            Incoming::Shell(None)
            | Incoming::Shell(Some(
                russh::ChannelMsg::Close | russh::ChannelMsg::ExitStatus { .. },
            )) => return Err(Error::Network),
            Incoming::Shell(_) => {}
            Incoming::Request(None) => return Ok(()),
            Incoming::Request(Some(r)) => {
                if shared.cancelled.lock().unwrap().contains(&r.id) {
                    shared.complete(r, Some(Error::Cancelled));
                    continue;
                }
                let result = async {
                    match &r.kind {
                        RequestKind::Input { target, bytes } => {
                            if shared.target().as_ref() != Some(target) {
                                return Err(Error::StaleGeneration);
                            }
                            if !matches!(
                                shared.state.lock().unwrap().phase,
                                Phase::Ready | Phase::Shell
                            ) {
                                return Err(Error::NotReady);
                            }
                            if let Some(sh) = &shell {
                                sh.data(bytes.as_slice())
                                    .await
                                    .map_err(|_| Error::Network)?;
                            } else if let Some(c) = control.as_mut() {
                                for chunk in bytes.chunks(256) {
                                    if shared.cancelled.lock().unwrap().contains(&r.id) {
                                        return Err(Error::Cancelled);
                                    }
                                    c.command(Operation::Send {
                                        target: target.pane_id.clone(),
                                        bytes: chunk.to_vec(),
                                    })
                                    .await?;
                                }
                            } else {
                                return Err(Error::NotReady);
                            }
                        }
                        RequestKind::Action(a) => match a {
                            Action::SelectSession { id } => {
                                if !tmux {
                                    return Err(Error::NotReady);
                                }
                                if let Some(c) = control.take() {
                                    let _ = c.stream.close().await;
                                }
                                control = Some(
                                    attach(ssh, shared.clone(), id.clone(), *size, None).await?,
                                );
                            }
                            Action::SelectPane { id } => {
                                control
                                    .as_mut()
                                    .ok_or(Error::NotReady)?
                                    .select_pane(id.clone())
                                    .await?;
                                *remembered = Some(id.clone());
                            }
                            Action::Manage { operation } => {
                                if let Some(c) = control.as_mut() {
                                    c.command(operation.clone()).await?;
                                    c.refresh().await?;
                                    if shared.target().is_some() {
                                        c.snapshot().await?;
                                    }
                                } else if tmux {
                                    if !matches!(operation, Operation::NewSession { .. }) {
                                        return Err(Error::NotReady);
                                    }
                                    let (code, _) =
                                        ssh.execute_tmux(&Command::new(operation)?).await?;
                                    if code != 0 {
                                        return Err(Error::InvalidArgument);
                                    }
                                    let remote = list(ssh).await?;
                                    shared.update(|s| s.remote = remote);
                                } else {
                                    return Err(Error::NotReady);
                                }
                            }
                            Action::Refresh => {
                                if let Some(c) = control.as_mut() {
                                    c.refresh().await?;
                                } else if tmux {
                                    let remote = list(ssh).await?;
                                    shared.update(|s| s.remote = remote);
                                }
                            }
                            Action::Snapshot => {
                                if let Some(c) = control.as_mut() {
                                    c.snapshot().await?;
                                }
                            }
                            Action::Resize { columns, rows } => {
                                *size = Size::new(*columns, *rows)?;
                                shared.update(|s| s.generation.size += 1);
                                if let Some(sh) = &shell {
                                    sh.window_change((*columns).into(), (*rows).into(), 0, 0)
                                        .await
                                        .map_err(|_| Error::Network)?;
                                    shared.emit(OutputKind::Resize, *size, &[]).await?;
                                }
                                if let Some(c) = control.as_mut() {
                                    c.command(Operation::ClientSize {
                                        columns: *columns,
                                        rows: *rows,
                                    })
                                    .await?;
                                    c.snapshot().await?;
                                }
                            }
                        },
                    }
                    Ok(())
                }
                .await;
                let error = result.err();
                shared.complete(r, error);
                if error == Some(Error::MissingSession) {
                    missing_session(ssh, &shared, &mut control).await?;
                } else if matches!(error, Some(Error::Network | Error::Protocol)) {
                    return Err(error.unwrap());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
