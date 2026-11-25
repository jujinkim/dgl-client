use super::*;
use russh_sftp::protocol::{
    Attrs, Data, File, Handle, Name, OpenFlags, SftpHandle, SftpString, Status, StatusCode,
};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

/// A subsystem-only peer with a home directory and one accessible pane folder.
struct Directories {
    reading: Arc<tokio::sync::Notify>,
}
impl russh_sftp::server::Handler for Directories {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn realpath(
        &mut self,
        id: u32,
        path: SftpString,
    ) -> std::result::Result<Name, StatusCode> {
        let path = match path.as_bytes() {
            b"." => "/home/test",
            b"/pane" => "/pane",
            _ => return Err(StatusCode::PermissionDenied),
        };
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }
    async fn lstat(
        &mut self,
        id: u32,
        _: SftpString,
    ) -> std::result::Result<russh_sftp::protocol::Attrs, StatusCode> {
        Ok(russh_sftp::protocol::Attrs {
            id,
            attrs: FileAttributes {
                permissions: Some(0o100600),
                size: Some(100_000),
                ..FileAttributes::empty()
            },
        })
    }
    async fn open(
        &mut self,
        id: u32,
        path: SftpString,
        _: russh_sftp::protocol::OpenFlags,
        _: FileAttributes,
    ) -> std::result::Result<Handle, StatusCode> {
        Ok(Handle {
            id,
            handle: path.into(),
        })
    }
    async fn read(
        &mut self,
        id: u32,
        _: SftpHandle,
        offset: u64,
        len: u32,
    ) -> std::result::Result<russh_sftp::protocol::Data, StatusCode> {
        if offset > 0 {
            self.reading.notify_one();
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        Ok(russh_sftp::protocol::Data {
            id,
            data: vec![0; len as usize],
        })
    }
    async fn opendir(
        &mut self,
        id: u32,
        path: SftpString,
    ) -> std::result::Result<Handle, StatusCode> {
        Ok(Handle {
            id,
            handle: path.into(),
        })
    }
    async fn readdir(&mut self, _: u32, _: SftpHandle) -> std::result::Result<Name, StatusCode> {
        Err(StatusCode::Eof)
    }
    async fn close(&mut self, id: u32, _: SftpHandle) -> std::result::Result<Status, StatusCode> {
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}
#[tokio::test]
async fn pane_path_and_default_folder_explain_both_fallbacks() {
    let (client, server) = tokio::io::duplex(4096);
    tokio::spawn(russh_sftp::server::run(
        server,
        Directories {
            reading: Default::default(),
        },
    ));
    let remote = SftpSession::new(client).await.unwrap();
    let pane = initial_listing(&remote, Ok("/pane".into())).await.unwrap();
    assert_eq!(pane.path, "/pane");
    assert!(pane.fallback.is_none());
    for path in [Ok("/denied".into()), Err(crate::Error::MissingSession)] {
        let list = initial_listing(&remote, path).await.unwrap();
        assert_eq!(list.path, "/home/test");
        assert!(list.fallback.is_some());
    }
}
#[test]
fn names_cannot_escape_the_destination_or_replace_a_new_arrival() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_str().unwrap();
    for bad in ["..", ".", "/absolute", "a/b", ""] {
        assert!(child_path(dir, bad).is_err());
        assert!(LocalPaths.create_file(dir, bad).is_err());
    }
    let part = LocalPaths.create_file(dir, "part").unwrap();
    let existing = LocalPaths.create_file(dir, "result").unwrap();
    fs::write(&existing, b"keep").unwrap();
    assert!(LocalPaths.publish(&part, dir, "result", None).is_err());
    assert_eq!(fs::read(existing).unwrap(), b"keep");
}

struct SubsystemOnly<H> {
    channel: Option<russh::Channel<russh::server::Msg>>,
    handler: Option<H>,
}
impl<H: russh_sftp::server::Handler + Send + 'static> russh::server::Handler for SubsystemOnly<H> {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        _: &str,
        _: &str,
    ) -> std::result::Result<russh::server::Auth, Self::Error> {
        Ok(russh::server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<russh::server::Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut russh::server::Session,
    ) -> std::result::Result<(), Self::Error> {
        self.channel = Some(channel);
        reply.accept().await;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: russh::ChannelId,
        name: &str,
        session: &mut russh::server::Session,
    ) -> std::result::Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        session.channel_success(id)?;
        russh_sftp::server::run(
            self.channel.take().unwrap().into_stream(),
            self.handler.take().unwrap(),
        )
        .await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        _: russh::ChannelId,
        _: &[u8],
        _: &mut russh::server::Session,
    ) -> std::result::Result<(), Self::Error> {
        panic!("host file tabs must not start tmux or a shell")
    }
}
async fn subsystem_browser(
    handler: impl russh_sftp::server::Handler + Send + 'static,
) -> (
    Arc<Browser>,
    tokio::sync::oneshot::Receiver<std::net::TcpStream>,
    tokio::task::JoinHandle<()>,
) {
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let fingerprint = key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let host = HostProfile {
        id: "test".into(),
        alias: "test".into(),
        hostname: "127.0.0.1".into(),
        port: listener.local_addr().unwrap().port(),
        username: "test".into(),
        auth: Default::default(),
        credential_ref: "test".into(),
        favorite: false,
        last_connected: 0,
        last_session: None,
        last_pane: None,
    };
    let peer = SubsystemOnly {
        channel: None,
        handler: Some(handler),
    };
    let (socket_tx, socket_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let stream = listener.accept().await.unwrap().0.into_std().unwrap();
        socket_tx.send(stream.try_clone().unwrap()).unwrap();
        let config = russh::server::Config {
            keys: vec![key],
            ..Default::default()
        };
        let session = russh::server::run_stream(
            Arc::new(config),
            tokio::net::TcpStream::from_std(stream).unwrap(),
            peer,
        )
        .await
        .unwrap();
        let _ = session.await;
    });
    let browser = Arc::new(Browser::new(
        host,
        Credential::default(),
        Some(fingerprint),
        None,
        None,
    ));
    (browser, socket_rx, server)
}

