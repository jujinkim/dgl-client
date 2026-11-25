//! Each browser owns an authenticated SSH connection, independent of terminal lifetime.
use crate::{
    command,
    model::{Credential, HostProfile},
    ssh,
};
use russh_sftp::{client::SftpSession, protocol::FileAttributes};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod transfer;
pub use transfer::*;
#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("{0}")]
    Connection(#[from] crate::Error),
    #[error("{0}")]
    Remote(#[from] russh_sftp::client::error::Error),
    #[error("{0}")]
    Local(#[from] std::io::Error),
    #[error("Invalid file name: {0}")]
    Name(String),
    #[error("Unsupported file type: {0}")]
    Type(String),
    #[error("Only regular files can be transferred. Unsupported source: {0}")]
    SourceType(String),
    #[error("{0}")]
    PromiseBatch(String),
    #[error("Cancelled")]
    Cancelled,
}
pub type Result<T> = std::result::Result<T, FilesError>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub directory: bool,
    pub regular: bool,
    pub size: Option<u64>,
}
impl Entry {
    fn remote(name: String, attrs: &FileAttributes) -> Self {
        let kind = attrs.file_type();
        Self {
            name,
            directory: kind.is_dir(),
            regular: kind.is_file(),
            size: attrs.size,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
    pub fallback: Option<String>,
}

pub struct Files {
    ssh: ssh::Ssh,
    remote: SftpSession,
}

pub fn child_path(parent: &str, name: &str) -> Result<String> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\0']) {
        return Err(FilesError::Name(name.into()));
    }
    Ok(format!("{}/{name}", parent.trim_end_matches('/')))
}

impl Files {
    pub async fn connect(
        host: &HostProfile,
        credential: &Credential,
        trusted: Option<String>,
        observed: Arc<Mutex<Option<String>>>,
    ) -> Result<Self> {
        let ssh = ssh::connect(host, credential, trusted, observed).await?;
        let remote = tokio::time::timeout(Duration::from_secs(15), async {
            let channel = ssh
                .handle
                .channel_open_session()
                .await
                .map_err(|_| crate::Error::Network)?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|_| crate::Error::Network)?;
            Ok::<_, FilesError>(SftpSession::new(channel.into_stream()).await?)
        })
        .await
        .map_err(|_| crate::Error::Network)??;
        Ok(Self { ssh, remote })
    }

    pub async fn list(&self, path: &str) -> Result<Listing> {
        list(&self.remote, path).await
    }

    pub async fn initial(&mut self, session: Option<&str>, pane: Option<&str>) -> Result<Listing> {
        let Some(session) = session else {
            return self.list(".").await;
        };
        let path = async {
            let target = command::target(session, '$')?;
            let lookup = if let Some(pane) = pane {
                command::target(pane, '%')?;
                // Restrict the pane lookup to this session, including linked windows.
                format!("list-panes -s -t {target} -f '#{{==:#{{pane_id}},{pane}}}' -F '#{{pane_current_path}}'")
            } else {
                format!("display-message -p -t {target} '#{{pane_current_path}}'")
            };
            if !self.ssh.discover().await? {
                return Err(crate::Error::MissingSession);
            }
            let tmux = command::quote(
                self.ssh
                    .tmux
                    .as_deref()
                    .ok_or(crate::Error::MissingSession)?,
            )?;
            let (code, path) = self.ssh.execute(&format!("{tmux} {lookup}")).await?;
            let path = path.strip_suffix(b"\n").unwrap_or(&path);
            let path = std::str::from_utf8(path).map_err(|_| crate::Error::Protocol)?;
            if code != 0 || !path.starts_with('/') {
                return Err(crate::Error::MissingSession);
            }
            Ok(path.to_owned())
        }
        .await;
        initial_listing(&self.remote, path).await
    }

    pub async fn copy(
        &self,
        local: &dyn LocalTree,
        request: &CopyRequest,
        job: &Transfer,
    ) -> Result<()> {
        job.copy(self, local, request).await
    }
    pub fn disconnected(&self) -> bool {
        self.ssh.handle.is_closed()
    }
}

async fn initial_listing(remote: &SftpSession, path: crate::Result<String>) -> Result<Listing> {
    let failed = match path {
        Ok(path) => match list(remote, &path).await {
            Ok(listing) => return Ok(listing),
            Err(e) => format!("{path} ({e})"),
        },
        Err(e) => e.to_string(),
    };
    let mut listing = list(remote, ".").await?;
    listing.fallback = Some(failed);
    Ok(listing)
}

