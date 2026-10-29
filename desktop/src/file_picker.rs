use gpui::{App, Task, Window};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub type Selection = Result<Option<PathBuf>, ()>;

pub fn select(directory: Option<PathBuf>, window: &Window, cx: &App) -> Task<Selection> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let owner = HasWindowHandle::window_handle(window)
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            });
        // A dedicated STA keeps the native dialog off GPUI's executor and avoids
        // inheriting a worker thread's COM apartment. Only an HWND crosses threads.
        let _ = std::thread::Builder::new()
            .name("dgl-key-picker".into())
            .spawn(move || {
                let _ = sender.send(windows::select(directory, owner).map_err(|_| ()));
            });
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        macos::select(directory, sender);
    }
    cx.foreground_executor()
        .spawn(async move { receiver.await.unwrap_or(Err(())) })
}
