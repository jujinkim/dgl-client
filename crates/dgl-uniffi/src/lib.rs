//! Kotlin owns this object explicitly; shutdown joins the actor before wrapper close().
use dgl_core::{
    connection::{Action, Connection},
    flow::{OutputEvent, OutputKind},
    model::{Credential, HostProfile},
};
use std::sync::{Arc, OnceLock};
uniffi::setup_scaffolding!();
/// Configure UI copy only; action names, error codes and terminal bytes stay stable.
#[uniffi::export]
pub fn configure_language(preference: String, system_locale: String) {
    dgl_core::i18n::configure(&preference, &system_locale);
}
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("native runtime")
    })
}
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{code}")]
    Failure { code: String },
}
impl From<dgl_core::Error> for CoreError {
    fn from(e: dgl_core::Error) -> Self {
        Self::Failure {
            code: e.to_string(),
        }
    }
}
fn invalid() -> CoreError {
    dgl_core::Error::InvalidArgument.into()
}
#[derive(uniffi::Record)]
pub struct NativeKeyPair {
    pub private_key: String,
    pub public_key: String,
}
#[uniffi::export]
pub fn generate_key() -> Result<NativeKeyPair, CoreError> {
    let credential = dgl_core::ssh::generate_key()?;
    Ok(NativeKeyPair {
        public_key: dgl_core::ssh::public_key(&credential)?,
        private_key: credential.private_key.clone(),
    })
}
#[uniffi::export]
pub fn public_key(private_key: String, passphrase: String) -> Result<String, CoreError> {
    Ok(dgl_core::ssh::public_key(&Credential {
        password: String::new(),
        private_key,
        passphrase,
    })?)
}
/// Typed so a new core output kind fails Kotlin's exhaustive `when` instead of being acked unseen.
#[derive(uniffi::Enum)]
pub enum NativeOutputKind {
    Resize,
    ResetBegin,
    ResetChunk,
    ResetEnd,
    Live,
}
impl From<OutputKind> for NativeOutputKind {
    fn from(kind: OutputKind) -> Self {
        match kind {
            OutputKind::Resize => Self::Resize,
            OutputKind::ResetBegin => Self::ResetBegin,
            OutputKind::ResetChunk => Self::ResetChunk,
            OutputKind::ResetEnd => Self::ResetEnd,
            OutputKind::Live => Self::Live,
        }
    }
}
#[derive(uniffi::Record)]
pub struct NativeOutput {
    pub subscription: u64,
    pub sequence: u64,
    pub target_json: String,
    pub kind: NativeOutputKind,
    pub columns: u16,
    pub rows: u16,
    pub bytes: Vec<u8>,
}
impl From<OutputEvent> for NativeOutput {
    fn from(e: OutputEvent) -> Self {
        Self {
            subscription: e.subscription,
            sequence: e.sequence,
            target_json: serde_json::to_string(&e.target).unwrap(),
            kind: e.kind.into(),
            columns: e.size.columns,
            rows: e.size.rows,
            bytes: e.bytes,
        }
    }
}
#[derive(uniffi::Object)]
pub struct NativeConnection {
    inner: Arc<Connection>,
}
#[uniffi::export]
impl NativeConnection {
    #[uniffi::constructor]
    pub fn new(
        host_json: String,
        credential_json: String,
        trusted_fingerprint: Option<String>,
    ) -> Result<Arc<Self>, CoreError> {
        let host: HostProfile = serde_json::from_str(&host_json).map_err(|_| invalid())?;
        host.validate()?;
        let credential: Credential =
            serde_json::from_str(&credential_json).map_err(|_| invalid())?;
        let _guard = runtime().enter();
        Ok(Arc::new(Self {
            inner: Connection::start(host, credential, trusted_fingerprint),
        }))
    }
    /// State fields plus `derived`, the core's own predicates for input and retry controls.
    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.inner.state().view()).unwrap()
    }
    /// Cheap change check: poll this and fetch `state_json` only when it differs.
    pub fn state_sequence(&self) -> u64 {
        self.inner.sequence()
    }
    pub fn target_json(&self) -> Option<String> {
        self.inner
            .target()
            .map(|t| serde_json::to_string(&t).unwrap())
    }
    /// Output identity belongs to a pane renderer, independently of input focus.
    pub fn accepts_output(&self, target_json: String) -> bool {
        let Ok(target) = serde_json::from_str::<dgl_core::flow::PaneTarget>(&target_json) else {
            return false;
        };
        self.inner.state().accepts_output(&target)
    }
    pub fn submit_input(&self, target_json: String, bytes: Vec<u8>) -> Result<u64, CoreError> {
        Ok(self.inner.input(
            serde_json::from_str(&target_json).map_err(|_| invalid())?,
            bytes,
        )?)
    }
    pub fn submit_action(&self, action_json: String) -> Result<u64, CoreError> {
        let action: Action = serde_json::from_str(&action_json).map_err(|_| invalid())?;
        Ok(self.inner.action(action)?)
    }
    pub fn set_title_updates_enabled(&self, enabled: bool) {
        self.inner.set_title_updates_enabled(enabled)
    }
    pub fn subscribe(&self) -> Result<u64, CoreError> {
        Ok(self.inner.output().subscribe()?)
    }
    pub fn unsubscribe(&self, subscription: u64) {
        self.inner.output().unsubscribe(subscription)
    }
    pub fn poll_output(&self, subscription: u64) -> Result<Option<NativeOutput>, CoreError> {
        Ok(self.inner.output().poll(subscription)?.map(Into::into))
    }
    pub fn acknowledge(&self, subscription: u64, sequence: u64) -> Result<(), CoreError> {
        Ok(self.inner.output().ack(subscription, sequence)?)
    }
    pub fn completions_json(&self) -> String {
        serde_json::to_string(&self.inner.completions()).unwrap()
    }
    pub fn cancel(&self, request: u64) -> Result<(), CoreError> {
        Ok(self.inner.cancel(request)?)
    }
    pub fn stop(&self) {
        self.inner.close()
    }
    #[uniffi::method(async_runtime = "tokio")]
    pub async fn shutdown(&self) {
        self.inner.close();
        self.inner.join().await;
    }
    pub fn buffered_bytes(&self) -> u64 {
        self.inner.output().metrics().0 as u64
    }
}
#[uniffi::export]
pub fn validate_settings(json: String) -> Result<String, CoreError> {
    let state = dgl_core::model::LocalState::decode(json.as_bytes())?;
    serde_json::to_string(&state).map_err(|_| invalid())
}

