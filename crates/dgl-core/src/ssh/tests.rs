use super::*;
use crate::connection::{Connection, Phase};
use russh::{
    ChannelId, Pty,
    server::{self, Server as _},
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{Instant, timeout},
};

// All keys and peers are generated locally. No sshd, tmux, user credentials or network
// outside loopback is involved. A blocked callback models a peer that stops reading.
#[derive(Clone)]
struct Peer {
    number: usize,
    connections: Arc<AtomicUsize>,
    authentications: Arc<AtomicUsize>,
    received: Arc<Mutex<Vec<(usize, Vec<u8>)>>>,
    stall: bool,
    hold_open: bool,
    stopped: watch::Receiver<bool>,
}

impl server::Server for Peer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let mut peer = self.clone();
        peer.number = self.connections.fetch_add(1, Ordering::SeqCst);
        peer
    }
}

impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        _: &str,
        _: &str,
    ) -> std::result::Result<server::Auth, Self::Error> {
        self.authentications.fetch_add(1, Ordering::SeqCst);
        Ok(server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> std::result::Result<(), Self::Error> {
        if self.hold_open {
            let _ = self.stopped.changed().await;
        } else {
            reply.accept().await;
        }
        Ok(())
    }
    async fn exec_request(
        &mut self,
        ch: ChannelId,
        _: &[u8],
        s: &mut server::Session,
    ) -> std::result::Result<(), Self::Error> {
        // No tmux: Connection proceeds to its ordinary shell path.
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
    ) -> std::result::Result<(), Self::Error> {
        s.channel_success(ch)
    }
    async fn shell_request(
        &mut self,
        ch: ChannelId,
        s: &mut server::Session,
    ) -> std::result::Result<(), Self::Error> {
        s.channel_success(ch)
    }
    async fn data(
        &mut self,
        _: ChannelId,
        bytes: &[u8],
        _: &mut server::Session,
    ) -> std::result::Result<(), Self::Error> {
        self.received
            .lock()
            .unwrap()
            .push((self.number, bytes.to_vec()));
        if self.stall && self.number == 0 {
            let _ = self.stopped.changed().await;
            return Err(russh::Error::Disconnect);
        }
        Ok(())
    }
}

pub(crate) struct Fixture {
    host: HostProfile,
    fingerprint: String,
    peer: Peer,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.task.abort();
    }
}
impl Fixture {
    pub(crate) async fn new(window: u32, packet: u32, stall: bool, hold_open: bool) -> Self {
        let key =
            russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
                .unwrap();
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let socket = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = socket.local_addr().unwrap().port();
        let (stop, stopped) = watch::channel(false);
        let peer = Peer {
            number: 0,
            connections: Arc::default(),
            authentications: Arc::default(),
            received: Arc::default(),
            stall,
            hold_open,
            stopped,
        };
        let mut server = peer.clone();
        let config = Arc::new(server::Config {
            keys: vec![key],
            window_size: window,
            maximum_packet_size: packet,
            auth_rejection_time: Duration::ZERO,
            ..Default::default()
        });
        let task = tokio::spawn(async move {
            server.run_on_socket(config, &socket).await.unwrap();
        });
        Self {
            host: HostProfile {
                id: "transport-fixture".into(),
                alias: "transport-fixture".into(),
                hostname: "127.0.0.1".into(),
                port,
                username: "fixture".into(),
                auth: AuthKind::Password,
                credential_ref: "fixture".into(),
                favorite: false,
                last_connected: 0,
                last_session: None,
                last_pane: None,
            },
            fingerprint,
            peer,
            stop,
            task,
        }
    }
    pub(crate) async fn connect(&self) -> Ssh {
        connect(
            &self.host,
            &Credential::default(),
            Some(self.fingerprint.clone()),
            Arc::default(),
        )
        .await
        .unwrap()
    }
    fn connection(&self) -> Arc<Connection> {
        Connection::start(
            self.host.clone(),
            Credential::default(),
            Some(self.fingerprint.clone()),
        )
    }
    fn bytes(&self, connection: usize) -> Vec<u8> {
        self.peer
            .received
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| *n == connection)
            .flat_map(|(_, bytes)| bytes.iter().copied())
            .collect()
    }
}

