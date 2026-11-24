use super::{Entry, FilesError, Result, child_path, list};
use russh_sftp::{
    client::{SftpSession, error::Error as SftpError},
    protocol::{FileAttributes, FileType, OpenFlags, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path, sync::Mutex};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{oneshot, watch},
};

/// IDs are opaque. Android supplies document URIs, the desktop supplies absolute paths.
pub trait LocalTree: Send + Sync {
    fn entry(&self, id: &str) -> io::Result<Entry>;
    fn children(&self, id: &str) -> io::Result<Vec<String>>;
    fn find(&self, parent: &str, name: &str) -> io::Result<Option<String>>;
    fn create(&self, parent: &str, name: &str, directory: bool) -> io::Result<String>;
    fn open(&self, id: &str, write: bool) -> io::Result<fs::File>;
    fn publish(
        &self,
        temp: &str,
        parent: &str,
        name: &str,
        existing: Option<&str>,
    ) -> io::Result<()>;
    fn discard(&self, temp: &str) -> io::Result<()>;
}

pub struct LocalPaths;
fn path_id(path: &Path) -> io::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("Invalid UTF-8 path"))
}
fn local_child(parent: &str, name: &str) -> io::Result<String> {
    if Path::new(name).file_name().and_then(|s| s.to_str()) != Some(name) {
        return Err(io::Error::other("Invalid local file name"));
    }
    #[cfg(windows)]
    {
        let stem = name
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end()
            .to_ascii_uppercase();
        if name.contains([':', '<', '>', '"', '|', '?', '*'])
            || name.ends_with([' ', '.'])
            || matches!(
                stem.as_str(),
                "CON" | "NUL" | "AUX" | "PRN" | "CONIN$" | "CONOUT$"
            )
            || (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.chars().count() == 4
                && stem.ends_with(|c: char| c.is_ascii_digit() || "¹²³".contains(c))
        {
            return Err(io::Error::other("Invalid Windows file name"));
        }
    }
    path_id(&Path::new(parent).join(name))
}
impl LocalTree for LocalPaths {
    fn entry(&self, id: &str) -> io::Result<Entry> {
        let m = fs::symlink_metadata(id)?;
        let name = Path::new(id)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| io::Error::other("Invalid file name"))?;
        Ok(Entry {
            name: name.to_owned(),
            directory: m.is_dir(),
            regular: m.is_file(),
            size: Some(m.len()),
        })
    }
    fn children(&self, id: &str) -> io::Result<Vec<String>> {
        fs::read_dir(id)?
            .map(|item| path_id(&item?.path()))
            .collect()
    }
    fn find(&self, parent: &str, name: &str) -> io::Result<Option<String>> {
        let id = local_child(parent, name)?;
        match fs::symlink_metadata(&id) {
            Ok(_) => Ok(Some(id)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn create(&self, parent: &str, name: &str, directory: bool) -> io::Result<String> {
        let id = local_child(parent, name)?;
        if directory {
            fs::create_dir(&id)?
        } else {
            let mut options = fs::OpenOptions::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.write(true).create_new(true).open(&id)?;
        }
        Ok(id)
    }
    fn open(&self, id: &str, write: bool) -> io::Result<fs::File> {
        fs::OpenOptions::new().read(!write).write(write).open(id)
    }
    fn publish(
        &self,
        temp: &str,
        parent: &str,
        name: &str,
        existing: Option<&str>,
    ) -> io::Result<()> {
        let dest = local_child(parent, name)?;
        let temp = tempfile::TempPath::try_from_path(temp)?;
        if existing.is_some() {
            temp.persist(dest)
        } else {
            temp.persist_noclobber(dest)
        }
        .map_err(|e| e.error)
    }
    fn discard(&self, temp: &str) -> io::Result<()> {
        fs::remove_file(temp)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyRequest {
    pub upload: bool,
    pub sources: Vec<String>,
    pub destination: String,
    /// A file promise receiver may assign a new name to a single dropped item.
    pub name: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub item: String,
    pub bytes: u64,
    pub size: Option<u64>,
    pub completed: u64,
    pub conflict: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}
#[derive(Default)]
struct State {
    progress: Progress,
    overwrite: Option<oneshot::Sender<()>>,
}
pub struct Transfer {
    state: Mutex<State>,
    cancel: watch::Sender<bool>,
}
impl Default for Transfer {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            cancel: watch::channel(false).0,
        }
    }
}
impl Transfer {
    pub fn progress(&self) -> Progress {
        self.state.lock().unwrap().progress.clone()
    }
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    pub fn overwrite(&self, expected: &str) {
        let mut state = self.state.lock().unwrap();
        if state.progress.conflict.as_deref() == Some(expected)
            && let Some(answer) = state.overwrite.take()
        {
            state.progress.conflict = None;
            let _ = answer.send(());
        }
    }
    pub fn finish(&self, result: &Result<()>) {
        let mut state = self.state.lock().unwrap();
        state.progress.done = true;
        state.progress.error = result.as_ref().err().map(ToString::to_string);
        state.progress.conflict = None;
        state.overwrite = None;
    }
    async fn confirm(&self, path: &str, files: &super::Files) -> Result<()> {
        let (answer, rx) = oneshot::channel();
        {
            let mut state = self.state.lock().unwrap();
            state.progress.conflict = Some(path.to_owned());
            state.overwrite = Some(answer);
        }
        let mut cancel = self.cancel.subscribe();
        tokio::select! {
            biased;
            _ = cancel.wait_for(|b| *b) => return Err(FilesError::Cancelled),
            _ = async {
                while !files.disconnected() { tokio::time::sleep(std::time::Duration::from_millis(250)).await; }
            } => return Err(crate::Error::Network.into()),
            _ = rx => {}
        }
        self.state.lock().unwrap().progress.conflict = None;
        Ok(())
    }
    fn check(&self) -> Result<()> {
        if *self.cancel.borrow() {
            Err(FilesError::Cancelled)
        } else {
            Ok(())
        }
    }
    async fn stream(
        &self,
        mut reader: impl AsyncRead + Unpin,
        mut writer: impl AsyncWrite + Unpin,
        size: Option<u64>,
    ) -> Result<()> {
        let mut buf = vec![0; 64 * 1024];
        let mut cancel = self.cancel.subscribe();
        tokio::select! {
            biased;
            _ = cancel.wait_for(|b| *b) => Err(FilesError::Cancelled),
            result = async {
                let mut copied = 0;
                loop {
                    let n = reader.read(&mut buf).await?;
                    if n == 0 { break }
                    writer.write_all(&buf[..n]).await?;
                    copied += n as u64;
                    self.state.lock().unwrap().progress.bytes += n as u64;
                }
                if size.is_some_and(|size| size != copied) {
                    return Err(io::Error::other("Source size changed during transfer").into());
                }
                writer.flush().await?;
                Ok(())
            } => result,
        }
    }
    /// Snapshot the tree before creating any destination entries. This also bounds a
    /// copy between two views of the same filesystem when the destination is inside the source.
    async fn snapshot(
        &self,
        remote: &SftpSession,
        local: &dyn LocalTree,
        req: &CopyRequest,
    ) -> Result<Vec<SourceItem>> {
        let mut items = Vec::new();
        let mut pending: Vec<_> = req
            .sources
            .iter()
            .rev()
            .map(|id| (id.clone(), None))
            .collect();
        let mut seen_dirs = std::collections::HashSet::new();
        let mut total = Some(0_u64);
        while let Some((id, parent)) = pending.pop() {
            self.check()?;
            let entry = if req.upload {
                local.entry(&id)?
            } else {
                let name = id.rsplit('/').next().unwrap_or("");
                Entry::remote(name.into(), &remote.symlink_metadata(&id).await?)
            };
            let name = if parent.is_none() {
                req.name.clone().unwrap_or(entry.name)
            } else {
                entry.name
            };
            child_path("", &name)?;
            if entry.directory {
                if !seen_dirs.insert(id.clone()) {
                    return Err(FilesError::Type(id));
                }
                let children = if req.upload {
                    local.children(&id)?
                } else {
                    list(remote, &id)
                        .await?
                        .entries
                        .iter()
                        .map(|e| child_path(&id, &e.name))
                        .collect::<Result<_>>()?
                };
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| (child, Some(items.len()))),
                );
            } else if entry.regular {
                total = total.zip(entry.size).and_then(|(a, b)| a.checked_add(b));
            } else {
                return Err(FilesError::Type(id));
            }
            items.push(SourceItem {
                id,
                parent,
                name,
                directory: entry.directory,
                size: entry.size,
            });
        }
        self.state.lock().unwrap().progress.size = total;
        Ok(items)
    }
    pub(super) async fn copy(
        &self,
        files: &super::Files,
        local: &dyn LocalTree,
        req: &CopyRequest,
    ) -> Result<()> {
        let items = self.snapshot(&files.remote, local, req).await?;
        let mut folders = std::collections::HashMap::<usize, String>::new();
        for (index, item) in items.into_iter().enumerate() {
            self.check()?;
            let parent = item
                .parent
                .map(|i| folders[&i].as_str())
                .unwrap_or(&req.destination);
            self.state.lock().unwrap().progress.item = item.name.clone();
            let dir = if req.upload {
                self.upload(files, local, parent, &item).await?
            } else {
                self.download(files, local, parent, &item).await?
            };
            if let Some(dir) = dir {
                folders.insert(index, dir);
            }
            self.state.lock().unwrap().progress.completed += 1;
        }
        Ok(())
    }
    async fn upload(
        &self,
        files: &super::Files,
        local: &dyn LocalTree,
        parent: &str,
        item: &SourceItem,
    ) -> Result<Option<String>> {
        let remote = &files.remote;
        let path = child_path(parent, &item.name)?;
        let existing = match remote.symlink_metadata(&path).await {
            Ok(attrs) => Some(attrs.file_type()),
            Err(SftpError::Status(s)) if s.status_code == StatusCode::NoSuchFile => None,
            Err(e) => return Err(e.into()),
        };
        if item.directory {
            match existing {
                Some(FileType::Dir) => {}
                Some(_) => return Err(FilesError::Type(path)),
                None => remote.create_dir(&path).await?,
            }
            return Ok(Some(path));
        }
        if let Some(kind) = existing {
            if !kind.is_file() {
                return Err(FilesError::Type(path));
            }
            self.confirm(&path, files).await?;
        }
        let input = tokio::fs::File::from_std(local.open(&item.id, false)?);
        let temp = child_path(parent, &temp_name())?;
        let mut attrs = FileAttributes::empty();
        attrs.permissions = Some(0o600);
        let mut file = remote
            .open_with_flags_and_attributes(
                &temp,
                OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
                attrs,
            )
            .await?;
        let result: Result<()> = async {
            self.stream(input, &mut file, item.size).await?;
            file.close().await?;
            self.check()?;
            publish_remote(remote, &temp, parent, &path, existing.is_some()).await
        }
        .await;
        if result.is_err() {
            let _ = remote.remove_file(&temp).await;
        }
        result.map(|_| None)
    }
    async fn download(
        &self,
        files: &super::Files,
        local: &dyn LocalTree,
        parent: &str,
        item: &SourceItem,
    ) -> Result<Option<String>> {
        let existing = local.find(parent, &item.name)?;
        if item.directory {
            let dir = match existing {
                Some(id) if local.entry(&id)?.directory => id,
                Some(id) => return Err(FilesError::Type(id)),
                None => local.create(parent, &item.name, true)?,
            };
            return Ok(Some(dir));
        }
        if let Some(id) = &existing {
            if !local.entry(id)?.regular {
                return Err(FilesError::Type(id.clone()));
            }
            self.confirm(&item.id, files).await?;
        }
        let mut file = files.remote.open(&item.id).await?;
        let temp = local.create(parent, &temp_name(), false)?;
        let result: Result<()> = async {
            let writer = tokio::fs::File::from_std(local.open(&temp, true)?);
            self.stream(&mut file, writer, item.size).await?;
            file.close().await?;
            self.check()?;
            local.publish(&temp, parent, &item.name, existing.as_deref())?;
            Ok(())
        }
        .await;
        if result.is_err() {
            let _ = local.discard(&temp);
        }
        result.map(|_| None)
    }
}

struct SourceItem {
    id: String,
    parent: Option<usize>,
    name: String,
    directory: bool,
    size: Option<u64>,
}

async fn publish_remote(
    remote: &SftpSession,
    temp: &str,
    parent: &str,
    path: &str,
    overwrite: bool,
) -> Result<()> {
    if !overwrite {
        return Ok(remote.rename(temp, path).await?);
    }
    if remote.posix_rename(temp, path).await? {
        return Ok(());
    }
    // Baseline SFTP v3 has no atomic replacement. Retain the old file until the
    // replacement has been acknowledged and report its backup path on a lost reply.
    let backup = child_path(parent, &temp_name())?;
    remote.rename(path, &backup).await?;
    if let Err(e) = remote.rename(temp, path).await {
        remote
            .rename(&backup, path)
            .await
            .map_err(|restore| io::Error::other(format!("{e}; restore {backup}: {restore}")))?;
        return Err(e.into());
    }
    Ok(remote.remove_file(&backup).await?)
}
fn temp_name() -> String {
    format!(".dgl-{:032x}.part", rand::random::<u128>())
}
