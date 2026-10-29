//! Native UserNotifications banners; clicks are queued for the UI timer.
use super::*;
use std::ffi::{CStr, CString, c_char};

unsafe extern "C" {
    fn dgl_notifications_init(callback: extern "C" fn(*const c_char)) -> bool;
    fn dgl_notifications_authorize();
    fn dgl_notifications_show(id: *const c_char, title: *const c_char, body: *const c_char);
}
extern "C" fn on_click(id: *const c_char) {
    if !id.is_null() {
        clicked(unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned());
    }
}
pub fn initialize() -> bool {
    unsafe { dgl_notifications_init(on_click) }
}
pub fn authorize() -> bool {
    if !initialize() {
        return false;
    }
    unsafe {
        dgl_notifications_authorize();
    }
    true
}
pub fn show(notice: &Notice) -> bool {
    if !initialize() {
        return false;
    }
    let values = [
        notice.id(),
        format!(
            "{} · {}{}",
            notice.event.agent.label(),
            notice.event.pane,
            if notice.event.actor != "parent" {
                t(" · 서브에이전트")
            } else {
                ""
            }
        ),
        notice.event.kind.label().to_string(),
    ];
    let c: Option<Vec<_>> = values.into_iter().map(|s| CString::new(s).ok()).collect();
    let Some(c) = c else {
        return false;
    };
    unsafe {
        dgl_notifications_show(c[0].as_ptr(), c[1].as_ptr(), c[2].as_ptr());
    }
    true
}
