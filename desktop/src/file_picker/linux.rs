//! XDG desktop portal key picker. GPUI's own path prompt cannot set a start folder,
//! so this calls the portal directly with the folder and the parent window.
use super::Selection;
use ashpd::{WindowIdentifier, desktop::file_chooser::SelectedFiles};
use gpui::{App, Task, Window};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::path::PathBuf;

pub fn select(directory: Option<PathBuf>, window: &Window, cx: &App) -> Task<Selection> {
    let handle = HasWindowHandle::window_handle(window)
        .ok()
        .map(|h| h.as_raw());
    let display = HasDisplayHandle::display_handle(window)
        .ok()
        .map(|h| h.as_raw());
    cx.foreground_executor().spawn(async move {
        let identifier = match handle {
            Some(handle) => WindowIdentifier::from_raw_handle(&handle, display.as_ref()).await,
            None => None,
        };
        let request = SelectedFiles::open_file()
            .identifier(identifier)
            .modal(true)
            .title(dgl_core::i18n::t("host.choose_private_key_file"))
            .multiple(false)
            // A folder the portal cannot encode opens at the desktop default instead.
            .current_folder::<PathBuf>(directory.filter(|d| d.is_dir()))
            .map_err(|_| ())?;
        match request.send().await.map_err(|_| ())?.response() {
            Ok(files) => Ok(files.uris().iter().find_map(|uri| uri.to_file_path().ok())),
            Err(ashpd::Error::Response(_)) => Ok(None),
            Err(_) => Err(()),
        }
    })
}
