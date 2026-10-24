use dgl_core::agent::Notice;
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

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{authorize, initialize, show};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn dgl_notifications_init(callback: extern "C" fn(*const std::ffi::c_char)) -> bool;
    fn dgl_notifications_authorize();
    fn dgl_notifications_show(
        id: *const std::ffi::c_char,
        title: *const std::ffi::c_char,
        body: *const std::ffi::c_char,
    );
}
#[cfg(target_os = "macos")]
extern "C" fn on_click(id: *const std::ffi::c_char) {
    if !id.is_null() {
        clicked(
            unsafe { std::ffi::CStr::from_ptr(id) }
                .to_string_lossy()
                .into_owned(),
        );
    }
}
#[cfg(target_os = "macos")]
pub fn initialize() -> bool {
    unsafe { dgl_notifications_init(on_click) }
}
#[cfg(target_os = "macos")]
pub fn authorize() -> bool {
    if !initialize() {
        return false;
    }
    unsafe {
        dgl_notifications_authorize();
    }
    true
}
#[cfg(target_os = "macos")]
pub fn show(notice: &Notice) -> bool {
    if !initialize() {
        return false;
    }
    let values = [
        notice.id(),
        format!(
            "{:?} · {}{}",
            notice.event.agent,
            notice.event.pane,
            if notice.event.actor != "parent" {
                " · 서브에이전트"
            } else {
                ""
            }
        ),
        notice.event.kind.label().to_string(),
    ];
    let c: Option<Vec<_>> = values
        .into_iter()
        .map(|s| std::ffi::CString::new(s).ok())
        .collect();
    let Some(c) = c else {
        return false;
    };
    unsafe {
        dgl_notifications_show(c[0].as_ptr(), c[1].as_ptr(), c[2].as_ptr());
    }
    true
}