async fn list(remote: &SftpSession, path: &str) -> Result<Listing> {
    let path = remote
        .canonicalize(path)
        .await?
        .into_string()
        .map_err(|p| FilesError::Name(p.to_string()))?;
    if !path.starts_with('/') {
        return Err(FilesError::Name(path));
    }
    let mut entries = Vec::new();
    for item in remote.read_dir(&path).await? {
        let name = item
            .file_name()
            .into_string()
            .map_err(|p| FilesError::Name(p.to_string()))?;
        child_path(&path, &name)?;
        entries.push(Entry::remote(name, &item.metadata()));
    }
    entries.sort_by(|a, b| b.directory.cmp(&a.directory).then(a.name.cmp(&b.name)));
    Ok(Listing {
        path,
        entries,
        fallback: None,
    })
}

/// Connection parameters are captured when the user opens the file tab. An explicit
/// browse or retry can reconnect, but a failed transfer never replays itself.
pub struct Browser {
    host: HostProfile,
    credential: Credential,
    trusted: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
    session: Option<String>,
    pane: Option<String>,
    connection: tokio::sync::Mutex<Option<Arc<Files>>>,
}

/// Native file promises assign a destination to each item separately. All items
/// from one drag must pass the same preflight before any promise can write.
pub struct DownloadBatch {
    sources: Vec<String>,
    preflight: tokio::sync::OnceCell<std::result::Result<(), String>>,
}
impl DownloadBatch {
    pub fn new(sources: Vec<String>) -> Self {
        Self {
            sources,
            preflight: Default::default(),
        }
    }

    pub fn retry(&self) -> Self {
        Self::new(self.sources.clone())
    }
}

impl Browser {
    pub fn new(
        host: HostProfile,
        credential: Credential,
        trusted: Option<String>,
        session: Option<String>,
        pane: Option<String>,
    ) -> Self {
        Self {
            host,
            credential,
            trusted,
            observed: Default::default(),
            session,
            pane,
            connection: Default::default(),
        }
    }
    pub fn observed(&self) -> Option<String> {
        self.observed.lock().unwrap().clone()
    }
    async fn connect(&self) -> Result<Files> {
        Files::connect(
            &self.host,
            &self.credential,
            self.trusted.clone(),
            self.observed.clone(),
        )
        .await
    }
    async fn connection(&self) -> Result<Arc<Files>> {
        let mut slot = self.connection.lock().await;
        if slot.as_ref().is_none_or(|files| files.disconnected()) {
            *slot = Some(Arc::new(self.connect().await?));
        }
        Ok(slot.as_ref().unwrap().clone())
    }
    async fn forget_connection(&self, files: &Arc<Files>) {
        let mut slot = self.connection.lock().await;
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, files))
        {
            *slot = None;
        }
    }
    pub async fn browse(&self, path: Option<&str>) -> Result<Listing> {
        if let Some(path) = path {
            let files = self.connection().await?;
            let result = files.list(path).await;
            if matches!(&result, Err(FilesError::Remote(error))
                if !matches!(error, russh_sftp::client::error::Error::Status(_)))
            {
                self.forget_connection(&files).await;
            }
            return result;
        }
        let mut slot = self.connection.lock().await;
        let mut files = self.connect().await?;
        let listing = files
            .initial(self.session.as_deref(), self.pane.as_deref())
            .await?;
        *slot = Some(Arc::new(files));
        Ok(listing)
    }
    pub fn start(
        self: &Arc<Self>,
        local: Arc<dyn LocalTree>,
        request: CopyRequest,
    ) -> Arc<Transfer> {
        self.start_job(local, request, None)
    }
    pub fn start_file_promise(
        self: &Arc<Self>,
        local: Arc<dyn LocalTree>,
        request: CopyRequest,
        batch: Arc<DownloadBatch>,
    ) -> Arc<Transfer> {
        self.start_job(local, request, Some(batch))
    }
    fn start_job(
        self: &Arc<Self>,
        local: Arc<dyn LocalTree>,
        request: CopyRequest,
        batch: Option<Arc<DownloadBatch>>,
    ) -> Arc<Transfer> {
        let job = Arc::new(Transfer::default());
        let worker = job.clone();
        let browser = self.clone();
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let result = runtime.block_on(async {
                worker.check()?;
                let files = browser.connection().await?;
                if let Some(batch) = batch {
                    let validated = batch
                        .preflight
                        .get_or_init(|| async {
                            for source in &batch.sources {
                                remote_source(&files.remote, source)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            }
                            Ok(())
                        })
                        .await;
                    if let Err(error) = validated {
                        return Err(FilesError::PromiseBatch(error.clone()));
                    }
                }
                let result = files.copy(local.as_ref(), &request, &worker).await;
                if matches!(
                    &result,
                    Err(FilesError::Connection(_) | FilesError::Remote(_))
                ) {
                    browser.forget_connection(&files).await;
                }
                result
            });
            worker.finish(&result);
        });
        job
    }
}
