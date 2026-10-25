use gpui::{App, Task, Window};
use std::path::PathBuf;

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
        use std::{
            ffi::{CStr, CString, c_char, c_void},
            os::unix::ffi::OsStrExt,
        };
        unsafe extern "C" {
            fn dgl_select_private_key(
                directory: *const c_char,
                context: *mut c_void,
                complete: extern "C" fn(*mut c_void, *const c_char),
            );
        }
        extern "C" fn complete(context: *mut c_void, path: *const c_char) {
            // The native completion runs once. Copy the temporary UTF-8 path
            // before returning to AppKit, then release the boxed sender.
            let sender =
                unsafe { Box::from_raw(context.cast::<tokio::sync::oneshot::Sender<Selection>>()) };
            let path = (!path.is_null()).then(|| {
                PathBuf::from(
                    unsafe { CStr::from_ptr(path) }
                        .to_string_lossy()
                        .into_owned(),
                )
            });
            let _ = sender.send(Ok(path));
        }
        let _ = window;
        let directory = directory.and_then(|path| CString::new(path.as_os_str().as_bytes()).ok());
        // Called on GPUI's UI thread; AppKit presents asynchronously.
        unsafe {
            dgl_select_private_key(
                directory.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                Box::into_raw(Box::new(sender)).cast(),
                complete,
            );
        }
    }
    cx.foreground_executor()
        .spawn(async move { receiver.await.unwrap_or(Err(())) })
}
