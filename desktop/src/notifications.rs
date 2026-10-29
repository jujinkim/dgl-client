use dgl_core::agent::Notice;
use dgl_core::i18n::t;
use std::sync::{Mutex, OnceLock};
static CLICKED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
fn clicked(id: String) {
    CLICKED
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .push(id);
}
pub fn take_clicks() -> Vec<String> {
    std::mem::take(&mut *CLICKED.get_or_init(Default::default).lock().unwrap())
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{authorize, initialize, show};
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{authorize, initialize, show};
