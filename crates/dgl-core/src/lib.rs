pub mod accent;
pub mod agent;
pub mod command;
pub mod connection;
pub mod flow;
pub mod i18n;
pub mod input;
pub mod model;
pub mod protocol;
pub mod ssh;
pub mod sync;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error,
)]
pub enum Error {
    #[error("Authentication")]
    Authentication,
    #[error("HostKeyUnknown")]
    HostKeyUnknown,
    #[error("HostKeyChanged")]
    HostKeyChanged,
    #[error("Network")]
    Network,
    #[error("Protocol")]
    Protocol,
    #[error("MissingSession")]
    MissingSession,
    #[error("Storage")]
    Storage,
    #[error("InvalidArgument")]
    InvalidArgument,
    #[error("QueueFull")]
    QueueFull,
    #[error("NotReady")]
    NotReady,
    #[error("StaleGeneration")]
    StaleGeneration,
    #[error("Cancelled")]
    Cancelled,
    #[error("Closed")]
    Closed,
}
pub type Result<T> = std::result::Result<T, Error>;