#[tokio::test]
async fn host_without_tmux_and_disconnect_during_download() {
    let reading = Arc::new(tokio::sync::Notify::new());
    let (browser, socket_rx, server) = subsystem_browser(Directories {
        reading: reading.clone(),
    })
    .await;
    assert_eq!(browser.browse(None).await.unwrap().path, "/home/test");
    let output = tempfile::tempdir().unwrap();
    let job = browser.start(
        Arc::new(LocalPaths),
        CopyRequest {
            upload: false,
            sources: vec!["/file".into()],
            destination: output.path().display().to_string(),
            name: None,
        },
    );
    tokio::time::timeout(Duration::from_secs(10), reading.notified())
        .await
        .unwrap();
    socket_rx
        .await
        .unwrap()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !job.progress().done {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(job.progress().error.is_some());
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    server.await.unwrap();
}

struct FileFixture(PathBuf);

impl FileFixture {
    fn path(&self, path: &[u8]) -> std::result::Result<PathBuf, StatusCode> {
        let path = std::str::from_utf8(path).map_err(|_| StatusCode::Failure)?;
        Ok(self.0.join(path.trim_start_matches('/')))
    }
}

fn fixture_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}

fn fixture_error(e: std::io::Error) -> StatusCode {
    if e.kind() == std::io::ErrorKind::NotFound {
        StatusCode::NoSuchFile
    } else {
        StatusCode::Failure
    }
}

impl russh_sftp::server::Handler for FileFixture {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn lstat(&mut self, id: u32, path: SftpString) -> std::result::Result<Attrs, StatusCode> {
        let mut attrs = FileAttributes::empty();
        if path.as_bytes() == b"/link" {
            attrs.permissions = Some(0o120777);
        } else {
            let meta = fs::metadata(self.path(path.as_bytes())?).map_err(fixture_error)?;
            attrs.permissions = Some(if meta.is_dir() { 0o040700 } else { 0o100600 });
            attrs.size = Some(meta.len());
        }
        Ok(Attrs { id, attrs })
    }

    async fn open(
        &mut self,
        id: u32,
        path: SftpString,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> std::result::Result<Handle, StatusCode> {
        if flags.contains(OpenFlags::CREATE) {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.path(path.as_bytes())?)
                .map_err(fixture_error)?;
        }
        Ok(Handle {
            id,
            handle: path.into(),
        })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: SftpHandle,
        offset: u64,
        len: u32,
    ) -> std::result::Result<Data, StatusCode> {
        let mut file = fs::File::open(self.path(handle.as_bytes())?).map_err(fixture_error)?;
        file.seek(SeekFrom::Start(offset)).map_err(fixture_error)?;
        let mut data = Vec::new();
        file.take(u64::from(len))
            .read_to_end(&mut data)
            .map_err(fixture_error)?;
        if data.is_empty() {
            Err(StatusCode::Eof)
        } else {
            Ok(Data { id, data })
        }
    }

