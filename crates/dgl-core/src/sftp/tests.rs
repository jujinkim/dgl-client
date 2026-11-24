use super::*;
use russh_sftp::protocol::{File, Handle, Name, SftpHandle, SftpString, Status, StatusCode};
use std::fs;

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
        assert!(LocalPaths.create(dir, bad, false).is_err());
    }
    let part = LocalPaths.create(dir, "part", false).unwrap();
    let existing = LocalPaths.create(dir, "result", false).unwrap();
    fs::write(&existing, b"keep").unwrap();
    assert!(LocalPaths.publish(&part, dir, "result", None).is_err());
    assert_eq!(fs::read(existing).unwrap(), b"keep");
}

struct SubsystemOnly {
    channel: Option<russh::Channel<russh::server::Msg>>,
    reading: Arc<tokio::sync::Notify>,
}
impl russh::server::Handler for SubsystemOnly {
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
            Directories {
                reading: self.reading.clone(),
            },
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
#[tokio::test]
async fn host_without_tmux_and_disconnect_during_download() {
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
    let reading = Arc::new(tokio::sync::Notify::new());
    let peer = SubsystemOnly {
        channel: None,
        reading: reading.clone(),
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
