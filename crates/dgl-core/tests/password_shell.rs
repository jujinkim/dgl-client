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
struct Fixture;
impl server::Server for Fixture {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self
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
        _: &[u8],
        s: &mut server::Session,
    ) -> Result<(), Self::Error> {
        s.channel_success(ch)?;
        s.exit_status_request(ch, 127)?;
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
    let task = tokio::spawn(async move {
        let mut server = Fixture;
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
        Some(fingerprint),
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
    let _ = stop_tx.send(());
    task.await.unwrap();
}
