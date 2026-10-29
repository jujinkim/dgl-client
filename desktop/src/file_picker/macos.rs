use super::Selection;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};
use tokio::sync::oneshot::Sender;

unsafe extern "C" {
    fn dgl_select_private_key(
        directory: *const c_char,
        title: *const c_char,
        prompt: *const c_char,
        context: *mut c_void,
        complete: extern "C" fn(*mut c_void, *const c_char),
    );
}

extern "C" fn complete(context: *mut c_void, path: *const c_char) {
    // The native completion runs once. Copy the temporary UTF-8 path
    // before returning to AppKit, then release the boxed sender.
    let sender = unsafe { Box::from_raw(context.cast::<Sender<Selection>>()) };
    let path = (!path.is_null()).then(|| {
        PathBuf::from(
            unsafe { CStr::from_ptr(path) }
                .to_string_lossy()
                .into_owned(),
        )
    });
    let _ = sender.send(Ok(path));
}

/// Called on GPUI's UI thread; AppKit presents asynchronously and completes `sender`.
pub fn select(directory: Option<PathBuf>, sender: Sender<Selection>) {
    let directory = directory.and_then(|path| CString::new(path.as_os_str().as_bytes()).ok());
    let title = CString::new(dgl_core::i18n::t("개인키 파일 선택")).unwrap();
    let prompt = CString::new(dgl_core::i18n::t("가져오기")).unwrap();
    unsafe {
        dgl_select_private_key(
            directory.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            title.as_ptr(),
            prompt.as_ptr(),
            Box::into_raw(Box::new(sender)).cast(),
            complete,
        );
    }
}
