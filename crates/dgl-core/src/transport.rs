//! Actor transport: SSH channels and owned local control clients share the same contract.
use crate::{
    Error, Result,
    command::Command,
    local::{Local, Multiplexer, Process},
    model::Size,
    ssh,
};
use russh::ChannelMsg;
pub(crate) enum Transport {
    Ssh(ssh::Ssh),
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
        matches!(self, Self::Local(l) if l.config.multiplexer == Multiplexer::Psmux)
    }
    pub async fn execute_tmux(&self, command: &Command) -> Result<(u32, Vec<u8>)> {
        match self {
            Self::Ssh(s) => s.execute_tmux(command).await,
            Self::Local(l) => l.execute(command).await,
        }
    }
    pub async fn control(&self, session: &str, ignore: bool) -> Result<Stream> {
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
        if let Self::Ssh(s) = self {
            s.close().await;
        }
    }
}