#[uniffi::export]
pub fn new_sync_document() -> Result<String, CoreError> {
    Ok(dgl_core::sync::Document::default().encode()?)
}

/// Shared with the desktop. JSON keeps the provider adapter independent of the
/// versioned journal schema; only the Rust allowlist can produce cloud content.
#[uniffi::export]
pub fn exchange_sync(
    cache: Option<String>,
    local: String,
    remote: String,
    resolutions: String,
) -> Result<String, CoreError> {
    let cache = cache
        .map(|v| dgl_core::sync::Cache::decode(v.as_bytes()))
        .transpose()?;
    let local = dgl_core::model::LocalState::decode(local.as_bytes())?;
    let resolutions: Vec<dgl_core::sync::Resolution> =
        serde_json::from_str(&resolutions).map_err(|_| invalid())?;
    let out = dgl_core::sync::exchange(cache, &local, remote.as_bytes(), &resolutions)?;
    Ok(
        serde_json::json!({"cache":out.cache.encode()?, "local":out.local,
        "document":out.document, "conflicts":out.conflicts})
        .to_string(),
    )
}

/// Mark only the settings successfully saved on this device as observed.
#[uniffi::export]
pub fn acknowledge_sync(cache: String, local: String) -> Result<String, CoreError> {
    let mut cache = dgl_core::sync::Cache::decode(cache.as_bytes())?;
    let local = dgl_core::model::LocalState::decode(local.as_bytes())?;
    cache.acknowledge(&local);
    Ok(cache.encode()?)
}

