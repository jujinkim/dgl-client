//! Local multiplexer discovery and child ownership. Never runs installation commands.
use crate::{Error, Result, command::Command, ssh::Probe};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout},
};

pub const TMUX_INSTALL: &str = "https://github.com/tmux/tmux/wiki/Installing";
pub const PSMUX_INSTALL: &str = "https://github.com/psmux/psmux#installation";
const LIMIT: usize = 4 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multiplexer {
    Tmux,
    Psmux,
}
#[derive(Clone, Debug)]
pub struct LocalConfig {
    pub executable: PathBuf,
    pub multiplexer: Multiplexer,
    /// Only fixtures supply a private socket/namespace. Product connections use the user's default.
    pub arguments: Vec<String>,
    pub last_session: Option<String>,
    pub last_pane: Option<String>,
}
impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            executable: if cfg!(windows) { "psmux.exe" } else { "tmux" }.into(),
            multiplexer: if cfg!(windows) {
                Multiplexer::Psmux
            } else {
                Multiplexer::Tmux
            },
            arguments: vec![],
            last_session: None,
            last_pane: None,
        }
    }
}
impl LocalConfig {
    pub async fn detect(mut self) -> Result<Local> {
        // GUI launches on macOS often omit Homebrew from PATH.
        if !self.executable.is_absolute() {
            let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default();
            if self.multiplexer == Multiplexer::Tmux {
                paths.extend(
                    [
                        "/opt/homebrew/bin",
                        "/usr/local/bin",
                        "/usr/bin",
                        "/bin",
                        "/opt/local/bin",
                    ]
                    .map(PathBuf::from),
                );
            }
            self.executable = paths
                .into_iter()
                .map(|p| p.join(&self.executable))
                .find(|p| p.is_file())
                .ok_or(Error::LocalToolMissing)?;
        }
        let local = Local {
            config: self,
            version: String::new(),
        };
        let (code, bytes) = local.execute_args(&["-V".into()]).await?;
        let version = String::from_utf8_lossy(&bytes).trim().to_owned();
        let supported = match local.config.multiplexer {
            Multiplexer::Tmux => crate::command::supports_tmux(&version),
            Multiplexer::Psmux => crate::local::psmux::supported(&version),
        };
        if code != 0 || !supported {
            return Err(Error::LocalToolUnsupported);
        }
        // A read-only query: do not start a server, create a session, or use a shell fallback.
        let (code, commands) = local.execute_args(&["list-commands".into()]).await?;
        if code != 0
            || ![
                "capture-pane",
                "refresh-client",
                "send-keys",
                "list-sessions",
            ]
            .iter()
            .all(|c| String::from_utf8_lossy(&commands).contains(c))
        {
            return Err(Error::LocalToolUnsupported);
        }
        Ok(Local { version, ..local })
    }
}
#[derive(Clone, Debug)]
pub struct Local {
    pub config: LocalConfig,
    pub version: String,
}
fn io_error(error: std::io::Error) -> Error {
    match error.kind() {
        std::io::ErrorKind::NotFound => Error::LocalToolMissing,
        std::io::ErrorKind::PermissionDenied => Error::LocalToolPermission,
        _ => Error::Network,
    }
}
/// Host programs must not inherit AppImage's bundled loader/plugin search paths.
pub(crate) fn host_command(path: &std::path::Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(path);
    if std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some() {
        for key in [
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "PYTHONHOME",
            "PYTHONPATH",
            "QT_PLUGIN_PATH",
            "GIO_EXTRA_MODULES",
            "GDK_PIXBUF_MODULE_FILE",
        ] {
            command.env_remove(key);
        }
    }
    command
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env_remove("PSMUX_SESSION_NAME")
        .env_remove("PSMUX_TARGET_SESSION")
        .env_remove("PSMUX_TARGET_FULL");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW: own only the client.
    command.kill_on_drop(true);
    command
}
impl Local {
    pub fn diagnostic(&self) -> Probe {
        Probe {
            stage: "local".into(),
            exit_status: Some(0),
            outcome: "found".into(),
            path: Some(self.config.executable.display().to_string()),
            version: Some(self.version.clone()),
        }
    }
    fn command(&self, args: &[String]) -> tokio::process::Command {
        let mut command = host_command(&self.config.executable);
        command.args(&self.config.arguments).args(args);
        command
    }
    pub(crate) async fn execute_args(&self, args: &[String]) -> Result<(u32, Vec<u8>)> {
        let mut command = self.command(args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(io_error)?;
        let mut stdout = child.stdout.take().ok_or(Error::Network)?;
        let result = tokio::time::timeout(TIMEOUT, async {
            let mut out = Vec::new();
            (&mut stdout)
                .take((LIMIT + 1) as u64)
                .read_to_end(&mut out)
                .await
                .map_err(io_error)?;
            if out.len() > LIMIT {
                return Err(Error::Protocol);
            }
            let status = child.wait().await.map_err(io_error)?;
            Ok((status.code().unwrap_or(1) as u32, out))
        })
        .await
        .unwrap_or(Err(Error::Network));
        if result.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        result
    }
    pub(crate) async fn execute(&self, command: &Command) -> Result<(u32, Vec<u8>)> {
        if self.config.multiplexer == Multiplexer::Psmux {
            if command.wire == Command::inventory().wire {
                return Ok((0, psmux::inventory(self).await?));
            }
            let mut args = command.argv()?;
            if let Some(index) = args.iter().position(|s| s == "-t") {
                if let Some(id) = args.get(index + 1).filter(|s| s.starts_with(['@', '%'])) {
                    let (session, native) = psmux::native(id)?;
                    args[index + 1] = format!("${session}:{native}");
                }
            }
            return self.execute_args(&args).await;
        }
        let mut args = command.argv()?;
        if self.config.multiplexer == Multiplexer::Tmux {
            args.insert(0, "-u".into());
        }
        self.execute_args(&args).await
    }
    pub(crate) async fn control(&self, session: &str, ignore_size: bool) -> Result<Process> {
        crate::command::target(session, '$')?;
        if self.config.multiplexer == Multiplexer::Psmux {
            // Resolve the stable session ID to its current name explicitly; bare -CC may create default.
            let command = Command::new(&crate::command::Operation::Sessions)?;
            let (code, bytes) = self.execute_args(&command.argv()?).await?;
            if code != 0 {
                return Err(Error::MissingSession);
            }
            let rows: Vec<_> = bytes
                .split(|b| *b == b'\n')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_vec())
                .collect();
            let sessions = crate::model::sessions(&rows)?;
            let name = sessions
                .iter()
                .find(|s| s.id == session)
                .ok_or(Error::MissingSession)?
                .name
                .clone();
            let mut process = self
                .spawn(
                    &[
                        "-CC".into(),
                        "attach-session".into(),
                        "-t".into(),
                        session.into(),
                    ],
                    Some(&name),
                )
                .await?;
            process.framing = Some(Default::default());
            return Ok(process);
        }
        let mut args = vec!["-u".into(), "-C".into(), "attach-session".into()];
        if ignore_size {
            args.extend(["-f".into(), "ignore-size".into()]);
        }
        args.extend(["-t".into(), session.into()]);
        self.spawn(&args, None).await
    }
    pub(crate) async fn spawn(&self, args: &[String], session: Option<&str>) -> Result<Process> {
        let mut command = self.command(args);
        if let Some(session) = session {
            command.env("PSMUX_SESSION_NAME", session);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(io_error)?;
        Ok(Process {
            input: tokio::sync::Mutex::new(child.stdin.take().ok_or(Error::Network)?),
            output: child.stdout.take().ok_or(Error::Network)?,
            child: tokio::sync::Mutex::new(child),
            framing: None,
        })
    }
}
pub(crate) struct Process {
    input: tokio::sync::Mutex<ChildStdin>,
    output: ChildStdout,
    child: tokio::sync::Mutex<Child>,
    framing: Option<psmux::Framing>,
}
impl Process {
    pub(crate) async fn write(&self, bytes: &[u8]) -> Result<()> {
        tokio::time::timeout(TIMEOUT, async {
            let mut input = self.input.lock().await;
            input.write_all(bytes).await.map_err(|_| Error::Network)?;
            input.flush().await.map_err(|_| Error::Network)
        })
        .await
        .map_err(|_| Error::Network)?
    }
    pub(crate) async fn read(&mut self) -> Option<Vec<u8>> {
        let mut bytes = vec![0; 16384];
        match self.output.read(&mut bytes).await {
            Ok(0) | Err(_) => None,
            Ok(n) => {
                bytes.truncate(n);
                match &mut self.framing {
                    Some(f) => f.feed(&bytes).ok(),
                    None => Some(bytes),
                }
            }
        }
    }
    pub(crate) async fn close(&self) -> Result<()> {
        let _ = self.input.lock().await.shutdown().await;
        let mut child = self.child.lock().await;
        if tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .is_err()
        {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        Ok(())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // Killing the control client never kills its server or the session's processes.
        let _ = self.child.get_mut().start_kill(); // Tokio reaps dropped children on its process driver.
    }
}
pub mod psmux;

pub fn installation(kind: Multiplexer, os_release: &str) -> (Option<&'static str>, &'static str) {
    if kind == Multiplexer::Psmux {
        return (Some("winget install psmux"), PSMUX_INSTALL);
    }
    if cfg!(target_os = "macos") && os_release.is_empty() {
        return (Some("brew install tmux"), TMUX_INSTALL);
    }
    let fields: std::collections::HashMap<_, _> = os_release
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k, v.trim_matches(['\'', '"'])))
        .collect();
    let ids = format!(
        "{} {}",
        fields.get("ID").unwrap_or(&""),
        fields.get("ID_LIKE").unwrap_or(&"")
    );
    let command = ids.split_whitespace().find_map(|id| match id {
        "ubuntu" | "debian" => Some("sudo apt install tmux"),
        "fedora" | "rhel" => Some("sudo dnf install tmux"),
        "arch" => Some("sudo pacman -S tmux"),
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "suse" => {
            Some("sudo zypper install tmux")
        }
        _ => None,
    });
    (command, TMUX_INSTALL)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_commands_follow_distribution_and_unknown_has_only_official_guide() {
        for (os, command) in [
            ("ID=ubuntu", "sudo apt install tmux"),
            ("ID=custom\nID_LIKE=debian", "sudo apt install tmux"),
            ("ID=fedora", "sudo dnf install tmux"),
            ("ID=arch", "sudo pacman -S tmux"),
            ("ID=opensuse-leap", "sudo zypper install tmux"),
        ] {
            assert_eq!(
                installation(Multiplexer::Tmux, os),
                (Some(command), TMUX_INSTALL)
            );
        }
        assert_eq!(
            installation(Multiplexer::Tmux, "ID=unknown"),
            (None, TMUX_INSTALL)
        );
        assert_eq!(
            installation(Multiplexer::Psmux, ""),
            (Some("winget install psmux"), PSMUX_INSTALL)
        );
    }
    #[tokio::test]
    async fn missing_tools_are_distinct_from_network_and_can_be_checked_again() {
        for _ in 0..2 {
            let config = LocalConfig {
                executable: "/nonexistent/dgl-fixture-tmux".into(),
                ..Default::default()
            };
            assert!(matches!(
                config.detect().await,
                Err(Error::LocalToolMissing)
            ));
        }
    }
    #[test]
    fn argv_preserves_untrusted_names_as_one_argument() {
        let name = "space ' quote $HOME `id` ";
        let args = Command::new(&crate::command::Operation::NewSession { name: name.into() })
            .unwrap()
            .argv()
            .unwrap();
        assert_eq!(args, ["new-session", "-d", "-s", name]);
        assert_eq!(
            Command::inventory()
                .argv()
                .unwrap()
                .iter()
                .filter(|s| *s == ";")
                .count(),
            2
        );
    }
}

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;
    #[tokio::test]
    async fn process_roundtrip_close_reaps_client_and_writes_fail_without_retry() {
        let local = Local {
            config: LocalConfig {
                executable: "/bin/cat".into(),
                ..Default::default()
            },
            version: String::new(),
        };
        let mut process = local.spawn(&[], None).await.unwrap();
        process.write(b"literal $HOME `id`\n").await.unwrap();
        assert_eq!(process.read().await.unwrap(), b"literal $HOME `id`\n");
        process.close().await.unwrap();
        assert!(process.child.lock().await.try_wait().unwrap().is_some());
        assert_eq!(
            process.write(b"never retry").await.unwrap_err(),
            Error::Network
        );
    }
    #[tokio::test]
    async fn execute_permission_and_unsupported_versions_are_distinct_and_recheck_recovers() {
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("dgl-local-tool-fixture-{}", std::process::id()));
        std::fs::write(&path, "#!/bin/sh\nprintf 'tmux 1.0\\n'\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config = LocalConfig {
            executable: path.clone(),
            ..Default::default()
        };
        assert!(matches!(
            config.clone().detect().await,
            Err(Error::LocalToolPermission)
        ));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(
            config.clone().detect().await,
            Err(Error::LocalToolUnsupported)
        ));
        std::fs::write(&path, "#!/bin/sh\nif [ \"$1\" = '-V' ]; then printf 'tmux 3.7c\\n'; else printf 'capture-pane refresh-client send-keys list-sessions\\n'; fi\n").unwrap();
        assert!(config.detect().await.is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
