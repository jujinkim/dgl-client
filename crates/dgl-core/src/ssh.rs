use crate::{
    Error, Result,
    command::{self, Command},
    model::{AuthKind, Credential, HostProfile, Size},
};
use russh::{
    Channel, ChannelMsg, client,
    keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
pub type Stream = Channel<client::Msg>;
pub struct Handler {
    trusted: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
}
impl client::Handler for Handler {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        *self.observed.lock().unwrap() = Some(fingerprint.clone());
        Ok(self.trusted.as_ref() == Some(&fingerprint))
    }
}
struct SocketGuard(std::net::TcpStream);
impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = self.0.shutdown(std::net::Shutdown::Both);
    }
}
pub struct Ssh {
    socket: SocketGuard,
    pub handle: client::Handle<Handler>,
    pub tmux: Option<String>,
    pub diagnostics: Vec<Probe>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Probe {
    pub stage: String,
    pub exit_status: Option<u32>,
    pub outcome: String,
    pub path: Option<String>,
    pub version: Option<String>,
}
pub async fn connect(
    host: &HostProfile,
    credential: &Credential,
    trusted: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
) -> Result<Ssh> {
    host.validate()?;
    let config = client::Config {
        window_size: 65536,
        maximum_packet_size: 16384,
        channel_buffer_size: 1,
        keepalive_interval: Some(Duration::from_secs(10)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    };
    let handler = Handler {
        trusted: trusted.clone(),
        observed: observed.clone(),
    };
    let tcp = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::net::TcpStream::connect((host.hostname.as_str(), host.port)),
    )
    .await
    .map_err(|_| Error::Network)?
    .map_err(|_| Error::Network)?;
    let socket = tcp.into_std().map_err(|_| Error::Network)?;
    let guard = SocketGuard(socket.try_clone().map_err(|_| Error::Network)?);
    let tcp = tokio::net::TcpStream::from_std(socket).map_err(|_| Error::Network)?;
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        client::connect_stream(Arc::new(config), tcp, handler),
    )
    .await
    .map_err(|_| Error::Network)?;
    let mut handle = result.map_err(|_| {
        if let Some(ref key) = *observed.lock().unwrap() {
            if trusted.as_ref() != Some(key) {
                if trusted.is_some() {
                    Error::HostKeyChanged
                } else {
                    Error::HostKeyUnknown
                }
            } else {
                Error::Network
            }
        } else {
            Error::Network
        }
    })?;
    let auth = tokio::time::timeout(Duration::from_secs(15), async {
        match host.auth {
            AuthKind::Password => handle
                .authenticate_password(&host.username, &credential.password)
                .await
                .map_err(|_| Error::Authentication),
            AuthKind::PrivateKey => {
                let key = russh::keys::decode_secret_key(
                    &credential.private_key,
                    if credential.passphrase.is_empty() {
                        None
                    } else {
                        Some(&credential.passphrase)
                    },
                )
                .map_err(|_| Error::Authentication)?;
                let alg = handle
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|_| Error::Authentication)?
                    .flatten();
                handle
                    .authenticate_publickey(
                        &host.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), alg),
                    )
                    .await
                    .map_err(|_| Error::Authentication)
            }
        }
    })
    .await
    .map_err(|_| Error::Authentication)??;
    if !auth.success() {
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        return Err(Error::Authentication);
    }
    Ok(Ssh {
        socket: guard,
        handle,
        tmux: None,
        diagnostics: vec![],
    })
}
impl Ssh {
    pub async fn execute(&self, command: &str) -> Result<(u32, Vec<u8>)> {
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut channel = self
                .handle
                .channel_open_session()
                .await
                .map_err(|_| Error::Network)?;
            channel
                .exec(true, command)
                .await
                .map_err(|_| Error::Network)?;
            let mut out = Vec::new();
            let mut exit = None;
            while let Some(m) = channel.wait().await {
                match m {
                    ChannelMsg::Data { data } => {
                        if out.len() + data.len() > 4 * 1024 * 1024 {
                            return Err(Error::Protocol);
                        }
                        out.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                    _ => {}
                }
            }
            Ok((exit.ok_or(Error::Network)?, out))
        })
        .await
        .map_err(|_| Error::Network)?
    }
    pub async fn execute_tmux(&self, c: &Command) -> Result<(u32, Vec<u8>)> {
        self.execute(&c.shell_with_executable(self.tmux.as_deref().ok_or(Error::NotReady)?)?)
            .await
    }
    pub async fn discover(&mut self) -> Result<bool> {
        let print = "printf '\\036DGL_TMUX_PATH\\037%s\\036' \"$dgl_tmux_path\"";
        let probe = format!(
            "dgl_tmux_path=$(command -v tmux) || exit 127; case \"$dgl_tmux_path\" in /*) ;; *) exit 127 ;; esac; [ -f \"$dgl_tmux_path\" ] && [ -x \"$dgl_tmux_path\" ] || exit 126; {print}"
        );
        let direct = format!("/bin/sh -c {}", command::quote(&probe)?);
        let login = format!(
            "/bin/sh -c {}",
            command::quote(&format!(
                "exec \"${{SHELL:-/bin/sh}}\" -lc {}",
                command::quote(&format!("exec {direct}"))?
            ))?
        );
        let common = format!(
            "/bin/sh -c {}",
            command::quote(&format!(
                "for dgl_tmux_path in /opt/homebrew/bin/tmux /usr/local/bin/tmux /usr/bin/tmux /bin/tmux /opt/local/bin/tmux /home/linuxbrew/.linuxbrew/bin/tmux /run/current-system/sw/bin/tmux /snap/bin/tmux \"$HOME/.local/bin/tmux\" \"$HOME/bin/tmux\" \"$HOME/.linuxbrew/bin/tmux\" \"$HOME/.nix-profile/bin/tmux\"; do if [ -f \"$dgl_tmux_path\" ] && [ -x \"$dgl_tmux_path\" ]; then {print}; exit 0; fi; done; exit 127"
            ))?
        );
        for (stage, command) in [("direct", direct), ("login", login), ("common", common)] {
            let (status, out) =
                match tokio::time::timeout(Duration::from_secs(4), self.execute(&command)).await {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        self.diagnostics.push(Probe {
                            stage: stage.into(),
                            exit_status: None,
                            outcome: "timeout".into(),
                            path: None,
                            version: None,
                        });
                        continue;
                    }
                };
            let marker = "\x1eDGL_TMUX_PATH\x1f";
            let text = String::from_utf8_lossy(&out);
            let parts: Vec<_> = text.split(marker).collect();
            let path = if status == 0 && parts.len() == 2 {
                parts[1]
                    .split_once('\x1e')
                    .map(|(p, _)| p)
                    .filter(|p| p.starts_with('/') && command::quote(p).is_ok())
            } else {
                None
            };
            self.diagnostics.push(Probe {
                stage: stage.into(),
                exit_status: Some(status),
                outcome: if path.is_some() {
                    "found"
                } else if status == 127 {
                    "not-found"
                } else if status != 0 {
                    "failed"
                } else {
                    "invalid"
                }
                .into(),
                path: path.map(str::to_owned),
                version: None,
            });
            if let Some(p) = path {
                self.tmux = Some(p.to_string());
                let (code, version) = self.execute(&format!("{} -V", command::quote(p)?)).await?;
                let supported =
                    code == 0 && command::supports_tmux(&String::from_utf8_lossy(&version));
                if !supported {
                    self.diagnostics.push(Probe {
                        stage: "version".into(),
                        exit_status: Some(code),
                        outcome: if code != 0 {
                            "failed"
                        } else if !String::from_utf8_lossy(&version)
                            .trim()
                            .starts_with("tmux ")
                        {
                            "invalid"
                        } else {
                            "unsupported"
                        }
                        .into(),
                        path: Some(p.to_string()),
                        version: Some(
                            String::from_utf8_lossy(&version)
                                .trim()
                                .chars()
                                .take(128)
                                .collect(),
                        ),
                    });
                }
                return Ok(supported);
            }
        }
        Ok(false)
    }
    pub async fn control(&self, id: &str) -> Result<Stream> {
        command::target(id, '$')?;
        let c = self
            .handle
            .channel_open_session()
            .await
            .map_err(|_| Error::Network)?;
        c.exec(
            true,
            format!(
                "{} -u -C attach-session -t {}",
                command::quote(self.tmux.as_deref().ok_or(Error::NotReady)?)?,
                command::target(id, '$')?
            ),
        )
        .await
        .map_err(|_| Error::Network)?;
        Ok(c)
    }
    pub async fn shell(&self, size: Size) -> Result<Stream> {
        let c = self
            .handle
            .channel_open_session()
            .await
            .map_err(|_| Error::Network)?;
        c.request_pty(
            true,
            "xterm-256color",
            size.columns.into(),
            size.rows.into(),
            0,
            0,
            &[],
        )
        .await
        .map_err(|_| Error::Network)?;
        c.request_shell(true).await.map_err(|_| Error::Network)?;
        Ok(c)
    }
    pub async fn close(mut self) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.handle
                .disconnect(russh::Disconnect::ByApplication, "", "en"),
        )
        .await;
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut self.handle).await;
        let _ = self.socket.0.shutdown(std::net::Shutdown::Both);
    }
}
pub fn public_key(credential: &Credential) -> Result<String> {
    russh::keys::decode_secret_key(
        &credential.private_key,
        if credential.passphrase.is_empty() {
            None
        } else {
            Some(&credential.passphrase)
        },
    )
    .map_err(|_| Error::Authentication)?
    .public_key()
    .to_openssh()
    .map_err(|_| Error::Authentication)
}

pub fn generate_key() -> Result<Credential> {
    let key = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
        .map_err(|_| Error::Authentication)?;
    let private_key = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .map_err(|_| Error::Authentication)?
        .to_string();
    Ok(Credential {
        password: String::new(),
        private_key,
        passphrase: String::new(),
    })
}
