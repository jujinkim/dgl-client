//! Kotlin owns this object explicitly; shutdown joins the actor before wrapper close().
use dgl_core::{
    connection::{Action, Connection},
    flow::OutputEvent,
    model::{Credential, HostProfile},
};
use std::sync::{Arc, OnceLock};
uniffi::setup_scaffolding!();
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
#[derive(uniffi::Record)]
pub struct NativeOutput {
    pub subscription: u64,
    pub sequence: u64,
    pub target_json: String,
    pub kind: String,
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
            kind: format!("{:?}", e.kind),
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
    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.inner.state()).unwrap()
    }
    pub fn target_json(&self) -> Option<String> {
        self.inner
            .target()
            .map(|t| serde_json::to_string(&t).unwrap())
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

#[cfg(test)]
mod tests {
    use super::*;

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
