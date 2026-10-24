use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthKind {
    #[default]
    Password,
    PrivateKey,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostProfile {
    pub id: String,
    pub alias: String,
    pub hostname: String,
    #[serde(default = "port")]
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub auth: AuthKind,
    #[serde(default)]
    pub credential_ref: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub last_connected: i64,
    #[serde(default)]
    pub last_session: Option<String>,
    #[serde(default)]
    pub last_pane: Option<String>,
}
fn port() -> u16 {
    22
}
pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
}
impl HostProfile {
    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.id)
            || !valid_id(&self.credential_ref)
            || self.port == 0
            || self.alias.trim().is_empty()
            || self.alias.encode_utf16().count() > 128
            || self.hostname.trim().is_empty()
            || self.hostname.len() > 253
            || self.hostname.chars().any(|c| c.is_whitespace() || c < ' ')
            || self.username.trim().is_empty()
            || self.username.encode_utf16().count() > 128
            || self.username.chars().any(|c| c < ' ')
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.hostname.to_lowercase(), self.port)
    }
}
#[derive(Clone, Default, Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub private_key: String,
    #[serde(default)]
    pub passphrase: String,
}
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credential([redacted])")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub theme: String,
    pub font_size: i32,
    pub scrollback: i32,
    pub volume_keys_enabled: bool,
    pub volume_up: crate::input::VolumeBinding,
    pub volume_down: crate::input::VolumeBinding,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            font_size: 14,
            scrollback: 5000,
            volume_keys_enabled: false,
            volume_up: Default::default(),
            volume_down: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalState {
    pub hosts: Vec<HostProfile>,
    pub trust: BTreeMap<String, String>,
    pub preferences: Preferences,
}
impl LocalState {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::InvalidArgument);
        }
        // credentialRef defaults to the host ID only when absent, not when malformed.
        let mut value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| Error::InvalidArgument)?;
        if let Some(hosts) = value.get_mut("hosts").and_then(|x| x.as_array_mut()) {
            for h in hosts {
                if h.get("credentialRef").is_none() {
                    h["credentialRef"] = h["id"].clone();
                }
            }
        }
        let state: Self = serde_json::from_value(value).map_err(|_| Error::InvalidArgument)?;
        state.validate()?;
        Ok(state)
    }
    pub fn validate(&self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for h in &self.hosts {
            h.validate()?;
            if !ids.insert(&h.id) {
                return Err(Error::InvalidArgument);
            }
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        use std::io::Read;
        match std::fs::File::open(path) {
            Ok(f) => {
                let mut data = Vec::new();
                f.take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut data)
                    .map_err(|_| Error::Storage)?;
                Self::decode(&data)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(Error::Storage),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| Error::Storage)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::InvalidArgument);
        }
        let parent = path.parent().ok_or(Error::Storage)?;
        std::fs::create_dir_all(parent).map_err(|_| Error::Storage)?;
        let temp = path.with_extension("json.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&temp).map_err(|_| Error::Storage)?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::Storage)?;
        std::fs::rename(temp, path).map_err(|_| Error::Storage)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub columns: u16,
    pub rows: u16,
}
impl Default for Size {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
        }
    }
}
impl Size {
    pub fn new(columns: u16, rows: u16) -> Result<Self> {
        if !(2..=500).contains(&columns) || !(2..=250).contains(&rows) {
            Err(Error::InvalidArgument)
        } else {
            Ok(Self { columns, rows })
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteState {
    pub sessions: Vec<RemoteSession>,
    pub windows: Vec<RemoteWindow>,
    pub panes: Vec<RemotePane>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSession {
    pub id: String,
    pub name: String,
    pub attached: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWindow {
    pub id: String,
    pub session: String,
    pub name: String,
    pub active: bool,
    pub layout: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePane {
    pub id: String,
    pub window: String,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub mode: String,
    pub command: String,
}

pub fn split_record(row: &[u8], count: usize) -> Result<Vec<String>> {
    let text = std::str::from_utf8(row).map_err(|_| Error::Protocol)?;
    let mut fields = vec![];
    let mut part = String::new();
    let mut escaped = false;
    for c in text.chars() {
        if c == '|' && !escaped {
            fields.push(std::mem::take(&mut part));
        } else {
            part.push(c);
            escaped = c == '\\' && !escaped;
        }
    }
    fields.push(part);
    if fields.len() != count {
        return Err(Error::Protocol);
    }
    Ok(fields)
}
fn unquote(s: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            out.push(c);
            escaped = false
        } else if c == '\\' {
            escaped = true
        } else {
            out.push(c)
        }
    }
    if escaped {
        out.push('\\')
    }
    out
}
fn target(s: &str, prefix: char) -> Result<String> {
    crate::command::target(s, prefix)
        .map(|_| s.to_string())
        .map_err(|_| Error::Protocol)
}
fn num<T: std::str::FromStr>(s: &str) -> Result<T> {
    s.parse().map_err(|_| Error::Protocol)
}
pub fn sessions(rows: &[Vec<u8>]) -> Result<Vec<RemoteSession>> {
    rows.iter()
        .map(|r| {
            let f = split_record(r, 3)?;
            Ok(RemoteSession {
                id: target(&f[0], '$')?,
                name: unquote(&f[1]),
                attached: num(&f[2])?,
            })
        })
        .collect()
}
pub fn windows(rows: &[Vec<u8>]) -> Result<Vec<RemoteWindow>> {
    rows.iter()
        .map(|r| {
            let f = split_record(r, 5)?;
            Ok(RemoteWindow {
                id: target(&f[1], '@')?,
                session: target(&f[0], '$')?,
                name: unquote(&f[2]),
                active: f[3] == "1",
                layout: f[4].clone(),
            })
        })
        .collect()
}
pub fn panes(rows: &[Vec<u8>]) -> Result<Vec<RemotePane>> {
    rows.iter()
        .map(|r| {
            let f = split_record(r, 7)?;
            Ok(RemotePane {
                id: target(&f[0], '%')?,
                window: target(&f[1], '@')?,
                width: num(&f[2])?,
                height: num(&f[3])?,
                active: f[4] == "1",
                mode: unquote(&f[5]),
                command: unquote(&f[6]),
            })
        })
        .collect()
}
