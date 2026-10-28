//! Provider-independent file synchronization. The shared document is an allowlist,
//! never a serialized LocalState. An append-only causal journal survives delayed
//! cloud writes; concurrent values remain explicit until the user resolves them.
use crate::{
    Error, Result,
    model::{AuthKind, HostProfile, LocalState},
};
use crate::{i18n::t, tr};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHANGES: usize = 20_000;
pub const FILE_NAME: &str = "dgl-sync.json";
fn identity() -> String {
    format!("{:032x}", rand::random::<u128>())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SharedHost {
    pub id: String,
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthKind,
    pub favorite: bool,
}
impl SharedHost {
    fn from_local(h: &HostProfile) -> Self {
        Self {
            id: h.id.clone(),
            alias: h.alias.clone(),
            hostname: h.hostname.clone(),
            port: h.port,
            username: h.username.clone(),
            auth: h.auth.clone(),
            favorite: h.favorite,
        }
    }
    fn local(&self, previous: Option<&HostProfile>) -> HostProfile {
        let compatible = previous.filter(|h| {
            h.hostname == self.hostname
                && h.port == self.port
                && h.username == self.username
                && h.auth == self.auth
        });
        HostProfile {
            id: self.id.clone(),
            alias: self.alias.clone(),
            hostname: self.hostname.clone(),
            port: self.port,
            username: self.username.clone(),
            auth: self.auth.clone(),
            favorite: self.favorite,
            credential_ref: compatible
                .map(|h| h.credential_ref.clone())
                .unwrap_or_else(|| format!("sync-{}", identity())),
            last_connected: compatible.map_or(0, |h| h.last_connected),
            last_session: compatible.and_then(|h| h.last_session.clone()),
            last_pane: compatible.and_then(|h| h.last_pane.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", deny_unknown_fields)]
pub enum Value {
    Host(Option<SharedHost>),
    Theme(String),
    Accent(crate::accent::AccentSettings),
    FontSize(i32),
    Scrollback(i32),
    Questions(bool),
    Responses(bool),
    Errors(bool),
}
impl Value {
    fn valid(&self, key: &str) -> bool {
        match self {
            Self::Host(host) => key.strip_prefix("host/").is_some_and(|id| {
                crate::model::valid_id(id)
                    && host
                        .as_ref()
                        .is_none_or(|h| h.id == id && h.local(None).validate().is_ok())
            }),
            Self::Theme(v) => key == "theme" && ["system", "dark", "light"].contains(&v.as_str()),
            Self::Accent(v) => key == "accent" && v.valid(),
            Self::FontSize(v) => key == "fontSize" && (10..=28).contains(v),
            Self::Scrollback(v) => key == "scrollback" && (0..=1_000_000).contains(v),
            Self::Questions(_) => key == "questions",
            Self::Responses(_) => key == "responses",
            Self::Errors(_) => key == "errors",
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Host(Some(h)) => format!(
                "{} · {}@{}:{} · {:?}{}",
                h.alias,
                h.username,
                h.hostname,
                h.port,
                h.auth,
                if h.favorite {
                    t(" · 즐겨찾기")
                } else {
                    ""
                }
            ),
            Self::Host(None) => t("호스트 삭제").into(),
            Self::Theme(v) => tr!("테마: {0}", v),
            Self::Accent(v) => tr!(
                "강조 색상: {0} {1}",
                v.mode,
                v.custom_color.as_deref().unwrap_or("")
            ),
            Self::FontSize(v) => tr!("글자 크기: {0}", v),
            Self::Scrollback(v) => tr!("스크롤백: {0}", v),
            Self::Questions(v) => tr!("질문 알림: {0}", v),
            Self::Responses(v) => tr!("응답 알림: {0}", v),
            Self::Errors(v) => tr!("오류 알림: {0}", v),
        }
    }
}
type Snapshot = BTreeMap<String, Value>;
fn snapshot(local: &LocalState) -> Snapshot {
    let p = &local.preferences;
    let mut result = BTreeMap::from([
        ("theme".into(), Value::Theme(p.theme.clone())),
        ("accent".into(), Value::Accent(p.accent.clone())),
        ("fontSize".into(), Value::FontSize(p.font_size)),
        ("scrollback".into(), Value::Scrollback(p.scrollback)),
        (
            "questions".into(),
            Value::Questions(p.agent_notifications.questions),
        ),
        (
            "responses".into(),
            Value::Responses(p.agent_notifications.responses),
        ),
        ("errors".into(), Value::Errors(p.agent_notifications.errors)),
    ]);
    for h in &local.hosts {
        result.insert(
            format!("host/{}", h.id),
            Value::Host(Some(SharedHost::from_local(h))),
        );
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    clock: u64,
    key: String,
    value: Value,
    parents: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    format: String,
    version: u32,
    dataset: String,
    changes: BTreeMap<String, Change>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            format: "dgl-sync".into(),
            version: 2,
            dataset: identity(),
            changes: BTreeMap::new(),
        }
    }
}
impl Document {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::InvalidArgument);
        }
        let mut doc: Self = serde_json::from_slice(bytes).map_err(|_| Error::InvalidArgument)?;
        doc.validate()?;
        doc.version = 2;
        Ok(doc)
    }
    fn validate(&self) -> Result<()> {
        if self.format != "dgl-sync"
            || ![1, 2].contains(&self.version)
            || !crate::model::valid_id(&self.dataset)
            || self.changes.len() > MAX_CHANGES
        {
            return Err(Error::InvalidArgument);
        }
        for (id, c) in &self.changes {
            if !crate::model::valid_id(id)
                || c.clock == 0
                || !c.value.valid(&c.key)
                || (self.version == 1 && matches!(c.value, Value::Accent(_)))
                || c.parents.len() > MAX_CHANGES
            {
                return Err(Error::InvalidArgument);
            }
            for parent in &c.parents {
                let p = self.changes.get(parent).ok_or(Error::InvalidArgument)?;
                if p.key != c.key || p.clock >= c.clock {
                    return Err(Error::InvalidArgument);
                }
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String> {
        self.validate()?;
        let json = serde_json::to_string(self).map_err(|_| Error::Storage)?;
        if json.len() > MAX_BYTES {
            return Err(Error::InvalidArgument);
        }
        Ok(json)
    }
    fn merge(&mut self, other: Self) -> Result<()> {
        if self.dataset != other.dataset {
            return Err(Error::InvalidArgument);
        }
        for (id, c) in other.changes {
            if self.changes.get(&id).is_some_and(|old| old != &c) {
                return Err(Error::InvalidArgument);
            }
            self.changes.insert(id, c);
        }
        self.validate()
    }
    fn heads(&self) -> BTreeMap<String, Vec<String>> {
        let superseded: BTreeSet<_> = self
            .changes
            .values()
            .flat_map(|c| c.parents.iter())
            .collect();
        let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, c) in &self.changes {
            if !superseded.contains(id) {
                result.entry(c.key.clone()).or_default().push(id.clone());
            }
        }
        result
    }
    fn add(
        &mut self,
        device: &str,
        key: String,
        value: Value,
        parents: Vec<String>,
    ) -> Result<String> {
        let clock = self
            .changes
            .values()
            .map(|c| c.clock)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(Error::InvalidArgument)?;
        let id = format!("{device}-{}", identity());
        self.changes.insert(
            id.clone(),
            Change {
                clock,
                key,
                value,
                parents: parents.into_iter().collect(),
            },
        );
        Ok(id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cache {
    device: String,
    observed: Snapshot,
    document: Document,
    #[serde(default)]
    known: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub key: String,
    pub revision: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub revision: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conflict {
    pub key: String,
    pub choices: Vec<Choice>,
}
pub struct Exchange {
    pub cache: Cache,
    pub local: LocalState,
    pub document: String,
    pub conflicts: Vec<Conflict>,
}
impl Cache {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES * 2 {
            return Err(Error::InvalidArgument);
        }
        let cache: Self = serde_json::from_slice(bytes).map_err(|_| Error::InvalidArgument)?;
        cache.upgrade()
    }
    fn upgrade(mut self) -> Result<Self> {
        self.document.validate()?;
        if !crate::model::valid_id(&self.device)
            || self.observed.iter().any(|(k, v)| !v.valid(k))
            || self.known.iter().any(|(key, ids)| {
                ids.iter()
                    .any(|id| self.document.changes.get(id).is_none_or(|c| &c.key != key))
            })
        {
            return Err(Error::InvalidArgument);
        }
        self.document.version = 2;
        self.observed
            .entry("accent".into())
            .or_insert_with(|| Value::Accent(Default::default()));
        Ok(self)
    }
    pub fn encode(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|_| Error::Storage)
    }
    /// Advance causal knowledge only after the proposed local settings were saved.
    /// Learning a remote revision during I/O is not equivalent to displaying it.
    pub fn acknowledge(&mut self, local: &LocalState) {
        self.observed = snapshot(local);
        for (key, heads) in self.document.heads() {
            let value = self
                .observed
                .get(&key)
                .cloned()
                .unwrap_or(Value::Host(None));
            let matching: Vec<_> = heads
                .into_iter()
                .filter(|id| self.document.changes[id].value == value)
                .collect();
            if !matching.is_empty() {
                self.known.insert(key, matching);
            }
        }
    }
}

/// Capture local changes against the previously observed state before learning any
/// remote revisions. A device must durably retain the returned cache before writing
/// the shared file. Re-exchange against the latest local state before applying.
pub fn exchange(
    cache: Option<Cache>,
    local: &LocalState,
    remote: &[u8],
    resolutions: &[Resolution],
) -> Result<Exchange> {
    local.validate()?;
    let remote = Document::decode(remote)?;
    // Desktop checkpoints embed Cache directly via serde; UniFFI uses decode.
    // Both entry paths must validate and migrate before adding v2 revisions.
    let cache = cache.map(Cache::upgrade).transpose()?;
    let current = snapshot(local);
    let first = cache.is_none();
    let mut cache = cache.unwrap_or_else(|| Cache {
        device: identity(),
        observed: Snapshot::new(),
        document: remote.clone(),
        known: BTreeMap::new(),
    });
    let heads = cache.document.heads();
    let keys: BTreeSet<_> = current
        .keys()
        .chain(cache.observed.keys())
        .cloned()
        .collect();
    for key in keys {
        let value = current.get(&key).cloned().unwrap_or(Value::Host(None));
        if !value.valid(&key) {
            return Err(Error::InvalidArgument);
        }
        let parents = heads.get(&key).cloned().unwrap_or_default();
        // First attach adopts existing common preferences, merges local hosts.
        if first && !key.starts_with("host/") && !parents.is_empty() {
            continue;
        }
        let matching: Vec<_> = parents
            .iter()
            .filter(|id| cache.document.changes[*id].value == value)
            .cloned()
            .collect();
        if !matching.is_empty() {
            cache.known.insert(key, matching);
            continue;
        }
        // Missing in v1 means system/default, not a new local edit. Do not
        // manufacture an upgrade revision concurrent with a remote color choice.
        if key == "accent" && parents.is_empty() && value == Value::Accent(Default::default()) {
            continue;
        }
        if !first && cache.observed.get(&key) == current.get(&key) {
            continue;
        }
        let parents = if first {
            vec![]
        } else {
            cache.known.get(&key).cloned().unwrap_or_default()
        };
        let id = cache
            .document
            .add(&cache.device, key.clone(), value, parents)?;
        cache.known.insert(key, vec![id]);
    }
    let resolution_heads = cache.document.heads();
    cache.document.merge(remote)?;
    for resolution in resolutions {
        let heads = cache.document.heads();
        let choices = heads.get(&resolution.key).ok_or(Error::InvalidArgument)?;
        if !choices.contains(&resolution.revision) {
            return Err(Error::StaleGeneration);
        }
        let seen = resolution_heads
            .get(&resolution.key)
            .ok_or(Error::StaleGeneration)?;
        if !seen.contains(&resolution.revision) {
            return Err(Error::StaleGeneration);
        }
        let value = cache.document.changes[&resolution.revision].value.clone();
        cache
            .document
            .add(&cache.device, resolution.key.clone(), value, seen.clone())?;
    }
    let mut next = local.clone();
    let mut conflicts = vec![];
    for (key, heads) in cache.document.heads() {
        let first = &cache.document.changes[&heads[0]].value;
        if heads
            .iter()
            .any(|id| &cache.document.changes[id].value != first)
        {
            conflicts.push(Conflict {
                key,
                choices: heads
                    .iter()
                    .map(|id| Choice {
                        revision: id.clone(),
                        label: cache.document.changes[id].value.label(),
                    })
                    .collect(),
            });
            continue;
        }
        match first {
            Value::Host(host) => {
                let id = key.strip_prefix("host/").ok_or(Error::InvalidArgument)?;
                let index = next.hosts.iter().position(|h| h.id == id);
                if let Some(h) = host {
                    let value = h.local(index.map(|i| &next.hosts[i]));
                    if let Some(i) = index {
                        next.hosts[i] = value;
                    } else {
                        next.hosts.push(value);
                    }
                } else if let Some(i) = index {
                    next.hosts.remove(i);
                }
            }
            Value::Theme(v) => next.preferences.theme = v.clone(),
            Value::Accent(v) => next.preferences.accent = v.clone(),
            Value::FontSize(v) => next.preferences.font_size = *v,
            Value::Scrollback(v) => next.preferences.scrollback = *v,
            Value::Questions(v) => next.preferences.agent_notifications.questions = *v,
            Value::Responses(v) => next.preferences.agent_notifications.responses = *v,
            Value::Errors(v) => next.preferences.agent_notifications.errors = *v,
        }
    }
    // Track the input, not the proposal: a failed local save must not look like an
    // edit on retry. Successful applications matching a head need no new revision.
    cache.observed = current;
    next.validate()?;
    let document = cache.document.encode()?;
    Ok(Exchange {
        cache,
        local: next,
        document,
        conflicts,
    })
}

/// Atomic private checkpoint; never use this replacement operation for a selected
/// cloud document, whose provider identity/Android URI must remain stable.
pub fn save_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().ok_or(Error::Storage)?;
    std::fs::create_dir_all(parent).map_err(|_| Error::Storage)?;
    let temporary = parent.join(format!(".sync-{}.tmp", identity()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|_| Error::Storage)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Storage)?;
        std::fs::rename(&temporary, path).map_err(|_| Error::Storage)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn exchange(
        cache: Option<Cache>,
        local: &LocalState,
        remote: &[u8],
        resolutions: &[Resolution],
    ) -> Result<Exchange> {
        let mut result = super::exchange(cache, local, remote, resolutions)?;
        result.cache.acknowledge(&result.local);
        Ok(result)
    }
    fn host(id: &str) -> HostProfile {
        SharedHost {
            id: id.into(),
            alias: id.into(),
            hostname: "example.test".into(),
            port: 22,
            username: "user".into(),
            auth: AuthKind::Password,
            favorite: false,
        }
        .local(None)
    }
    fn initial() -> (Exchange, String) {
        let mut local = LocalState::default();
        local.hosts.push(host("one"));
        let empty = Document::default().encode().unwrap();
        let a = exchange(None, &local, empty.as_bytes(), &[]).unwrap();
        let doc = a.document.clone();
        (a, doc)
    }
    #[test]
    fn allowlist_and_credentials_stay_local() {
        let (a, doc) = initial();
        let mut local = a.local.clone();
        local.hosts[0].credential_ref = "SECRET-REFERENCE".into();
        local.hosts[0].last_session = Some("PRIVATE-SESSION".into());
        local
            .trust
            .insert("PRIVATE-TRUST".into(), "fingerprint".into());
        local
            .preferences
            .volume_up
            .steps
            .push(crate::input::MacroStep::Text {
                text: "PRIVATE-MACRO-TOKEN".into(),
            });
        local.preferences.agent_notifications.os = true;
        local.preferences.show_all_hierarchy = true;
        local.preferences.language = "ko".into();
        local.preferences.keep_connections_in_background = false;
        local.preferences.mobile_shortcut_bar_two_rows = false;
        local.preferences.android_keyboard_mode = "resize".into();
        local.preferences.android_keyboard_always_visible = true;
        let out = exchange(Some(a.cache), &local, doc.as_bytes(), &[]).unwrap();
        for value in [
            "SECRET-REFERENCE",
            "PRIVATE-SESSION",
            "PRIVATE-TRUST",
            "PRIVATE-MACRO-TOKEN",
            "credentialRef",
            "volumeUp",
            "showAllHierarchy",
            "language",
            "keepConnectionsInBackground",
            "mobileShortcutBarTwoRows",
            "androidKeyboardMode",
            "androidKeyboardAlwaysVisible",
            "privateKey",
            "password",
        ] {
            assert!(!out.document.contains(value));
            assert!(!out.cache.encode().unwrap().contains(value));
        }
        assert_eq!(out.local, local);
        let other = exchange(None, &LocalState::default(), out.document.as_bytes(), &[]).unwrap();
        assert_ne!(
            other.local.hosts[0].credential_ref,
            local.hosts[0].credential_ref
        );
        assert!(other.local.trust.is_empty());
        assert!(other.local.preferences.volume_up.steps.is_empty());
        assert!(!other.local.preferences.agent_notifications.os);
        assert!(!other.local.preferences.show_all_hierarchy);
        assert_eq!(other.local.preferences.language, "system");
        assert!(other.local.preferences.keep_connections_in_background);
        assert!(other.local.preferences.mobile_shortcut_bar_two_rows);
        assert_eq!(other.local.preferences.android_keyboard_mode, "auto");
        assert!(!other.local.preferences.android_keyboard_always_visible);
    }
    #[test]
    fn offline_disjoint_changes_and_late_cloud_overwrite_converge() {
        let (a, doc) = initial();
        let b = exchange(None, &LocalState::default(), doc.as_bytes(), &[]).unwrap();
        let mut la = a.local.clone();
        la.hosts.push(host("two"));
        let mut lb = b.local.clone();
        lb.preferences.theme = "dark".into();
        let a = exchange(Some(a.cache), &la, doc.as_bytes(), &[]).unwrap();
        let b = exchange(Some(b.cache), &lb, doc.as_bytes(), &[]).unwrap();
        let a = exchange(Some(a.cache), &a.local, b.document.as_bytes(), &[]).unwrap();
        let b = exchange(Some(b.cache), &b.local, a.document.as_bytes(), &[]).unwrap();
        assert_eq!(a.document, b.document);
        assert_eq!(b.local.hosts.len(), 2);
        assert_eq!(b.local.preferences.theme, "dark");
        assert!(b.conflicts.is_empty());
    }
    #[test]
    fn edit_delete_conflict_is_preserved_and_resolved() {
        let (a, doc) = initial();
        let b = exchange(None, &LocalState::default(), doc.as_bytes(), &[]).unwrap();
        let mut la = a.local.clone();
        la.hosts.clear();
        let mut lb = b.local.clone();
        lb.hosts[0].alias = "edited".into();
        let a = exchange(Some(a.cache), &la, doc.as_bytes(), &[]).unwrap();
        let b = exchange(Some(b.cache), &lb, doc.as_bytes(), &[]).unwrap();
        let a = exchange(Some(a.cache), &a.local, b.document.as_bytes(), &[]).unwrap();
        assert_eq!(a.conflicts.len(), 1);
        assert_eq!(a.conflicts[0].choices.len(), 2);
        let deletion = a.conflicts[0]
            .choices
            .iter()
            .find(|c| c.label == "호스트 삭제")
            .unwrap();
        let resolved = exchange(
            Some(a.cache),
            &a.local,
            a.document.as_bytes(),
            &[Resolution {
                key: "host/one".into(),
                revision: deletion.revision.clone(),
            }],
        )
        .unwrap();
        let b = exchange(Some(b.cache), &b.local, resolved.document.as_bytes(), &[]).unwrap();
        assert!(b.local.hosts.is_empty());
        assert!(b.conflicts.is_empty());
        let stale = exchange(None, &LocalState::default(), doc.as_bytes(), &[]).unwrap();
        let b = exchange(Some(b.cache), &b.local, stale.document.as_bytes(), &[]).unwrap();
        assert!(b.local.hosts.is_empty());
    }
    #[test]
    fn failed_apply_retry_and_changed_endpoint_do_not_reuse_credentials() {
        let (a, doc) = initial();
        let b = exchange(None, &LocalState::default(), doc.as_bytes(), &[]).unwrap();
        let mut la = a.local.clone();
        la.hosts[0].hostname = "other.test".into();
        let a = exchange(Some(a.cache), &la, doc.as_bytes(), &[]).unwrap();
        let merged = super::exchange(Some(b.cache), &b.local, a.document.as_bytes(), &[]).unwrap();
        assert_ne!(
            merged.local.hosts[0].credential_ref,
            b.local.hosts[0].credential_ref
        );
        let retry =
            super::exchange(Some(merged.cache), &b.local, a.document.as_bytes(), &[]).unwrap();
        assert_eq!(retry.local.hosts[0].hostname, "other.test");
        assert!(retry.conflicts.is_empty());
        let again = exchange(
            Some(retry.cache),
            &retry.local,
            retry.document.as_bytes(),
            &[],
        )
        .unwrap();
        assert_eq!(again.document, retry.document);
    }
    #[test]
    fn edits_during_io_or_failed_apply_remain_concurrent_until_acknowledged() {
        let (a, doc) = initial();
        let b = exchange(None, &LocalState::default(), doc.as_bytes(), &[]).unwrap();
        let mut remote = b.local.clone();
        remote.hosts[0].alias = "remote edit".into();
        let b = exchange(Some(b.cache), &remote, doc.as_bytes(), &[]).unwrap();
        let pending = super::exchange(Some(a.cache), &a.local, b.document.as_bytes(), &[]).unwrap();
        let mut concurrent = a.local.clone();
        concurrent.hosts[0].alias = "edit while reading".into();
        let out = exchange(
            Some(pending.cache.clone()),
            &concurrent,
            pending.document.as_bytes(),
            &[],
        )
        .unwrap();
        assert_eq!(out.conflicts.len(), 1);
        assert_eq!(out.conflicts[0].choices.len(), 2);
        let mut applied = pending.cache;
        applied.acknowledge(&pending.local);
        let out = exchange(Some(applied), &concurrent, pending.document.as_bytes(), &[]).unwrap();
        assert!(out.conflicts.is_empty());
        assert_eq!(out.local.hosts[0].alias, "edit while reading");
    }
    #[test]
    fn reject_wrong_format_missing_parents_future_schema_and_cross_dataset() {
        let (a, doc) = initial();
        for source in ["{}", "", "{\"password\":\"x\"}"] {
            assert!(Document::decode(source.as_bytes()).is_err());
        }
        let mut value: serde_json::Value = serde_json::from_str(&doc).unwrap();
        value["version"] = 3.into();
        assert!(Document::decode(value.to_string().as_bytes()).is_err());
        let other = Document::default().encode().unwrap();
        assert!(exchange(Some(a.cache), &a.local, other.as_bytes(), &[]).is_err());
        let mut document = Document::decode(doc.as_bytes()).unwrap();
        document
            .changes
            .values_mut()
            .next()
            .unwrap()
            .parents
            .insert("missing".into());
        assert!(document.encode().is_err());
    }
    #[test]
    fn v1_cache_upgrades_without_conflict_and_delayed_v1_preserves_history() {
        let (a, doc) = initial();
        // A real v1 has neither an accent revision nor an observed accent value.
        let mut legacy: serde_json::Value = serde_json::from_str(&doc).unwrap();
        legacy["version"] = 1.into();
        let v1 = legacy.to_string();
        let mut cache: serde_json::Value =
            serde_json::from_str(&a.cache.encode().unwrap()).unwrap();
        cache["document"] = legacy.clone();
        cache["observed"].as_object_mut().unwrap().remove("accent");
        let migrated = Cache::decode(cache.to_string().as_bytes()).unwrap();
        let embedded: Cache = serde_json::from_value(cache).unwrap();
        assert_eq!(migrated.document.dataset, a.cache.document.dataset);
        assert_eq!(migrated.document.changes, a.cache.document.changes);
        let upgraded = exchange(Some(migrated.clone()), &a.local, v1.as_bytes(), &[]).unwrap();
        assert!(upgraded.conflicts.is_empty());
        assert_eq!(
            upgraded.cache.document.changes.len(),
            a.cache.document.changes.len()
        );
        assert_eq!(upgraded.cache.document.version, 2);

        let mut custom = a.local.clone();
        custom.preferences.accent = crate::accent::AccentSettings {
            mode: "custom".into(),
            custom_color: Some("#E6BB24".into()),
        };
        let edited = exchange(Some(embedded), &custom, v1.as_bytes(), &[]).unwrap();
        let applied = exchange(Some(migrated), &a.local, edited.document.as_bytes(), &[]).unwrap();
        assert!(applied.conflicts.is_empty());
        assert_eq!(applied.local.preferences.accent, custom.preferences.accent);
        let mut old_doc = Document::decode(v1.as_bytes()).unwrap();
        let parents = old_doc.heads()["fontSize"].clone();
        old_doc
            .add(
                "old-device",
                "fontSize".into(),
                Value::FontSize(22),
                parents,
            )
            .unwrap();
        let mut old_json = serde_json::to_value(old_doc).unwrap();
        old_json["version"] = 1.into();
        let merged = exchange(
            Some(applied.cache),
            &applied.local,
            old_json.to_string().as_bytes(),
            &[],
        )
        .unwrap();
        assert!(merged.conflicts.is_empty());
        assert_eq!(merged.local.preferences.font_size, 22);
        assert_eq!(merged.local.preferences.accent, custom.preferences.accent);
        assert_eq!(merged.cache.document.dataset, a.cache.document.dataset);
        assert!(
            a.cache.document.changes.keys().all(|id| merged
                .cache
                .document
                .changes
                .contains_key(id))
        );
    }
    #[test]
    fn accent_is_one_atomic_choice_with_explicit_concurrent_resolution() {
        let (a, doc) = initial();
        let b = exchange(None, &a.local, doc.as_bytes(), &[]).unwrap();
        let mut la = a.local.clone();
        la.preferences.accent = crate::accent::AccentSettings {
            mode: "custom".into(),
            custom_color: Some("#8056D9".into()),
        };
        let mut lb = b.local.clone();
        lb.preferences.accent = crate::accent::AccentSettings {
            mode: "default".into(),
            custom_color: Some("#21999D".into()),
        };
        let a = exchange(Some(a.cache), &la, doc.as_bytes(), &[]).unwrap();
        let b = exchange(Some(b.cache), &lb, doc.as_bytes(), &[]).unwrap();
        let conflict = exchange(Some(a.cache), &a.local, b.document.as_bytes(), &[]).unwrap();
        assert_eq!(conflict.conflicts.len(), 1);
        assert_eq!(conflict.conflicts[0].key, "accent");
        assert_eq!(conflict.local.preferences.accent, la.preferences.accent);
        let revision = conflict.conflicts[0]
            .choices
            .iter()
            .find(|c| c.label.contains("#21999D"))
            .unwrap()
            .revision
            .clone();
        let resolved = exchange(
            Some(conflict.cache),
            &conflict.local,
            conflict.document.as_bytes(),
            &[Resolution {
                key: "accent".into(),
                revision,
            }],
        )
        .unwrap();
        assert!(resolved.conflicts.is_empty());
        assert_eq!(resolved.local.preferences.accent, lb.preferences.accent);
        assert!(!resolved.document.contains("systemColor"));
        assert!(!resolved.document.contains("resolved"));
    }
}