/// One per application, shared by all connection adapters (including hidden tabs).
#[derive(uniffi::Object, Default)]
pub struct NativeAgentInbox {
    inner: std::sync::Mutex<dgl_core::agent::Inbox>,
}
#[uniffi::export]
impl NativeAgentInbox {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn tick(
        &self,
        sources_json: String,
        visible_json: String,
        settings_json: String,
    ) -> Result<String, CoreError> {
        let sources: Vec<(String, dgl_core::agent::Snapshot)> =
            serde_json::from_str(&sources_json).map_err(|_| invalid())?;
        let visible: std::collections::HashSet<String> =
            serde_json::from_str(&visible_json).map_err(|_| invalid())?;
        let settings = serde_json::from_str(&settings_json).map_err(|_| invalid())?;
        let mut inbox = self.inner.lock().unwrap();
        let deliver = inbox.tick(
            sources
                .iter()
                .map(|(endpoint, snapshot)| (endpoint.as_str(), snapshot)),
            &visible,
            &settings,
            dgl_core::agent::now_ms(),
        );
        Ok(serde_json::json!({"recent": inbox.recent, "deliver": deliver}).to_string())
    }
    pub fn mark_read(&self, id: String) {
        self.inner.lock().unwrap().mark_read(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_encoder_matches_core_and_rejects_unsupported_compositions() {
        assert_eq!(
            encode_terminal_input("left".into(), None, true, true, true, true).unwrap(),
            b"\x1b[1;8D"
        );
        assert_eq!(
            encode_terminal_input("c".into(), Some("c".into()), true, false, false, false).unwrap(),
            b"\x03"
        );
        assert!(
            encode_terminal_input("text".into(), Some("한".into()), true, false, false, false)
                .is_err()
        );
    }
    #[test]
    fn title_visibility_is_transient_and_independent_of_subscriptions() {
        let c = NativeConnection::new(
            r#"{"id":"title-test","alias":"test","hostname":"127.0.0.1","port":1,"username":"test","credentialRef":"test"}"#.into(),
            r#"{"password":"","privateKey":"","passphrase":""}"#.into(), None).unwrap();
        let before: serde_json::Value = serde_json::from_str(&c.state_json()).unwrap();
        for enabled in [true, true, false, true, false] {
            c.set_title_updates_enabled(enabled);
        }
        // Visibility is a direct control signal: no queued actions or output consumer.
        assert_eq!(c.completions_json(), "[]");
        assert_eq!(c.inner.output().subscription(), 0);
        assert!(before.get("title_updates_enabled").is_none());
        // Bindings read the core's predicates instead of re-deriving them.
        let derived = &before["derived"];
        for key in [
            "observing_agents",
            "reconnecting",
            "retry_available",
            "retry_exhausted",
        ] {
            assert!(derived[key].is_boolean(), "{key}");
        }
        assert!(derived.get("retry_message").is_some());
        c.stop();
        runtime().block_on(c.inner.join());
    }
    #[test]
    fn native_key_round_trip_and_invalid_import() {
        let key = generate_key().unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert_eq!(
            public_key(key.private_key, String::new()).unwrap(),
            key.public_key
        );
        assert!(public_key("invalid key".into(), "secret".into()).is_err());
    }
}

/// Encodes semantic keyboard input, not already encoded onData/paste/mouse bytes.
#[uniffi::export]
pub fn encode_terminal_input(
    key: String,
    text: Option<String>,
    control: bool,
    shift: bool,
    alt: bool,
    application_cursor: bool,
) -> Result<Vec<u8>, CoreError> {
    dgl_core::input::encode_key(
        &key,
        text.as_deref(),
        dgl_core::input::Modifiers {
            control,
            shift,
            alt,
        },
        application_cursor,
    )
    .ok_or_else(invalid)
}
