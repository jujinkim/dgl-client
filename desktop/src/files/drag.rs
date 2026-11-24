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
        name: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.files.get_mut(&id).unwrap();
        if !tab.selected.contains(name) {
            tab.selected.clear();
            tab.selected.insert(name.to_owned());
        }
        let sources = tab.selection();
        let dirs = tab
            .selected
            .iter()
            .map(|name| {
                tab.listing
                    .entries
                    .iter()
                    .any(|e| &e.name == name && e.directory)
            })
            .collect();
        if let Err(error) = tab.outgoing.begin(
            sources,
            tab.browser.clone(),
            self.runtime.clone(),
            &mut tab.jobs,
            w,
            dirs,
        ) {
            tab.error = Some(error);
        }
        cx.notify();
    }
}
