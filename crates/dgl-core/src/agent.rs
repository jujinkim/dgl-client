//! Observed facts, never inferred task success or persistent approval/input wait.
use crate::{Error, Result, command, ssh::Ssh};
use crate::{i18n::t, tr};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

pub const HELPER: &str = include_str!("../agent/observer.py");
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Agent {
    Codex,
    Claude,
    Agy,
    Grok,
}
impl Agent {
    pub const ALL: [Self; 4] = [Self::Codex, Self::Claude, Self::Agy, Self::Grok];
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Agy => "AGY",
            Self::Grok => "Grok Build",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Codex => "launcher/openai.svg",
            Self::Claude => "launcher/claude.svg",
            Self::Agy => "launcher/agy.svg",
            Self::Grok => "launcher/grok.svg",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Activity,
    Question,
    Approval,
    Response,
    Error,
    Ended,
}
impl Kind {
    pub fn priority(self) -> u8 {
        match self {
            Self::Question | Self::Approval => 0,
            Self::Error => 1,
            Self::Response => 2,
            Self::Activity => 3,
            Self::Ended => 4,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Question => t("질문이 발생했습니다"),
            Self::Approval => t("승인 요청이 발생했습니다"),
            Self::Response => t("응답 종료 신호를 받았습니다"),
            Self::Error => t("명시적 오류를 받았습니다"),
            Self::Activity => t("작업 사건을 받았습니다"),
            Self::Ended => t("실행 종료/중단 신호를 받았습니다"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: String,
    pub seq: u64,
    pub server: String,
    pub run: String,
    pub actor: String,
    pub session: String,
    pub window: String,
    pub pane: String,
    pub agent: Agent,
    pub kind: Kind,
    pub event: String,
    pub at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub session: String,
    pub window: String,
    pub pane: String,
    pub run: String,
    pub actor: String,
    pub agent: Agent,
    pub last: Option<AgentEvent>,
    pub supported: Vec<Kind>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub server: String,
    pub epoch: String,
    pub cursor: u64,
    pub gap: bool,
    pub observations: Vec<Observation>,
    pub events: Vec<AgentEvent>,
    pub valid: bool,
    pub observed_at: u64,
}
impl Snapshot {
    pub fn discard_observations_through(&mut self, sequence: u64) {
        if sequence == 0 {
            return;
        }
        for observation in &mut self.observations {
            if observation.last.as_ref().is_some_and(|e| e.seq <= sequence) {
                observation.last = None;
                observation.supported.clear();
            }
        }
        self.events.retain(|e| e.seq > sequence);
    }
    pub fn invalidate(&mut self) {
        self.valid = false;
        self.events.clear();
    }
    pub fn current(&self, now: u64) -> bool {
        self.valid && now.saturating_sub(self.observed_at) < 10_000 && !self.gap
    }
    /// Select by event data, independently of translated display text.
    pub fn display_kind(&self, pane: Option<&str>, now: u64) -> Option<Kind> {
        if !self.current(now) {
            return None;
        }
        self.observations
            .iter()
            .filter(|o| pane.is_none_or(|p| p == o.pane))
            .filter_map(|o| o.last.as_ref())
            .filter(|e| e.kind != Kind::Ended)
            .min_by_key(|e| e.kind.priority())
            .map(|e| e.kind)
    }
    pub fn label(&self, pane: Option<&str>, now: u64) -> String {
        let observations: Vec<_> = self
            .observations
            .iter()
            .filter(|o| pane.is_none_or(|p| p == o.pane))
            .collect();
        let agents = Agent::ALL
            .into_iter()
            .filter(|a| observations.iter().any(|o| o.agent == *a))
            .map(|a| a.label())
            .collect::<Vec<_>>()
            .join(" · ");
        if observations.is_empty() {
            return t("터미널 · 상태 미확인").into();
        }
        let last = observations
            .iter()
            .filter_map(|o| o.last.as_ref())
            .filter(|e| e.kind != Kind::Ended)
            .min_by_key(|e| e.kind.priority());
        if !self.current(now) || last.is_none() {
            return tr!("{0} · 상태 미확인", agents);
        }
        let last = last.unwrap();
        let count = observations
            .iter()
            .filter_map(|o| o.last.as_ref())
            .filter(|e| e.kind.priority() == last.kind.priority())
            .count();
        format!("{agents} · {} {count}", last.kind.label())
    }
    pub fn validate(&self) -> Result<()> {
        if self.observations.len() > 2048 || self.events.len() > 2048 {
            return Err(Error::Protocol);
        }
        for o in &self.observations {
            command::target(&o.session, '$')?;
            command::target(&o.window, '@')?;
            command::target(&o.pane, '%')?;
            if o.run.len() > 128 || o.actor.len() > 128 {
                return Err(Error::Protocol);
            }
        }
        for e in self
            .events
            .iter()
            .chain(self.observations.iter().filter_map(|o| o.last.as_ref()))
        {
            command::target(&e.session, '$')?;
            command::target(&e.window, '@')?;
            command::target(&e.pane, '%')?;
            if e.id.len() > 128 || e.event.len() > 80 || e.server != self.server {
                return Err(Error::Protocol);
            }
        }
        Ok(())
    }
}

// Only trusted bundled source is passed to this larger shell quote. All dynamic arguments
// continue through command::quote/target's strict size and control-character validation.
fn source_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
pub fn remote_command(tmux: &str, args: &[&str]) -> Result<String> {
    let source = serde_json::to_string(HELPER).unwrap();
    // JSON string literals use true unicode and Python accepts the escapes used here.
    let script = format!(
        "s={source};g={{'__name__':'dgl_observer'}};exec(compile(s,'dgl-observer','exec'),g);g['main'](s)"
    );
    let args = args
        .iter()
        .map(|v| command::quote(v))
        .collect::<Result<Vec<_>>>()?
        .join(" ");
    let inner = format!(
        "exec python3 -c {} {args} --tmux {}",
        source_quote(&script),
        command::quote(tmux)?
    );
    // Login PATH is necessary for per-user CLI/Python installations. Mark output to ignore banners.
    Ok(format!(
        "exec \"${{SHELL:-/bin/sh}}\" -lc {}",
        source_quote(&inner)
    ))
}
/// Runs a setup action (`inspect`, `install`, `remove`). A failing helper still prints
/// its `{"error": code}` object, so the exit status adds nothing.
pub async fn setup(ssh: &Ssh, args: &[&str]) -> Result<serde_json::Value> {
    let cmd = remote_command(ssh.tmux.as_deref().ok_or(Error::NotReady)?, args)?;
    let (_, out) = ssh.execute(&cmd).await?;
    let line = out
        .split(|b| *b == b'\n')
        .rev()
        .find(|l| l.starts_with(b"{"))
        .ok_or(Error::Protocol)?;
    let value = serde_json::from_slice(line).map_err(|_| Error::Protocol)?;
    Ok(localize_setup(value))
}
fn limitations(agent: &str) -> &'static str {
    match agent {
        "Codex" => {
            t("질문·일반 오류는 수집하지 않습니다. /hooks 신뢰 검토와 실제 수신은 별도입니다.")
        }
        "Agy" => t(
            "승인은 ask_permission만 수집하며 일반 권한 팝업은 감지하지 않습니다. 새 실행의 SessionStart가 필요합니다.",
        ),
        "Grok" => t("idle_prompt와 task_complete는 응답 종료 알림으로 수집하지 않습니다."),
        _ => "",
    }
}
/// The helper reports codes; the wording and its translations stay in the app.
fn localize_setup(mut value: serde_json::Value) -> serde_json::Value {
    if value.get("error").is_some() {
        value["error"] = t("연동 조회/설정 실패: Python·tmux·CLI 버전·설정 형식 또는 외부 변경을 확인한 뒤 다시 검사하세요.").into();
    }
    if let Some(configs) = value.get_mut("configs").and_then(|v| v.as_array_mut()) {
        for c in configs.iter_mut().filter(|c| c.is_object()) {
            let limitations = limitations(c["agent"].as_str().unwrap_or(""));
            c["limitations"] = limitations.into();
            if c["error"].as_str().is_some_and(|e| !e.is_empty()) {
                c["error"] = t("설정 형식·경로 또는 CLI 버전 조회 실패 · 설치 제외").into();
            }
        }
    }
    value
}

/// The helper started once per connection; each poll is one request line and one reply.
pub struct Observer {
    stream: crate::ssh::Stream,
    buffer: Vec<u8>,
}
impl Observer {
    pub async fn start(ssh: &Ssh) -> Result<Self> {
        let cmd = remote_command(ssh.tmux.as_deref().ok_or(Error::NotReady)?, &["serve"])?;
        Ok(Self {
            stream: ssh.process(&cmd).await?,
            buffer: Vec::new(),
        })
    }
    /// A helper that is missing or exits ends its stream: `NotReady`.
    pub async fn poll(
        &mut self,
        session: &str,
        after: u64,
        epoch: &str,
    ) -> Result<serde_json::Value> {
        command::target(session, '$')?;
        let request = serde_json::json!({"session": session, "after": after, "epoch": epoch});
        self.stream
            .data(format!("{request}\n").as_bytes())
            .await
            .map_err(|_| Error::NotReady)?;
        loop {
            while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                // Replies start with RS; shell start-up output, even without its own
                // line break, is skipped.
                if let Some(at) = line.iter().position(|b| *b == 0x1e) {
                    return serde_json::from_slice(&line[at + 1..]).map_err(|_| Error::Protocol);
                }
            }
            self.read().await?;
        }
    }
    /// Reads until the helper goes away. The SSH session holds one message per channel,
    /// so a stream left unread between polls would stop every other channel.
    pub async fn drain(&mut self) -> Error {
        loop {
            if let Err(error) = self.read().await {
                return error;
            }
        }
    }
    async fn read(&mut self) -> Result<()> {
        match self.stream.wait().await {
            Some(russh::ChannelMsg::Data { data }) => {
                if self.buffer.len() + data.len() > 4 * 1024 * 1024 {
                    return Err(Error::Protocol);
                }
                self.buffer.extend_from_slice(&data);
                Ok(())
            }
            Some(russh::ChannelMsg::Eof | russh::ChannelMsg::Close)
            | Some(russh::ChannelMsg::ExitStatus { .. })
            | None => Err(Error::NotReady),
            Some(_) => Ok(()),
        }
    }
    /// Bounded: the session may itself be waiting for this channel to be read.
    pub async fn close(self) {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), self.stream.close()).await;
    }
}

