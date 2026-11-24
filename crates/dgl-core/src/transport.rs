//! Actor transport: SSH channels and owned local control clients share the same contract.
use crate::{
    Error, Result,
    command::Command,
    local::{Local, Multiplexer, Process},
    model::Size,
    ssh,
};
use russh::ChannelMsg;
#[derive(Clone)]
pub(crate) enum Transport {
    Ssh(std::sync::Arc<ssh::Ssh>),
    Local(Local),
}
pub(crate) enum Stream {
    Ssh(ssh::Stream),
    Local(Process),
}
impl From<ssh::Stream> for Stream {
    fn from(s: ssh::Stream) -> Self {
        Self::Ssh(s)
    }
}
impl Stream {
    pub async fn wait(&mut self) -> Option<ChannelMsg> {
        match self {
            Self::Ssh(s) => s.wait().await,
            Self::Local(s) => s
                .read()
                .await
                .map(|data| ChannelMsg::Data { data: data.into() }),
        }
    }
    pub async fn close(&self) -> Result<()> {
        match self {
            Self::Ssh(s) => s.close().await.map_err(|_| Error::Network),
            Self::Local(s) => s.close().await,
        }
    }
    pub async fn window_change(
        &self,
        columns: u32,
        rows: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        match self {
            Self::Ssh(s) => s
                .window_change(columns, rows, width, height)
                .await
                .map_err(|_| Error::Network),
            Self::Local(_) => Err(Error::NotReady),
        }
    }
    pub async fn write(&self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Ssh(s) => ssh::write(s, bytes).await,
            Self::Local(s) => s.write(bytes).await,
        }
    }
}
impl Transport {
    pub fn as_ssh(&self) -> Option<&ssh::Ssh> {
        if let Self::Ssh(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::Ssh(s) => s.tmux_version.as_deref().unwrap_or(""),
            Self::Local(l) => &l.version,
        }
    }
    pub fn psmux(&self) -> bool {
        match self {
            Self::Local(l) => l.config.multiplexer == Multiplexer::Psmux,
            Self::Ssh(s) => s.windows_host && s.tmux.is_some(),
        }
    }
    pub(crate) async fn execute_args(&self, args: &[String]) -> Result<(u32, Vec<u8>)> {
        match self {
            Self::Local(l) => l.execute_args(args).await,
            Self::Ssh(s) => {
                s.execute(&ssh::windows_process(
                    s.tmux.as_deref().ok_or(Error::NotReady)?,
                    args,
                    None,
                )?)
                .await
            }
        }
    }
    pub async fn execute_tmux(&self, command: &Command) -> Result<(u32, Vec<u8>)> {
        if self.psmux() {
            if command.wire == Command::inventory().wire {
                return Ok((0, crate::local::psmux::inventory(self).await?));
            }
            let mut args = command.argv()?;
            if let Some(i) = args.iter().position(|s| s == "-t")
                && let Some(id) = args.get(i + 1).filter(|s| s.starts_with(['@', '%']))
            {
                let (session, native) = crate::local::psmux::native(id)?;
                args[i + 1] = format!("${session}:{native}");
            }
            return self.execute_args(&args).await;
        }
        match self {
            Self::Ssh(s) => s.execute_tmux(command).await,
            Self::Local(l) => l.execute(command).await,
        }
    }
    /// Like `execute_tmux`, but output over `limit` bytes stops the command with
    /// `Error::Protocol`. Not for psmux.
    pub async fn execute_tmux_limited(
        &self,
        command: &Command,
        limit: usize,
    ) -> Result<(u32, Vec<u8>)> {
        match self {
            Self::Ssh(s) => s.execute_tmux_limited(command, limit).await,
            Self::Local(l) => l.execute_tmux_limited(command, limit).await,
        }
    }
    pub async fn control(&self, session: &str, ignore: bool) -> Result<Stream> {
        if self.psmux() {
            crate::command::target(session, '$')?;
            let cmd = Command::new(&crate::command::Operation::Sessions)?;
            let (code, bytes) = self.execute_args(&cmd.argv()?).await?;
            if code != 0 {
                return Err(Error::MissingSession);
            }
            let rows = bytes
                .split(|b| *b == b'\n')
                .filter(|line| !line.is_empty())
                .map(|line| line.strip_suffix(b"\r").unwrap_or(line).to_vec())
                .collect::<Vec<_>>();
            let sessions = crate::model::sessions(&rows)?;
            let name = &sessions
                .iter()
                .find(|row| row.id == session)
                .ok_or(Error::MissingSession)?
                .name;
            let args = [
                "-CC".into(),
                "attach-session".into(),
                "-t".into(),
                session.into(),
            ];
            return match self {
                Self::Local(l) => Ok(Stream::Local(l.spawn(&args, Some(name)).await?)),
                Self::Ssh(s) => Ok(Stream::Ssh(
                    s.process(&ssh::windows_process(
                        s.tmux.as_deref().ok_or(Error::NotReady)?,
                        &args,
                        Some(name),
                    )?)
                    .await?,
                )),
            };
        }
        match self {
            Self::Ssh(s) => s.control(session, ignore).await.map(Stream::Ssh),
            Self::Local(l) => l.control(session, ignore).await.map(Stream::Local),
        }
    }
    pub async fn shell(&self, size: Size) -> Result<Stream> {
        match self {
            Self::Ssh(s) => s.shell(size).await.map(Stream::Ssh),
            Self::Local(_) => Err(Error::NotReady),
        }
    }
    pub async fn check(&self) -> Result<(u32, Vec<u8>)> {
        match self {
            Self::Ssh(s) if s.windows_host => s.execute("cmd.exe /d /c exit 0").await,
            Self::Ssh(s) => s.execute(":").await,
            Self::Local(l) => l.execute_args(&["-V".into()]).await,
        }
    }
    pub fn abort(&self) {
        if let Self::Ssh(s) = self {
            s.abort();
        }
    }
    pub async fn close(self) {
        if let Self::Ssh(s) = self
            && let Some(s) = std::sync::Arc::into_inner(s)
        {
            s.close().await;
        }
    }
}
