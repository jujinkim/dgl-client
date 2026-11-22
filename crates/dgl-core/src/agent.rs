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
    /// A CLI's own executable name, as `ps` or tmux `pane_current_command` reports it.
    pub fn from_command(command: &str) -> Option<Self> {
        match command.rsplit('/').next()? {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "agy" => Some(Self::Agy),
            "grok" => Some(Self::Grok),
            _ => None,
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
            Self::Question => t("agent.question_reported"),
            Self::Approval => t("agent.approval_request_reported"),
            Self::Response => t("agent.response_end_signal_received"),
            Self::Error => t("agent.explicit_error_reported"),
            Self::Activity => t("agent.activity_event_received"),
            Self::Ended => t("agent.run_ended_interrupted"),
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub session: String,
    pub window: String,
    pub pane: String,
    pub run: String,
    pub actor: String,
    pub agent: Agent,
    pub last: Option<AgentEvent>,
    pub supported: Vec<Kind>,
    /// Account, usage, context and model the helper collected for this run. A status that
    /// does not parse or fails `AgentStatus::is_valid` becomes `None`; the rest of the
    /// snapshot is kept.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_status"
    )]
    pub status: Option<AgentStatus>,
}
fn lenient_status<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<AgentStatus>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .and_then(|v| serde_json::from_value::<AgentStatus>(v).ok())
        .filter(AgentStatus::is_valid))
}

