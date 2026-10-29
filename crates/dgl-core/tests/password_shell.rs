use dgl_core::{
    Error,
    model::{AuthKind, Credential, HostProfile, Size},
    ssh,
};
use russh::{
    Channel, ChannelId, Pty,
    server::{self, Server as _},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Clone)]
struct Fixture {
    sizes: Arc<Mutex<Vec<(u32, u32)>>>,
}
impl server::Server for Fixture {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}
impl server::Handler for Fixture {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "synthetic-test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        ch: ChannelId,
        command: &[u8],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)?;
        s.exit_status_request(ch, if command == b":" { 0 } else { 127 })?;
        s.eof(ch)?;
        s.close(ch)?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    async fn pty_request(
        &mut self,
        ch: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        ch: ChannelId,
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)?;
        Ok(())
    }
    async fn data(
        &mut self,
        ch: ChannelId,
        data: &[u8],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.data(ch, data.to_vec())?;
        Ok(())
    }
    async fn window_change_request(
        &mut self,
        _: ChannelId,
        columns: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.sizes.lock().unwrap().push((columns, rows));
        Ok(())
    }
}
#[tokio::test]
async fn password_failure_and_plain_shell_diagnostics() {
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let fingerprint = key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let socket = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        ..Default::default()
    });
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let sizes = Arc::new(Mutex::new(Vec::new()));
    let server_sizes = sizes.clone();
    let task = tokio::spawn(async move {
        let mut server = Fixture {
            sizes: server_sizes,
        };
        let running = server.run_on_socket(config, &socket);
        let stop = running.handle();
        tokio::select! {r=running=>r.unwrap(),_=stop_rx=>stop.shutdown("test complete".into())}
    });
    let host = HostProfile {
        id: "fixture".into(),
        alias: "fixture".into(),
        hostname: "127.0.0.1".into(),
        port,
        username: "fixture".into(),
        auth: AuthKind::Password,
        credential_ref: "fixture".into(),
        favorite: false,
        last_connected: 0,
        last_session: None,
        last_pane: None,
    };
    let mut credential = Credential::default();
    credential.password = "wrong".into();
    assert_eq!(
        ssh::connect(
            &host,
            &credential,
            Some(fingerprint.clone()),
            Arc::new(Mutex::new(None))
        )
        .await
        .err(),
        Some(Error::Authentication)
    );
    credential.password = "synthetic-test-only".into();
    let mut ssh = ssh::connect(
        &host,
        &credential,
        Some(fingerprint.clone()),
        Arc::new(Mutex::new(None)),
    )
    .await
    .unwrap();
    assert!(!ssh.discover().await.unwrap());
    assert_eq!(ssh.diagnostics.len(), 3);
    assert!(ssh.diagnostics.iter().all(|p| p.exit_status == Some(127)));
    let mut shell = ssh.shell(Size::default()).await.unwrap();
    shell.data(&b"plain-shell\r"[..]).await.unwrap();
    let bytes = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(russh::ChannelMsg::Data { data }) = shell.wait().await {
                return data;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(&bytes[..], b"plain-shell\r");
    shell.close().await.unwrap();
    ssh.close().await;
    use dgl_core::{
        connection::{Action, Connection, Phase},
        flow::OutputKind,
    };
    let connection = Connection::start(host, credential, Some(fingerprint));
    tokio::time::timeout(Duration::from_secs(5), async {
        while connection.state().phase != Phase::Shell {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let before = connection.target().unwrap();
    let subscription = connection.output().subscribe().unwrap();
    for _ in 0..2 {
        let request = connection
            .action(Action::Resize {
                columns: 97,
                rows: 19,
            })
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(result) = connection
                    .completions()
                    .into_iter()
                    .find(|r| r.request_id == request)
                {
                    assert_eq!(result.error, None);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    let after = connection.target().unwrap();
    assert_eq!(before.generation.connection, after.generation.connection);
    assert_eq!(before.generation.pane, after.generation.pane);
    assert_eq!(before.generation.size + 1, after.generation.size);
    assert_eq!(connection.state().phase, Phase::Shell);
    assert!(!connection.state().resizing);
    let event = connection.output().poll(subscription).unwrap().unwrap();
    assert_eq!(event.kind, OutputKind::Resize);
    assert_eq!(event.size, Size::new(97, 19).unwrap());
    connection
        .output()
        .ack(subscription, event.sequence)
        .unwrap();
    let check = connection.action(Action::CheckConnection).unwrap();
    assert!(connection.state().checking);
    assert_eq!(
        connection.input(after.clone(), b"blocked".to_vec()),
        Err(Error::NotReady)
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(result) = connection
                .completions()
                .into_iter()
                .find(|r| r.request_id == check)
            {
                assert_eq!(result.error, None);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!connection.state().checking);
    assert_eq!(connection.state().phase, Phase::Shell);
    assert_eq!(connection.target().unwrap(), after);
    connection.input(after, b"after-resize".to_vec()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(event) = connection.output().poll(subscription).unwrap() {
                assert_eq!(event.kind, OutputKind::Live);
                assert_eq!(event.bytes, b"after-resize");
                connection
                    .output()
                    .ack(subscription, event.sequence)
                    .unwrap();
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(*sizes.lock().unwrap(), vec![(97, 19)]);
    connection.close();
    connection.join().await;
    let _ = stop_tx.send(());
    task.await.unwrap();
}