/// Python is optional for basic process identification. No screen, spinner, CPU or
/// generic `node` heuristics are used by this conservative POSIX fallback.
pub async fn basic_snapshot(ssh: &Ssh, session: &str) -> Result<Snapshot> {
    let tmux = command::quote(ssh.tmux.as_deref().ok_or(Error::NotReady)?)?;
    let (code, panes) = ssh
        .execute(&format!(
            "{tmux} list-panes -a -F '#{{session_id}}|#{{window_id}}|#{{pane_id}}|#{{pane_pid}}'"
        ))
        .await?;
    if code != 0 {
        return Err(Error::NotReady);
    }
    let (code, ps) = ssh
        .execute("LC_ALL=C ps -ww -axo pid=,ppid=,lstart=,comm=")
        .await?;
    if code != 0 {
        return Err(Error::NotReady);
    }
    let procs: HashMap<u32, (u32, String, Option<Agent>)> = String::from_utf8_lossy(&ps)
        .lines()
        .filter_map(|line| {
            let p: Vec<_> = line.split_whitespace().collect();
            if p.len() != 8 {
                return None;
            }
            let agent = match p[7].rsplit('/').next()? {
                "codex" => Some(Agent::Codex),
                "claude" => Some(Agent::Claude),
                "agy" => Some(Agent::Agy),
                "grok" => Some(Agent::Grok),
                _ => None,
            };
            Some((
                p[0].parse().ok()?,
                (p[1].parse().ok()?, p[2..7].join("-"), agent),
            ))
        })
        .collect();
    let mut snapshot = Snapshot::default();
    for line in String::from_utf8_lossy(&panes).lines() {
        let p: Vec<_> = line.split('|').collect();
        if p.len() != 4 || p[0] != session {
            continue;
        }
        let root: u32 = p[3].parse().map_err(|_| Error::Protocol)?;
        for (pid, (_, start, agent)) in &procs {
            let Some(agent) = agent else {
                continue;
            };
            let mut at = *pid;
            let mut seen = HashSet::new();
            while let Some((parent, _, _)) = procs.get(&at) {
                if at == root {
                    snapshot.observations.push(Observation {
                        session: p[0].into(),
                        window: p[1].into(),
                        pane: p[2].into(),
                        run: format!("basic:{pid}:{start}"),
                        actor: "parent".into(),
                        agent: *agent,
                        last: None,
                        supported: vec![],
                    });
                    break;
                }
                if !seen.insert(at) {
                    break;
                }
                at = *parent;
            }
        }
    }
    snapshot.validate()?;
    Ok(snapshot)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotificationSettings {
    pub questions: bool,
    pub responses: bool,
    pub errors: bool,
    pub os: bool,
}
impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            questions: true,
            responses: true,
            errors: true,
            os: false,
        }
    }
}
impl NotificationSettings {
    pub fn allows(&self, kind: Kind) -> bool {
        match kind {
            Kind::Question | Kind::Approval => self.questions,
            Kind::Response => self.responses,
            Kind::Error => self.errors,
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub endpoint: String,
    pub event: AgentEvent,
    pub read: bool,
}
impl Notice {
    pub fn key(&self) -> String {
        pane_key(&self.endpoint, &self.event.server, &self.event.pane)
    }
    pub fn id(&self) -> String {
        format!("{}:{}", self.endpoint, self.event.id)
    }
}
/// Identifies a visible pane for `Inbox::tick`; notices for it are read and never delivered.
pub fn pane_key(endpoint: &str, server: &str, pane: &str) -> String {
    format!("{endpoint}:{server}:{pane}")
}
fn run_key(endpoint: &str, server: &str, run: &str) -> String {
    format!("{endpoint}:{server}:{run}")
}
#[derive(Default)]
pub struct Inbox {
    pub recent: VecDeque<Notice>,
    seen: HashSet<String>,
    order: VecDeque<String>,
    pending: HashMap<String, (u64, Notice)>,
}
impl Inbox {
    /// One notification pass shared by every platform. `sources` are the endpoints whose
    /// connection is observing agents (`State::observing_agents`) with that snapshot; a
    /// coalesced notice is delivered only while its run is still observed and current.
    pub fn tick<'a>(
        &mut self,
        sources: impl IntoIterator<Item = (&'a str, &'a Snapshot)>,
        visible: &HashSet<String>,
        settings: &NotificationSettings,
        now: u64,
    ) -> Vec<Notice> {
        let mut live = HashSet::new();
        for (endpoint, snapshot) in sources {
            if snapshot.current(now) {
                live.extend(
                    snapshot
                        .observations
                        .iter()
                        .map(|o| run_key(endpoint, &snapshot.server, &o.run)),
                );
            }
            self.ingest(endpoint, snapshot, settings, now);
        }
        self.read_visible(visible);
        self.ready(now, visible, &live, settings)
    }
    pub fn ingest(
        &mut self,
        endpoint: &str,
        snapshot: &Snapshot,
        settings: &NotificationSettings,
        now: u64,
    ) {
        if !snapshot.current(now) {
            return;
        }
        for event in &snapshot.events {
            let notice = Notice {
                endpoint: endpoint.into(),
                event: event.clone(),
                read: false,
            };
            let id = notice.id();
            if !self.seen.insert(id.clone()) {
                continue;
            }
            self.order.push_back(id);
            while self.order.len() > 8192 {
                if let Some(id) = self.order.pop_front() {
                    self.seen.remove(&id);
                }
            }
            if !settings.allows(event.kind) {
                continue;
            }
            // Coalesce only the same server + pane within a bounded window. Never use
            // remote wall-clock time for local notification deadlines.
            let key = notice.key();
            let due = self.pending.get(&key).map(|p| p.0).unwrap_or(now + 2000);
            self.pending.insert(key, (due, notice.clone()));
            self.recent.push_front(notice);
            self.recent.truncate(100);
        }
    }
    pub fn read_visible(&mut self, visible: &HashSet<String>) {
        for n in &mut self.recent {
            if visible.contains(&n.key()) {
                n.read = true;
            }
        }
        for (_, n) in self.pending.values_mut() {
            if visible.contains(&n.key()) {
                n.read = true;
            }
        }
    }
    pub fn mark_read(&mut self, id: &str) {
        for (_, n) in self.pending.values_mut() {
            if n.id() == id {
                n.read = true;
            }
        }
        for n in &mut self.recent {
            if n.id() == id {
                n.read = true;
            }
        }
    }
    pub fn ready(
        &mut self,
        now: u64,
        visible: &HashSet<String>,
        live: &HashSet<String>,
        settings: &NotificationSettings,
    ) -> Vec<Notice> {
        let keys: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, (due, _))| *due <= now)
            .map(|(k, _)| k.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| {
                let (_, n) = self.pending.remove(&key)?;
                (settings.os
                    && !n.read
                    && settings.allows(n.event.kind)
                    && !visible.contains(&key)
                    && live.contains(&run_key(&n.endpoint, &n.event.server, &n.event.run)))
                .then_some(n)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(id: &str, kind: Kind) -> AgentEvent {
        AgentEvent {
            id: id.into(),
            seq: 1,
            server: "server".into(),
            run: "run".into(),
            actor: "parent".into(),
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            agent: Agent::Codex,
            kind,
            event: "Stop".into(),
            at: 1,
        }
    }
    #[test]
    fn observations_are_independent_of_read_and_never_mean_success() {
        let e = event("a", Kind::Response);
        let mut snapshot = Snapshot {
            valid: true,
            observed_at: 100,
            server: "server".into(),
            observations: vec![Observation {
                session: "$1".into(),
                window: "@1".into(),
                pane: "%1".into(),
                run: "run".into(),
                actor: "parent".into(),
                agent: Agent::Codex,
                last: Some(e.clone()),
                supported: vec![Kind::Response],
            }],
            events: vec![e],
            ..Default::default()
        };
        let settings = NotificationSettings {
            os: true,
            ..Default::default()
        };
        let mut inbox = Inbox::default();
        inbox.ingest("host", &snapshot, &settings, 100);
        inbox.ingest("host", &snapshot, &settings, 101);
        assert_eq!(inbox.recent.len(), 1);
        let visible = HashSet::from(["host:server:%1".into()]);
        inbox.read_visible(&visible);
        assert!(inbox.recent[0].read);
        assert!(snapshot.label(None, 100).contains("응답 종료 신호"));
        assert!(
            inbox
                .ready(
                    2100,
                    &HashSet::new(), // Changing tabs after reading must not send a delayed alert.
                    &HashSet::from(["host:server:run".into()]),
                    &settings
                )
                .is_empty()
        );
        assert!(snapshot.label(None, 10100).contains("상태 미확인"));
        assert_eq!(snapshot.display_kind(None, 100), Some(Kind::Response));
        assert_eq!(snapshot.display_kind(None, 10100), None);
        assert_eq!(snapshot.display_kind(Some("%2"), 100), None);
        for kind in [
            Kind::Question,
            Kind::Approval,
            Kind::Error,
            Kind::Response,
            Kind::Activity,
            Kind::Ended,
        ] {
            snapshot.observations[0].last.as_mut().unwrap().kind = kind;
            assert_eq!(
                snapshot.display_kind(None, 100),
                (kind != Kind::Ended).then_some(kind)
            );
        }
        snapshot.gap = true;
        assert_eq!(snapshot.display_kind(None, 100), None);
        snapshot.gap = false;

        // Missing events must not revive an earlier observation on the next successful poll.
        snapshot.discard_observations_through(1);
        assert!(snapshot.label(None, 100).contains("상태 미확인"));
        let mut fresh = event("after-gap", Kind::Activity);
        fresh.seq = 2;
        snapshot.observations[0].last = Some(fresh);
        snapshot.discard_observations_through(1);
        assert!(snapshot.label(None, 100).contains("작업 사건"));
        snapshot.invalidate();
        assert!(snapshot.events.is_empty());
    }
    #[test]
    fn coalesces_cross_tab_events_filters_dead_runs_and_bounds_history() {
        let settings = NotificationSettings {
            os: true,
            ..Default::default()
        };
        let mut s = Snapshot {
            valid: true,
            observed_at: 1,
            ..Default::default()
        };
        let mut inbox = Inbox::default();
        for i in 0..110 {
            s.events = vec![event(&i.to_string(), Kind::Approval)];
            inbox.ingest("host", &s, &settings, 1 + i);
        }
        assert_eq!(inbox.recent.len(), 100);
        assert!(
            inbox
                .ready(2000, &HashSet::new(), &HashSet::new(), &settings)
                .is_empty()
        );
        let ready = inbox.ready(
            2001,
            &HashSet::new(),
            &HashSet::from(["host:server:run".into()]),
            &settings,
        );
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].event.id, "109");
        s.events = vec![event("dead", Kind::Error)];
        inbox.ingest("host", &s, &settings, 3000);
        assert!(
            inbox
                .ready(6000, &HashSet::new(), &HashSet::new(), &settings)
                .is_empty()
        );
    }
    #[test]
    fn setup_codes_become_app_copy() {
        let value = localize_setup(serde_json::json!({
            "token": "t",
            "configs": [
                {"agent": "Codex", "error": "config"},
                {"agent": "Claude", "error": ""},
            ],
        }));
        assert_eq!(value["configs"][0]["limitations"], limitations("Codex"));
        assert!(!limitations("Codex").is_empty());
        assert_ne!(value["configs"][0]["error"], "config");
        assert_eq!(value["configs"][1]["limitations"], "");
        assert_eq!(value["configs"][1]["error"], "");
        assert!(value.get("error").is_none());
        let failed = localize_setup(serde_json::json!({"error": "failed"}));
        assert_ne!(failed["error"], "failed");
        // Remote output: an odd entry is left alone rather than ending the connection.
        let odd = localize_setup(serde_json::json!({"configs": [1, null, "x"]}));
        assert_eq!(odd["configs"], serde_json::json!([1, null, "x"]));
    }
    #[test]
    fn tick_delivers_only_observed_runs_and_reads_visible_panes() {
        let settings = NotificationSettings {
            os: true,
            ..Default::default()
        };
        let observation = Observation {
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            run: "run".into(),
            actor: "parent".into(),
            agent: Agent::Codex,
            last: None,
            supported: vec![],
        };
        let snapshot = |id: &str| Snapshot {
            server: "server".into(),
            valid: true,
            observed_at: 1,
            observations: vec![observation.clone()],
            events: vec![event(id, Kind::Approval)],
            ..Default::default()
        };
        let none = HashSet::new();
        let mut inbox = Inbox::default();
        let first = snapshot("observed");
        assert!(
            inbox
                .tick([("host", &first)], &none, &settings, 1)
                .is_empty()
        );
        let delivered = inbox.tick([("host", &first)], &none, &settings, 2001);
        assert_eq!(delivered.len(), 1);
        // A connection that stops observing is omitted: its pending notice is dropped.
        let second = snapshot("unobserved");
        inbox.tick([("host", &second)], &none, &settings, 3000);
        assert!(inbox.tick([], &none, &settings, 5001).is_empty());
        // A visible pane reads the notice instead of delivering it.
        let third = snapshot("visible");
        let visible = HashSet::from([pane_key("host", "server", "%1")]);
        assert!(
            inbox
                .tick([("host", &third)], &visible, &settings, 6000)
                .is_empty()
        );
        assert!(
            inbox
                .recent
                .iter()
                .find(|n| n.event.id == "visible")
                .unwrap()
                .read
        );
        assert!(
            inbox
                .tick([("host", &third)], &none, &settings, 8001)
                .is_empty()
        );
    }
    #[test]
    fn all_agents_keep_wire_identity_and_share_notification_routes() {
        for (agent, wire) in Agent::ALL
            .into_iter()
            .zip(["Codex", "Claude", "Agy", "Grok"])
        {
            assert_eq!(
                serde_json::to_string(&agent).unwrap(),
                format!("\"{wire}\"")
            );
            for kind in [Kind::Question, Kind::Approval, Kind::Response, Kind::Error] {
                let mut e = event(wire, kind);
                e.agent = agent;
                let snapshot = Snapshot {
                    valid: true,
                    events: vec![e],
                    ..Default::default()
                };
                let settings = NotificationSettings {
                    os: true,
                    ..Default::default()
                };
                let mut inbox = Inbox::default();
                inbox.ingest("host", &snapshot, &settings, 1);
                inbox.ingest("host", &snapshot, &settings, 2);
                assert_eq!(inbox.recent.len(), 1);
                let deliver = inbox.ready(
                    2100,
                    &HashSet::new(),
                    &HashSet::from(["host:server:run".into()]),
                    &settings,
                );
                assert_eq!(deliver.len(), 1);
                assert_eq!(deliver[0].event.agent, agent);
                assert_eq!(deliver[0].event.kind, kind);
            }
        }
    }
}