async fn until(mut ready: impl FnMut() -> bool) {
    timeout(Duration::from_secs(5), async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("local peer did not reach the expected state");
}

#[test]
fn russh_guard_rejects_nested_certificates_before_recursive_decode() {
    use russh::keys::{PublicKey, key::PublicKeyExt, ssh_key};
    fn field(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        out.extend_from_slice(bytes);
    }
    // SSH wire fields for a synthetic extension key and certificate. Each signing
    // key is itself a certificate; a shallow control confirms the fixture parses.
    let mut blob = Vec::new();
    field(&mut blob, b"fixture@dgl");
    field(&mut blob, b"key");
    for depth in 1..=64 {
        let mut cert = Vec::new();
        field(&mut cert, b"fixture-cert-v01@dgl");
        field(&mut cert, b"nonce");
        field(&mut cert, b"key");
        cert.extend_from_slice(&1u64.to_be_bytes());
        cert.extend_from_slice(&1u32.to_be_bytes());
        field(&mut cert, b"local-fixture");
        field(&mut cert, b"");
        cert.extend_from_slice(&0u64.to_be_bytes());
        cert.extend_from_slice(&u64::MAX.to_be_bytes());
        for _ in 0..3 {
            field(&mut cert, b"");
        }
        field(&mut cert, &blob);
        let mut signature = Vec::new();
        field(&mut signature, b"fixture@dgl");
        field(&mut signature, b"signature");
        field(&mut cert, &signature);
        blob = cert;
        // Call russh's protected entry point, never ssh-key's direct decoder.
        let result = <PublicKey as PublicKeyExt>::decode(&blob);
        if depth <= 8 {
            assert!(result.is_ok(), "shallow certificate {depth}: {result:?}");
        } else {
            assert!(
                matches!(
                    result,
                    Err(russh::keys::Error::SshKey(
                        ssh_key::Error::CertificateFieldInvalid(
                            ssh_key::certificate::Field::SignatureKey
                        )
                    ))
                ),
                "depth {depth}: {result:?}"
            );
        }
    }
}

#[tokio::test]
async fn zero_maximum_packet_size_is_rejected_during_channel_open() {
    let fixture = Fixture::new(65536, 0, false, false).await;
    let ssh = fixture.connect().await;
    // The outer watchdog is shorter than DGL's timeout: this must be a peer
    // protocol rejection, not the write timeout hiding an unfixed russh parser.
    let result = timeout(Duration::from_secs(2), ssh.handle.channel_open_session())
        .await
        .unwrap();
    assert!(result.is_err());
    assert!(ssh.handle.is_closed());
    ssh.close().await;
}

#[tokio::test]
async fn stalled_channel_open_is_bounded() {
    let fixture = Fixture::new(65536, 16384, false, true).await;
    let ssh = fixture.connect().await;
    let start = Instant::now();
    assert_eq!(
        timeout(Duration::from_secs(18), ssh.shell(Size::default()))
            .await
            .unwrap()
            .err(),
        Some(Error::Network)
    );
    assert!(start.elapsed() >= TRANSPORT_TIMEOUT);
    ssh.abort();
    ssh.close().await;
}

#[tokio::test]
async fn stalled_zero_window_write_times_out_but_normal_write_succeeds() {
    let fixture = Fixture::new(0, 16384, false, false).await;
    let ssh = fixture.connect().await;
    let stream = ssh.shell(Size::default()).await.unwrap();
    let start = Instant::now();
    assert_eq!(
        timeout(Duration::from_secs(18), write(&stream, b"unsent"))
            .await
            .unwrap(),
        Err(Error::Network)
    );
    assert!(start.elapsed() >= TRANSPORT_TIMEOUT);
    assert!(fixture.bytes(0).is_empty());
    ssh.abort();
    ssh.close().await;

    let fixture = Fixture::new(65536, 16384, false, false).await;
    let ssh = fixture.connect().await;
    let stream = ssh.shell(Size::default()).await.unwrap();
    // A quiet terminal is allowed to outlive the active-write deadline.
    tokio::time::pause();
    tokio::time::advance(TRANSPORT_TIMEOUT + Duration::from_secs(1)).await;
    tokio::time::resume();
    write(&stream, b"normal write").await.unwrap();
    until(|| fixture.bytes(0) == b"normal write").await;
    ssh.close().await;
}

#[tokio::test]
async fn partial_input_failure_reconnects_without_replay_and_rejects_old_target() {
    let fixture = Fixture::new(32, 32, true, false).await;
    let c = fixture.connection();
    until(|| c.state().phase == Phase::Shell).await;
    let old = c.target().unwrap();
    let input = c.input(old.clone(), vec![b'x'; 65536]).unwrap();
    until(|| !fixture.bytes(0).is_empty()).await;
    let completion = timeout(Duration::from_secs(18), async {
        loop {
            if let Some(result) = c.completions().into_iter().find(|r| r.request_id == input) {
                break result;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(completion.error, Some(Error::Network));
    let prefix = fixture.bytes(0);
    assert!(!prefix.is_empty() && prefix.len() < 65536);
    until(|| {
        c.state().phase == Phase::Shell
            && c.state().generation.connection > old.generation.connection
    })
    .await;
    assert_eq!(
        c.input(old, b"stale input".to_vec()),
        Err(Error::StaleGeneration)
    );
    let fresh = c.input(c.target().unwrap(), b"fresh".to_vec()).unwrap();
    until(|| fixture.bytes(1) == b"fresh").await;
    until(|| {
        c.completions()
            .iter()
            .any(|r| r.request_id == fresh && r.error.is_none())
    })
    .await;
    assert_eq!(fixture.bytes(0), prefix);
    c.close();
    c.join().await;
}

#[tokio::test]
async fn close_immediately_cancels_a_partly_sent_input() {
    let fixture = Fixture::new(32, 32, true, false).await;
    let c = fixture.connection();
    until(|| c.state().phase == Phase::Shell).await;
    let request = c.input(c.target().unwrap(), vec![b'x'; 65536]).unwrap();
    until(|| !fixture.bytes(0).is_empty()).await;
    c.close();
    timeout(Duration::from_secs(1), c.join()).await.unwrap();
    let results = c.completions();
    assert_eq!(
        results.iter().filter(|r| r.request_id == request).count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .find(|r| r.request_id == request)
            .unwrap()
            .error,
        Some(Error::Cancelled)
    );
    assert_eq!(fixture.peer.connections.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn host_key_mismatch_blocks_authentication() {
    let fixture = Fixture::new(65536, 16384, false, false).await;
    let observed = Arc::new(Mutex::new(None));
    let result = connect(
        &fixture.host,
        &Credential::default(),
        Some("wrong-fingerprint".into()),
        observed.clone(),
    )
    .await;
    assert_eq!(result.err(), Some(Error::HostKeyChanged));
    assert_eq!(*observed.lock().unwrap(), Some(fixture.fingerprint.clone()));
    assert_eq!(fixture.peer.authentications.load(Ordering::SeqCst), 0);
    fixture.connect().await.close().await;
    assert_eq!(fixture.peer.authentications.load(Ordering::SeqCst), 1);
}
