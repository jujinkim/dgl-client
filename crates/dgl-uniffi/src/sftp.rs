use super::{CoreError, invalid, runtime};
use dgl_core::sftp::{Browser, CopyRequest, Entry, LocalTree, Transfer};
use std::{io, sync::Arc};

/// Document IDs belong to the system provider. open_fd transfers ownership of a
/// ParcelFileDescriptor.detachFd() to Rust, which closes it after streaming.
#[uniffi::export(callback_interface)]
pub trait Documents: Send + Sync {
    fn entry_json(&self, id: String) -> Result<String, CoreError>;
    fn find(&self, parent: String, name: String) -> Result<Option<String>, CoreError>;
    fn create_file(&self, parent: String, name: String) -> Result<String, CoreError>;
    fn open_fd(&self, id: String, write: bool) -> Result<i32, CoreError>;
    fn publish(
        &self,
        temp: String,
        parent: String,
        name: String,
        existing: Option<String>,
    ) -> Result<(), CoreError>;
    fn discard(&self, temp: String) -> Result<(), CoreError>;
}
impl From<uniffi::UnexpectedUniFFICallbackError> for CoreError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failure {
            code: e.to_string(),
        }
    }
}
struct DocumentTree(Box<dyn Documents>);
impl LocalTree for DocumentTree {
    fn entry(&self, id: &str) -> io::Result<Entry> {
        Ok(serde_json::from_str(
            &self.0.entry_json(id.into()).map_err(io::Error::other)?,
        )?)
    }
    fn find(&self, parent: &str, name: &str) -> io::Result<Option<String>> {
        self.0
            .find(parent.into(), name.into())
            .map_err(io::Error::other)
    }
    fn create_file(&self, parent: &str, name: &str) -> io::Result<String> {
        self.0
            .create_file(parent.into(), name.into())
            .map_err(io::Error::other)
    }
    fn open(&self, id: &str, write: bool) -> io::Result<std::fs::File> {
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;
            let fd = self.0.open_fd(id.into(), write).map_err(io::Error::other)?;
            if fd < 0 {
                return Err(io::Error::other("Invalid document descriptor"));
            }
            // The Documents contract transfers an owned, open descriptor exactly once.
            Ok(unsafe { std::fs::File::from_raw_fd(fd) })
        }
        #[cfg(not(unix))]
        {
            let _ = (id, write);
            Err(io::Error::other("Android document provider required"))
        }
    }
    fn publish(
        &self,
        temp: &str,
        parent: &str,
        name: &str,
        existing: Option<&str>,
    ) -> io::Result<()> {
        self.0
            .publish(
                temp.into(),
                parent.into(),
                name.into(),
                existing.map(str::to_owned),
            )
            .map_err(io::Error::other)
    }
    fn discard(&self, temp: &str) -> io::Result<()> {
        self.0.discard(temp.into()).map_err(io::Error::other)
    }
}

#[derive(uniffi::Object)]
pub struct NativeFiles {
    browser: Arc<Browser>,
}
#[uniffi::export]
impl NativeFiles {
    #[uniffi::constructor]
    pub fn new(
        host_json: String,
        credential_json: String,
        trusted: Option<String>,
        session: Option<String>,
        pane: Option<String>,
    ) -> Result<Arc<Self>, CoreError> {
        let host = serde_json::from_str(&host_json).map_err(|_| invalid())?;
        let cred = serde_json::from_str(&credential_json).map_err(|_| invalid())?;
        Ok(Arc::new(Self {
            browser: Arc::new(Browser::new(host, cred, trusted, session, pane)),
        }))
    }
    pub fn fingerprint(&self) -> Option<String> {
        self.browser.observed()
    }
    /// Kotlin invokes blocking provider/SSH calls on Dispatchers.IO.
    pub fn list(&self, path: Option<String>) -> Result<String, CoreError> {
        let list = runtime()
            .block_on(self.browser.browse(path.as_deref()))
            .map_err(|e| CoreError::Failure {
                code: e.to_string(),
            })?;
        Ok(serde_json::to_string(&list).unwrap())
    }
    pub fn transfer(
        &self,
        documents: Box<dyn Documents>,
        upload: bool,
        sources: Vec<String>,
        destination: String,
    ) -> Arc<NativeFileTransfer> {
        let _runtime = runtime().enter();
        Arc::new(NativeFileTransfer {
            job: self.browser.start(
                Arc::new(DocumentTree(documents)),
                CopyRequest {
                    upload,
                    sources,
                    destination,
                    name: None,
                },
            ),
        })
    }
}
#[derive(uniffi::Object)]
pub struct NativeFileTransfer {
    job: Arc<Transfer>,
}
#[uniffi::export]
impl NativeFileTransfer {
    pub fn progress(&self) -> String {
        serde_json::to_string(&self.job.progress()).unwrap()
    }
    pub fn cancel(&self) {
        self.job.cancel();
    }
    pub fn overwrite(&self, expected: String) {
        self.job.overwrite(&expected);
    }
}
