//! Finder accepts file promises, so network I/O starts after the user drops.
//! File-URL receivers require completed local files before the native drag begins.
use super::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod staged;
#[cfg(target_os = "macos")]
pub(super) use macos::Outgoing;
#[cfg(not(target_os = "macos"))]
pub(super) use staged::Outgoing;

#[cfg(target_os = "linux")]
pub(crate) use staged::LinuxDrag;

impl Desktop {
    pub(super) fn drag_files(
        &mut self,
        id: FileId,
        path: &str,
        name: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.files.get_mut(&id).unwrap();
        if !tab.folders.select_for_drag(path, name) {
            return;
        }
        tab.error = None;
        let sources = tab.folders.selection();
        if let Err(error) = tab.outgoing.begin(
            sources,
            tab.browser.clone(),
            self.runtime.clone(),
            &mut tab.jobs,
            w,
        ) {
            tab.error = Some(error);
        } else {
            tab.jobs_open = true;
        }
        cx.notify();
    }
}