/// Status values the helper reported. `None` is unknown and never means zero.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentStatus {
    /// Milliseconds since the epoch of the newest contributing sample.
    pub at: Option<u64>,
    /// Collectors that contributed: `mod`, `statusline`, `hook`, `file`, `command`.
    pub sources: Vec<String>,
    pub model: Option<ModelInfo>,
    pub effort: Option<String>,
    pub thinking: Option<bool>,
    pub context: Option<ContextUsage>,
    pub cache: Option<CacheUsage>,
    pub cost_usd: Option<f64>,
    pub account: Option<AccountInfo>,
    pub limits: Vec<UsageLimit>,
    /// Milliseconds since the epoch when `limits` were sampled; `at` covers every key.
    pub limits_at: Option<u64>,
    /// `rate_limit`, `billing_error` or `authentication_failed` from a failure hook.
    pub alert: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelInfo {
    pub id: Option<String>,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextUsage {
    pub used: Option<u64>,
    pub size: Option<u64>,
    /// 0..=100; the CLI's own figure when it reports one.
    pub percent: Option<f64>,
    /// Auto-compact threshold in percent, when known.
    pub compact_percent: Option<f64>,
    /// Only from a real per-category source (the Claude Code mod).
    pub breakdown: Vec<ContextPart>,
    pub breakdown_total: Option<u64>,
    pub breakdown_window: Option<u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextPart {
    /// One of `CONTEXT_CATEGORIES`; anything else is shown as `other` with `name`.
    pub category: String,
    pub name: Option<String>,
    pub tokens: Option<u64>,
    pub kind: Option<String>,
}
/// Provider-side prompt cache of the last request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheUsage {
    /// Uncached input tokens.
    pub input: Option<u64>,
    pub read: Option<u64>,
    pub write: Option<u64>,
    pub output: Option<u64>,
    /// `5m` or `1h` (Claude only).
    pub ttl: Option<String>,
    /// Milliseconds since the epoch when that request completed.
    pub last_at: Option<u64>,
    pub hit_ratio: Option<f64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountInfo {
    /// `subscription`, `api_key`, `cloud`, `enterprise` or `other`.
    pub kind: Option<String>,
    pub plan: Option<String>,
    /// Hash of non-secret identity; only groups limit samples of one account.
    pub key: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageLimit {
    /// `five_hour`, `seven_day`, `spend_limit` or a CLI's own bucket id.
    pub id: String,
    pub label: Option<String>,
    /// May exceed 100 for an exceeded spend limit.
    pub used_percent: Option<f64>,
    pub remaining_fraction: Option<f64>,
    /// Seconds since the epoch.
    pub resets_at: Option<u64>,
}

pub const CONTEXT_CATEGORIES: [&str; 11] = [
    "system_prompt",
    "system_tools",
    "mcp_tools",
    "custom_agents",
    "memory_files",
    "skills",
    "messages",
    "autocompact_buffer",
    "free",
    "deferred",
    "other",
];
/// A status sample older than this is shown as stale.
pub const STATUS_STALE_MS: u64 = 60 * 60 * 1000;

fn text_ok(value: &str, max: usize) -> bool {
    value.chars().count() <= max && !value.chars().any(char::is_control)
}
fn optional_text(value: &Option<String>, max: usize) -> bool {
    value.as_deref().is_none_or(|v| text_ok(v, max))
}
fn in_range(value: Option<f64>, max: f64) -> bool {
    value.is_none_or(|v| v.is_finite() && (0.0..=max).contains(&v))
}
impl AgentStatus {
    /// Bounds for remote data. A failing status is dropped, never the snapshot.
    pub fn is_valid(&self) -> bool {
        let model = self
            .model
            .as_ref()
            .is_none_or(|m| optional_text(&m.id, 128) && optional_text(&m.name, 64));
        let context = self.context.as_ref().is_none_or(|c| {
            in_range(c.percent, 100.0)
                && in_range(c.compact_percent, 100.0)
                && c.breakdown.len() <= 24
                && c.breakdown.iter().all(|p| {
                    text_ok(&p.category, 32)
                        && optional_text(&p.name, 64)
                        && optional_text(&p.kind, 32)
                })
        });
        let cache = self
            .cache
            .as_ref()
            .is_none_or(|c| optional_text(&c.ttl, 16) && in_range(c.hit_ratio, 1.0));
        let account = self.account.as_ref().is_none_or(|a| {
            optional_text(&a.kind, 32) && optional_text(&a.plan, 64) && optional_text(&a.key, 128)
        });
        let limits = self.limits.len() <= 16
            && self.limits.iter().all(|l| {
                text_ok(&l.id, 128)
                    && optional_text(&l.label, 64)
                    && in_range(l.used_percent, 1000.0)
                    && in_range(l.remaining_fraction, 1.0)
            });
        self.sources.len() <= 8
            && self.sources.iter().all(|s| text_ok(s, 32))
            && optional_text(&self.effort, 32)
            && optional_text(&self.alert, 64)
            && self.cost_usd.is_none_or(|v| v.is_finite() && v >= 0.0)
            && model
            && context
            && cache
            && account
            && limits
    }
    /// The CLI's own percentage, else `used / size`. Unknown stays `None`.
    pub fn context_percent(&self) -> Option<f64> {
        let c = self.context.as_ref()?;
        c.percent.or_else(|| match (c.used, c.size) {
            (Some(used), Some(size)) if size > 0 => {
                Some((used as f64 * 100.0 / size as f64).clamp(0.0, 100.0))
            }
            _ => None,
        })
    }
    /// The reported ratio, else `read / (input + read + write)` when computable.
    pub fn cache_hit_ratio(&self) -> Option<f64> {
        let c = self.cache.as_ref()?;
        c.hit_ratio.or_else(|| {
            let (input, read) = (c.input?, c.read?);
            let total = input + read + c.write.unwrap_or(0);
            (total > 0).then(|| read as f64 / total as f64)
        })
    }
    pub fn account_key(&self) -> Option<&str> {
        self.account.as_ref()?.key.as_deref()
    }
    /// When the limits were sampled: `limits_at`, else the newest sample of any key.
    pub fn limits_sampled_at(&self) -> u64 {
        self.limits_at.or(self.at).unwrap_or(0)
    }
    pub fn is_stale(&self, now_ms: u64) -> bool {
        self.at
            .is_none_or(|at| now_ms.saturating_sub(at) >= STATUS_STALE_MS)
    }
}
impl ContextPart {
    /// The vocabulary key used for the label and colour; unknown categories are `other`.
    pub fn category_key(&self) -> &'static str {
        CONTEXT_CATEGORIES
            .into_iter()
            .find(|c| *c == self.category)
            .unwrap_or("other")
    }
    pub fn label(&self) -> String {
        let key = match self.category_key() {
            "system_prompt" => "agent_status.category.system_prompt",
            "system_tools" => "agent_status.category.system_tools",
            "mcp_tools" => "agent_status.category.mcp_tools",
            "custom_agents" => "agent_status.category.custom_agents",
            "memory_files" => "agent_status.category.memory_files",
            "skills" => "agent_status.category.skills",
            "messages" => "agent_status.category.messages",
            "autocompact_buffer" => "agent_status.category.autocompact_buffer",
            "free" => "agent_status.category.free",
            "deferred" => "agent_status.category.deferred",
            _ => {
                return self
                    .name
                    .clone()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| t("agent_status.category.other").into());
            }
        };
        t(key).into()
    }
}
impl CacheUsage {
    /// `5m`, `1h`, `30s` style TTLs in milliseconds.
    pub fn ttl_ms(&self) -> Option<u64> {
        let ttl = self.ttl.as_deref()?.trim();
        let unit = ttl.chars().last()?;
        let number: u64 = ttl[..ttl.len() - unit.len_utf8()].parse().ok()?;
        let unit = match unit {
            's' => 1_000,
            'm' => 60_000,
            'h' => 3_600_000,
            _ => return None,
        };
        number.checked_mul(unit)
    }
    /// Estimated: the provider does not report expiry, so this is `last_at + ttl`.
    pub fn estimated_expiry_ms(&self) -> Option<u64> {
        self.last_at?.checked_add(self.ttl_ms()?)
    }
}
impl AccountInfo {
    pub fn kind_label(&self) -> Option<&'static str> {
        Some(match self.kind.as_deref()? {
            "subscription" => t("agent_status.account.subscription"),
            "api_key" => t("agent_status.account.api_key"),
            "cloud" => t("agent_status.account.cloud"),
            "enterprise" => t("agent_status.account.enterprise"),
            _ => t("agent_status.account.other"),
        })
    }
}
impl UsageLimit {
    /// Used percentage, from `remaining_fraction` when only that is reported.
    pub fn used(&self) -> Option<f64> {
        self.used_percent
            .or_else(|| self.remaining_fraction.map(|f| 100.0 * (1.0 - f)))
    }
    pub fn title(&self) -> String {
        match self.id.as_str() {
            "five_hour" => t("agent_status.limit.five_hour").into(),
            "seven_day" => t("agent_status.limit.seven_day").into(),
            "spend_limit" => t("agent_status.limit.spend_limit").into(),
            id => self
                .label
                .clone()
                .filter(|l| !l.trim().is_empty())
                .unwrap_or_else(|| id.into()),
        }
    }
    /// Seconds until the reset, computed locally; `None` when unknown.
    pub fn resets_in_s(&self, now_ms: u64) -> Option<u64> {
        Some(self.resets_at?.saturating_sub(now_ms / 1000))
    }
}
pub fn alert_label(alert: &str) -> Option<&'static str> {
    Some(match alert {
        "rate_limit" => t("agent_status.alert.rate_limit"),
        "billing_error" => t("agent_status.alert.billing_error"),
        "authentication_failed" => t("agent_status.alert.authentication_failed"),
        _ => return None,
    })
}
/// Compact token count: `950`, `1.2k`, `84k`, `1.2M`, `12M`.
pub fn format_tokens(n: u64) -> String {
    fn tenths(value: u64, unit: &str) -> String {
        if value.is_multiple_of(10) {
            format!("{}{unit}", value / 10)
        } else {
            format!("{}.{}{unit}", value / 10, value % 10)
        }
    }
    if n < 1_000 {
        return n.to_string();
    }
    let k = (n + 50) / 100;
    if k < 100 {
        return tenths(k, "k");
    }
    let k = (n + 500) / 1_000;
    if k < 1_000 {
        return format!("{k}k");
    }
    let m = (n + 50_000) / 100_000;
    if m < 100 {
        return tenths(m, "M");
    }
    format!("{}M", (n + 500_000) / 1_000_000)
}
/// Compact duration: `45s`, `3m`, `2h 5m`, `4d`, `4d 3h`.
pub fn format_duration_s(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        let (h, m) = (secs / 3_600, secs % 3_600 / 60);
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m}m")
        }
    } else {
        let (d, h) = (secs / 86_400, secs % 86_400 / 3_600);
        if h == 0 {
            format!("{d}d")
        } else {
            format!("{d}d {h}h")
        }
    }
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
    /// A CLI process found under `pane` in a fresh poll. The CLI launcher must not type
    /// its command into one. An event gap leaves process observations intact, so only
    /// staleness counts; unknown state returns `None` and does not block.
    pub fn agent_in_pane(&self, pane: &str, now: u64) -> Option<Agent> {
        if !self.valid || now.saturating_sub(self.observed_at) >= 10_000 {
            return None;
        }
        self.observations
            .iter()
            .find(|o| o.pane == pane)
            .map(|o| o.agent)
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
            return t("agent.terminal_status_unknown").into();
        }
        let last = observations
            .iter()
            .filter_map(|o| o.last.as_ref())
            .filter(|e| e.kind != Kind::Ended)
            .min_by_key(|e| e.kind.priority());
        if !self.current(now) || last.is_none() {
            return tr!("agent.name_status_unknown", agents);
        }
        let last = last.unwrap();
        let count = observations
            .iter()
            .filter_map(|o| o.last.as_ref())
            .filter(|e| e.kind.priority() == last.kind.priority())
            .count();
        format!("{agents} · {} {count}", last.kind.label())
    }
    /// Observations carrying a status, in snapshot order. `filter` is a window (`@ID`),
    /// session (`$ID`) or pane (`%ID`); `None` takes every observation.
    pub fn statuses(&self, filter: Option<&str>) -> Vec<&Observation> {
        self.observations
            .iter()
            .filter(|o| o.status.is_some())
            .filter(|o| match filter {
                None => true,
                Some(f) if f.starts_with('@') => o.window == f,
                Some(f) if f.starts_with('$') => o.session == f,
                Some(f) => o.pane == f,
            })
            .collect()
    }
    /// Usage is per account, so every pane of one account shares the newest sample
    /// (largest `limits_at`, else `at`) that carries limits.
    pub fn freshest_limits(&self, agent: Agent, key: &str) -> &[UsageLimit] {
        self.observations
            .iter()
            .filter(|o| o.agent == agent)
            .filter_map(|o| o.status.as_ref())
            .filter(|s| s.account_key() == Some(key) && !s.limits.is_empty())
            .max_by_key(|s| s.limits_sampled_at())
            .map_or(&[], |s| s.limits.as_slice())
    }
    /// Limits shown for `observation`: its account's newest sample, else its own.
    pub fn limits_for<'a>(&'a self, observation: &'a Observation) -> &'a [UsageLimit] {
        let Some(status) = observation.status.as_ref() else {
            return &[];
        };
        match status.account_key() {
            Some(key) => {
                let shared = self.freshest_limits(observation.agent, key);
                if shared.is_empty() {
                    &status.limits
                } else {
                    shared
                }
            }
            None => &status.limits,
        }
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
/// Helper arguments for `Action::AgentSetup`; `--status` is passed for an install only.
pub fn setup_args<'a>(operation: &'a str, token: Option<&'a str>, status: bool) -> Vec<&'a str> {
    let mut args = vec![operation];
    if let Some(token) = token {
        args.extend(["--token", token]);
    }
    if status && operation == "install" {
        args.push("--status");
    }
    args
}
fn limitations(agent: &str) -> &'static str {
    match agent {
        "Codex" => t("agent.limit.questions_not_collected"),
        "Agy" => t("agent.limit.approval_ask_permission"),
        "Grok" => t("agent.limit.idle_prompt_not_collected"),
        _ => "",
    }
}
/// The helper reports codes; the wording and its translations stay in the app.
fn localize_setup(mut value: serde_json::Value) -> serde_json::Value {
    if value.get("error").is_some() {
        value["error"] = t("agent.error.integration_failed").into();
    }
    if let Some(configs) = value.get_mut("configs").and_then(|v| v.as_array_mut()) {
        for c in configs.iter_mut().filter(|c| c.is_object()) {
            let limitations = limitations(c["agent"].as_str().unwrap_or(""));
            c["limitations"] = limitations.into();
            if c["error"].as_str().is_some_and(|e| !e.is_empty()) {
                c["error"] = t("agent.error.config_unreadable").into();
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
    /// A helper that is missing or exits ends its stream: `NotReady`. Every session of
    /// the tmux server is observed, so the sidebar shows CLIs outside the attached one.
    pub async fn poll(&mut self, after: u64, epoch: &str) -> Result<serde_json::Value> {
        let request = serde_json::json!({"session": "", "after": after, "epoch": epoch});
        crate::ssh::write(&self.stream, format!("{request}\n").as_bytes())
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
/// generic `node` heuristics are used by this conservative POSIX fallback. Like the
/// helper, it covers every session of the tmux server.
pub async fn basic_snapshot(ssh: &Ssh) -> Result<Snapshot> {
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
            let agent = Agent::from_command(p[7]);
            Some((
                p[0].parse().ok()?,
                (p[1].parse().ok()?, p[2..7].join("-"), agent),
            ))
        })
        .collect();
    let mut snapshot = Snapshot::default();
    for line in String::from_utf8_lossy(&panes).lines() {
        let p: Vec<_> = line.split('|').collect();
        if p.len() != 4 {
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
                        status: None,
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

/// Which agent status parts the app shows. Stays on this device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentStatusSettings {
    pub status_bar: bool,
    pub sidebar: bool,
    pub account: bool,
    pub usage: bool,
    pub context: bool,
    pub breakdown: bool,
    pub cache: bool,
    pub model: bool,
    pub effort: bool,
    pub cost: bool,
}
impl Default for AgentStatusSettings {
    fn default() -> Self {
        Self {
            status_bar: true,
            sidebar: true,
            account: true,
            usage: true,
            context: true,
            breakdown: true,
            cache: true,
            model: true,
            effort: true,
            cost: false,
        }
    }
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
                status: None,
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
        assert!(snapshot.label(None, 100).contains("작업 이벤트"));
        snapshot.invalidate();
        assert!(snapshot.events.is_empty());
    }
    #[test]
    fn launcher_sees_only_a_fresh_cli_process_in_the_target_pane() {
        let mut snapshot = Snapshot {
            valid: true,
            observed_at: 100,
            observations: vec![Observation {
                session: "$1".into(),
                window: "@1".into(),
                pane: "%1".into(),
                run: "run".into(),
                actor: "parent".into(),
                agent: Agent::Claude,
                // A process without any received hook still counts.
                last: None,
                supported: vec![],
                status: None,
            }],
            ..Default::default()
        };
        assert_eq!(snapshot.agent_in_pane("%1", 100), Some(Agent::Claude));
        assert_eq!(snapshot.agent_in_pane("%2", 100), None);
        snapshot.gap = true;
        assert_eq!(snapshot.agent_in_pane("%1", 100), Some(Agent::Claude));
        assert_eq!(snapshot.agent_in_pane("%1", 10_100), None);
        snapshot.invalidate();
        assert_eq!(snapshot.agent_in_pane("%1", 100), None);
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
            status: None,
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

    fn status_observation(
        pane: &str,
        agent: Agent,
        status: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "session": "$1", "window": "@1", "pane": pane, "run": format!("run{pane}"),
            "actor": "parent", "agent": agent, "last": null, "supported": [],
            "status": status,
        })
    }
    #[test]
    fn status_parses_every_field_and_keeps_unknown_as_none() {
        let full = serde_json::json!({
            "at": 1_763_790_000_000u64,
            "sources": ["mod", "hook"],
            "model": {"id": "claude-opus-4-x", "name": "Opus 4.x"},
            "effort": "high",
            "thinking": true,
            "context": {
                "used": 84000, "size": 200000, "percent": 42.0, "compact_percent": 85.0,
                "breakdown": [
                    {"category": "messages", "name": "Messages", "tokens": 30000, "kind": "used"},
                    {"category": "future_kind", "name": "Plugin data", "tokens": 10},
                ],
                "breakdown_total": 120000, "breakdown_window": 200000,
                "future_field": 1,
            },
            "cache": {"input": 1200, "read": 80000, "write": 2800, "output": 900,
                      "ttl": "5m", "last_at": 1_763_790_000_000u64},
            "cost_usd": 1.23,
            "account": {"kind": "subscription", "plan": "max", "key": "abc"},
            "limits": [{"id": "five_hour", "used_percent": 63.0, "resets_at": 1_763_795_000u64},
                       {"id": "gemini-pro", "label": "Gemini Pro", "remaining_fraction": 0.25}],
            "alert": "rate_limit",
            "unknown_top_level": {"x": 1},
        });
        let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
            "server": "s", "valid": true,
            "observations": [status_observation("%1", Agent::Claude, full),
                             status_observation("%2", Agent::Codex, serde_json::json!({}))],
        }))
        .unwrap();
        snapshot.validate().unwrap();
        let status = snapshot.observations[0].status.as_ref().unwrap();
        assert_eq!(
            status.model.as_ref().unwrap().name.as_deref(),
            Some("Opus 4.x")
        );
        assert_eq!(status.context_percent(), Some(42.0));
        let ratio = status.cache_hit_ratio().unwrap();
        assert!((ratio - 80000.0 / 84000.0).abs() < 1e-9);
        let cache = status.cache.as_ref().unwrap();
        assert_eq!(cache.ttl_ms(), Some(300_000));
        assert_eq!(cache.estimated_expiry_ms(), Some(1_763_790_300_000));
        let parts = &status.context.as_ref().unwrap().breakdown;
        assert_eq!(parts[0].category_key(), "messages");
        assert_eq!(parts[0].label(), t("agent_status.category.messages"));
        assert_eq!(parts[1].category_key(), "other");
        assert_eq!(parts[1].label(), "Plugin data");
        assert_eq!(status.limits[0].used(), Some(63.0));
        assert_eq!(status.limits[0].title(), t("agent_status.limit.five_hour"));
        assert_eq!(status.limits[0].resets_in_s(1_763_794_000_000), Some(1000));
        assert_eq!(status.limits[1].used(), Some(75.0));
        assert_eq!(status.limits[1].title(), "Gemini Pro");
        assert_eq!(
            status.account.as_ref().unwrap().kind_label(),
            Some(t("agent_status.account.subscription"))
        );
        assert_eq!(
            alert_label("rate_limit"),
            Some(t("agent_status.alert.rate_limit"))
        );
        assert_eq!(alert_label("x"), None);
        assert!(!status.is_stale(1_763_790_000_000 + 1000));
        assert!(status.is_stale(1_763_790_000_000 + STATUS_STALE_MS));
        // An empty status is all unknown: nothing becomes zero.
        let empty = snapshot.observations[1].status.as_ref().unwrap();
        assert_eq!(empty, &AgentStatus::default());
        assert_eq!(empty.context_percent(), None);
        assert_eq!(empty.cache_hit_ratio(), None);
        assert!(empty.is_stale(0));
        // Round trip keeps wire names; an observation without status omits the key.
        let wire = serde_json::to_value(&snapshot.observations[0]).unwrap();
        assert_eq!(wire["status"]["context"]["compact_percent"], 85.0);
        assert_eq!(wire["status"]["limits"][0]["resets_at"], 1_763_795_000u64);
        let mut bare = snapshot.observations[0].clone();
        bare.status = None;
        assert!(serde_json::to_value(&bare).unwrap().get("status").is_none());
    }
    #[test]
    fn invalid_status_is_dropped_without_rejecting_the_snapshot() {
        let long = "x".repeat(129);
        for bad in [
            serde_json::json!({"context": {"percent": 101.0}}),
            serde_json::json!({"context": {"percent": -1.0}}),
            serde_json::json!({"context": {"compact_percent": 200.0}}),
            serde_json::json!({"context": {"breakdown": vec![serde_json::json!({"category": "free"}); 25]}}),
            serde_json::json!({"context": {"breakdown": [{"category": "other", "name": "n".repeat(65)}]}}),
            serde_json::json!({"cache": {"hit_ratio": 1.5}}),
            serde_json::json!({"limits": vec![serde_json::json!({"id": "a"}); 17]}),
            serde_json::json!({"limits": [{"id": "a", "used_percent": 1001.0}]}),
            serde_json::json!({"limits": [{"id": "a", "remaining_fraction": 2.0}]}),
            serde_json::json!({"limits": [{"id": long}]}),
            serde_json::json!({"effort": "e".repeat(33)}),
            serde_json::json!({"model": {"id": "a\nb"}}),
            serde_json::json!({"sources": ["a", "b", "c", "d", "e", "f", "g", "h", "i"]}),
            serde_json::json!({"cost_usd": -1.0}),
            // Wrong types fail to parse and are dropped the same way.
            serde_json::json!({"context": {"used": "many"}}),
            serde_json::json!({"limits": {"id": "a"}}),
            serde_json::json!("status"),
        ] {
            let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
                "server": "s", "valid": true,
                "observations": [status_observation("%1", Agent::Claude, bad.clone())],
            }))
            .unwrap();
            snapshot.validate().unwrap();
            assert_eq!(snapshot.observations.len(), 1, "{bad}");
            assert!(snapshot.observations[0].status.is_none(), "{bad}");
        }
        // Exceeded spend limits may report more than 100 %.
        let ok: Snapshot = serde_json::from_value(serde_json::json!({
            "observations": [status_observation("%1", Agent::Claude,
                serde_json::json!({"limits": [{"id": "spend_limit", "used_percent": 140.0}]}))],
        }))
        .unwrap();
        assert!(ok.observations[0].status.is_some());
        // Absent and null status are both none.
        let none: Snapshot = serde_json::from_value(serde_json::json!({
            "observations": [status_observation("%1", Agent::Claude, serde_json::Value::Null),
                {"session": "$1", "window": "@1", "pane": "%2", "run": "r", "actor": "parent",
                 "agent": "Codex", "last": null, "supported": []}],
        }))
        .unwrap();
        assert!(none.observations.iter().all(|o| o.status.is_none()));
    }
    #[test]
    fn statuses_filter_by_target_and_limits_follow_the_newest_account_sample() {
        let mut first = status_observation(
            "%1",
            Agent::Claude,
            // A newer hook moves `at`, but its limits are older than the other pane's.
            serde_json::json!({"at": 50, "limits_at": 10, "account": {"key": "k"},
                "limits": [{"id": "five_hour", "used_percent": 10.0}],
                "context": {"used": 50, "size": 200}}),
        );
        first["window"] = "@1".into();
        let mut second = status_observation(
            "%2",
            Agent::Claude,
            serde_json::json!({"at": 20, "account": {"key": "k"},
                "limits": [{"id": "five_hour", "used_percent": 30.0}]}),
        );
        second["window"] = "@2".into();
        let other_account = status_observation(
            "%3",
            Agent::Claude,
            serde_json::json!({"at": 30, "account": {"key": "other"},
                "limits": [{"id": "five_hour", "used_percent": 90.0}]}),
        );
        let no_key = status_observation(
            "%4",
            Agent::Codex,
            serde_json::json!({"at": 40, "limits": [{"id": "x", "used_percent": 5.0}]}),
        );
        let mut without = status_observation("%5", Agent::Claude, serde_json::Value::Null);
        without["window"] = "@2".into();
        let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
            "observations": [first, second, other_account, no_key, without],
        }))
        .unwrap();
        let panes = |f: Option<&str>| {
            snapshot
                .statuses(f)
                .into_iter()
                .map(|o| o.pane.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(panes(None), ["%1", "%2", "%3", "%4"]);
        assert_eq!(panes(Some("@2")), ["%2"]);
        assert_eq!(panes(Some("%3")), ["%3"]);
        assert_eq!(panes(Some("$1")), ["%1", "%2", "%3", "%4"]);
        assert!(panes(Some("%5")).is_empty());
        assert_eq!(
            snapshot.freshest_limits(Agent::Claude, "k")[0].used_percent,
            Some(30.0)
        );
        assert!(snapshot.freshest_limits(Agent::Codex, "k").is_empty());
        let o = &snapshot.observations;
        assert_eq!(snapshot.limits_for(&o[0])[0].used_percent, Some(30.0));
        assert_eq!(snapshot.limits_for(&o[2])[0].used_percent, Some(90.0));
        assert_eq!(snapshot.limits_for(&o[3])[0].used_percent, Some(5.0));
        assert!(snapshot.limits_for(&o[4]).is_empty());
        assert_eq!(o[0].status.as_ref().unwrap().context_percent(), Some(25.0));
    }
    #[test]
    fn token_and_duration_formats_are_compact() {
        for (n, text) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1k"),
            (1_250, "1.3k"),
            (9_949, "9.9k"),
            (9_950, "10k"),
            (84_000, "84k"),
            (999_499, "999k"),
            (999_500, "1M"),
            (1_200_000, "1.2M"),
            (9_960_000, "10M"),
            (12_400_000, "12M"),
        ] {
            assert_eq!(format_tokens(n), text, "{n}");
        }
        for (s, text) in [
            (0, "0s"),
            (59, "59s"),
            (60, "1m"),
            (180, "3m"),
            (3_600, "1h"),
            (7_500, "2h 5m"),
            (86_400, "1d"),
            (4 * 86_400, "4d"),
            (4 * 86_400 + 3 * 3_600 + 59, "4d 3h"),
        ] {
            assert_eq!(format_duration_s(s), text, "{s}");
        }
        let ttl = |v: &str| {
            CacheUsage {
                ttl: Some(v.into()),
                ..Default::default()
            }
            .ttl_ms()
        };
        assert_eq!(ttl("1h"), Some(3_600_000));
        assert_eq!(ttl("5분"), None);
        assert_eq!(ttl("m"), None);
    }
    #[test]
    fn setup_adds_status_collection_to_install_only() {
        assert_eq!(setup_args("inspect", None, true), ["inspect"]);
        assert_eq!(
            setup_args("install", Some("t"), true),
            ["install", "--token", "t", "--status"]
        );
        assert_eq!(
            setup_args("install", Some("t"), false),
            ["install", "--token", "t"]
        );
        assert_eq!(
            setup_args("remove", Some("t"), true),
            ["remove", "--token", "t"]
        );
        use crate::connection::Action;
        let Action::AgentSetup { status, .. } = serde_json::from_value(
            serde_json::json!({"action": "agentSetup", "operation": "inspect"}),
        )
        .unwrap() else {
            panic!("agentSetup action");
        };
        assert!(!status);
        let Action::AgentSetup { status, .. } = serde_json::from_value(serde_json::json!(
            {"action": "agentSetup", "operation": "install", "token": "t", "status": true}
        ))
        .unwrap() else {
            panic!("agentSetup action");
        };
        assert!(status);
    }
    #[test]
    fn status_settings_default_to_everything_but_cost() {
        let settings: AgentStatusSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, AgentStatusSettings::default());
        assert!(settings.status_bar && settings.sidebar && settings.breakdown);
        assert!(!settings.cost);
        let wire = serde_json::to_value(&settings).unwrap();
        assert_eq!(wire["statusBar"], true);
        assert_eq!(wire["cost"], false);
    }
}
