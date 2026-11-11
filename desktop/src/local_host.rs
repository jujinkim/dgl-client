//! Desktop-only target and resume state. Never enters HostProfile storage or sync payloads.
use super::{Desktop, HostProfile};
use serde::{Deserialize, Serialize};
/// ':' is rejected by HostProfile::validate, so imported SSH IDs cannot collide.
pub const ID: &str = ":local";
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Local,
    Ssh(String),
}
impl Target {
    pub fn of(host: &HostProfile) -> Self {
        if host.id == ID {
            Self::Local
        } else {
            Self::Ssh(host.id.clone())
        }
    }
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub session: Option<String>,
    pub pane: Option<String>,
}
impl Recent {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read(path)
            .ok()
            .filter(|b| b.len() <= 8192)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &std::path::Path) -> dgl_core::Result<()> {
        dgl_core::sync::save_private(
            path,
            &serde_json::to_vec(self).map_err(|_| dgl_core::Error::Storage)?,
        )
    }
    pub fn host(&self) -> HostProfile {
        HostProfile {
            id: ID.into(),
            alias: "Local".into(),
            hostname: "localhost".into(),
            port: 22,
            username: "local".into(),
            auth: Default::default(),
            credential_ref: String::new(),
            favorite: false,
            last_connected: 0,
            last_session: self.session.clone(),
            last_pane: self.pane.clone(),
        }
    }
}
pub fn installation() -> (Option<&'static str>, &'static str) {
    static GUIDE: std::sync::OnceLock<(Option<&'static str>, &'static str)> =
        std::sync::OnceLock::new();
    *GUIDE.get_or_init(|| {
        let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        dgl_core::local::installation(dgl_core::local::LocalConfig::default().multiplexer, &os)
    })
}
impl Desktop {
    pub fn local_host(&self) -> HostProfile {
        self.recent_local.host()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_has_a_separate_identity_and_device_only_resume_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("local-session.json");
        let recent = Recent {
            session: Some("$3".into()),
            pane: Some("%7".into()),
        };
        recent.save(&path).unwrap();
        let host = Recent::load(&path).host();
        assert!(!dgl_core::model::valid_id(&host.id));
        assert_eq!(Target::of(&host), Target::Local);
        assert_eq!(host.last_pane.as_deref(), Some("%7"));
        let state = dgl_core::model::LocalState::default();
        assert!(!serde_json::to_string(&state).unwrap().contains(ID));
        assert!(state.hosts.is_empty());
    }
}
