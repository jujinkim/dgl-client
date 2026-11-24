use ::windows::Win32::{
    Foundation::HWND,
    System::{Com::IDataObject, Ole::DROPEFFECT_COPY},
    UI::Shell::*,
};
use ::windows::core::PCWSTR;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::os::windows::ffi::OsStrExt;
use std::{path::PathBuf, sync::Arc};

pub(super) fn start(
    w: &impl HasWindowHandle,
    paths: Vec<PathBuf>,
    _dir: Arc<tempfile::TempDir>,
) -> Result<bool, String> {
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(w)
        .map_err(|e| e.to_string())?
        .as_raw()
    else {
        return Err("Missing HWND".into());
    };
    if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1) } >= 0 {
        return Ok(false);
    }
    // GPUI initializes OLE on this UI thread. The Shell data object offers CF_HDROP.
    unsafe {
        let mut ids = Vec::new();
        for path in paths {
            let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let id = ILCreateFromPathW(PCWSTR(wide.as_ptr()));
            if id.is_null() {
                for id in ids {
                    ILFree(Some(id));
                }
                return Err(format!("Cannot drag {}", path.display()));
            }
            ids.push(id.cast_const());
        }
        let array = SHCreateShellItemArrayFromIDLists(&ids);
        for id in ids {
            ILFree(Some(id));
        }
        let data: IDataObject = array
            .and_then(|a| a.BindToHandler(None, &BHID_DataObject))
            .map_err(|e| e.to_string())?;
        SHDoDragDrop(
            Some(HWND(handle.hwnd.get() as *mut _)),
            &data,
            None,
            DROPEFFECT_COPY,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(true)
}
