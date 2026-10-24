#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::{delete, read, read_optional, write};
#[cfg(target_os = "windows")]
pub use windows::{delete, read, read_optional, write};

pub fn trust_ref(endpoint: &str) -> String {
    format!("host-key:{endpoint}")
}