    async fn write(
        &mut self,
        id: u32,
        handle: SftpHandle,
        offset: u64,
        data: Vec<u8>,
    ) -> std::result::Result<Status, StatusCode> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(self.path(handle.as_bytes())?)
            .map_err(fixture_error)?;
        file.seek(SeekFrom::Start(offset)).map_err(fixture_error)?;
        file.write_all(&data).map_err(fixture_error)?;
        Ok(fixture_status(id))
    }

    async fn close(&mut self, id: u32, _: SftpHandle) -> std::result::Result<Status, StatusCode> {
        Ok(fixture_status(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        old: SftpString,
        new: SftpString,
    ) -> std::result::Result<Status, StatusCode> {
        fs::rename(self.path(old.as_bytes())?, self.path(new.as_bytes())?)
            .map_err(fixture_error)?;
        Ok(fixture_status(id))
    }

    async fn remove(
        &mut self,
        id: u32,
        path: SftpString,
    ) -> std::result::Result<Status, StatusCode> {
        fs::remove_file(self.path(path.as_bytes())?).map_err(fixture_error)?;
        Ok(fixture_status(id))
    }
}

async fn wait_for(job: &Transfer, ready: impl Fn(&Progress) -> bool) -> Progress {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let progress = job.progress();
            if ready(&progress) {
                return progress;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected transfer progress")
}

#[tokio::test]
async fn regular_files_round_trip_and_overwrite_cancel_then_retry() {
    let remote = tempfile::tempdir().unwrap();
    let (browser, _socket, server) = subsystem_browser(FileFixture(remote.path().into())).await;
    let input = tempfile::tempdir().unwrap();
    let file = input.path().join("a.txt");
    fs::write(&file, b"upload").unwrap();
    let upload = CopyRequest {
        upload: true,
        sources: vec![file.to_str().unwrap().into()],
        destination: "/".into(),
        name: None,
    };
    let job = browser.start(Arc::new(LocalPaths), upload.clone());
    let done = wait_for(&job, |p| p.done).await;
    assert_eq!((done.completed, done.bytes, done.error), (1, 6, None));
    assert_eq!(fs::read(remote.path().join("a.txt")).unwrap(), b"upload");

    fs::write(&file, b"retry").unwrap();
    let job = browser.start(Arc::new(LocalPaths), upload.clone());
    let conflict = wait_for(&job, |p| p.conflict.is_some()).await;
    assert_eq!(conflict.conflict.as_deref(), Some("/a.txt"));
    job.cancel();
    assert_eq!(
        wait_for(&job, |p| p.done).await.error.as_deref(),
        Some("Cancelled")
    );
    assert_eq!(fs::read(remote.path().join("a.txt")).unwrap(), b"upload");

    let retry = browser.start(Arc::new(LocalPaths), upload);
    let conflict = wait_for(&retry, |p| p.conflict.is_some()).await;
    retry.overwrite(conflict.conflict.as_deref().unwrap());
    assert!(wait_for(&retry, |p| p.done).await.error.is_none());
    assert_eq!(fs::read(remote.path().join("a.txt")).unwrap(), b"retry");
    assert_eq!(fs::read_dir(remote.path()).unwrap().count(), 1);

    let output = tempfile::tempdir().unwrap();
    let download = CopyRequest {
        upload: false,
        sources: vec!["/a.txt".into()],
        destination: output.path().to_str().unwrap().into(),
        name: None,
    };
    let job = browser.start(Arc::new(LocalPaths), download);
    let done = wait_for(&job, |p| p.done).await;
    assert_eq!((done.completed, done.bytes, done.error), (1, 5, None));
    assert_eq!(fs::read(output.path().join("a.txt")).unwrap(), b"retry");
    server.abort();
}

#[tokio::test]
async fn mixed_sources_and_file_promises_reject_the_whole_batch_before_writing() {
    let remote = tempfile::tempdir().unwrap();
    let (browser, _socket, server) = subsystem_browser(FileFixture(remote.path().into())).await;
    fs::write(remote.path().join("download.txt"), b"remote").unwrap();
    fs::create_dir(remote.path().join("folder")).unwrap();
    let input = tempfile::tempdir().unwrap();
    fs::write(input.path().join("upload.txt"), b"local").unwrap();
    fs::create_dir(input.path().join("folder")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("upload.txt", input.path().join("link")).unwrap();
    let mut invalid = vec!["folder"];
    if cfg!(unix) {
        invalid.push("link");
    }
    for name in invalid {
        let upload = CopyRequest {
            upload: true,
            sources: ["upload.txt", name]
                .map(|n| input.path().join(n).to_str().unwrap().to_owned())
                .to_vec(),
            destination: "/".into(),
            name: None,
        };
        let job = browser.start(Arc::new(LocalPaths), upload);
        assert_rejected(&job, name).await;
        assert_eq!(fs::read_dir(remote.path()).unwrap().count(), 2);
    }
    let output = tempfile::tempdir().unwrap();
    for source in ["/folder", "/link"] {
        let request = CopyRequest {
            upload: false,
            sources: vec!["/download.txt".into(), source.into()],
            destination: output.path().to_str().unwrap().into(),
            name: None,
        };
        let batch = Arc::new(DownloadBatch::new(request.sources.clone()));
        let promise = CopyRequest {
            sources: vec!["/download.txt".into()],
            name: Some("renamed.txt".into()),
            ..request.clone()
        };
        let job = browser.start(Arc::new(LocalPaths), request);
        let promised =
            browser.start_file_promise(Arc::new(LocalPaths), promise.clone(), batch.clone());
        let retry =
            browser.start_file_promise(Arc::new(LocalPaths), promise, Arc::new(batch.retry()));
        for job in [job, promised, retry] {
            assert_rejected(&job, source).await;
        }
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    }
    server.abort();
}

async fn assert_rejected(job: &Transfer, source: &str) {
    let done = wait_for(job, |p| p.done).await;
    assert_eq!((done.completed, done.bytes), (0, 0));
    assert!(done.error.unwrap().ends_with(source));
}
