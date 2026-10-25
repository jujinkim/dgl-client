use std::{
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::Shell::{
            FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FileOpenDialog,
            IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
        },
    },
    core::{HRESULT, PCWSTR},
};

pub fn select(
    directory: Option<PathBuf>,
    owner: Option<isize>,
) -> windows::core::Result<Option<PathBuf>> {
    // All COM objects are released before the matching CoUninitialize.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    let _apartment = Apartment;
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            dialog.GetOptions()? | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM,
        )?;
        let title: Vec<u16> = dgl_core::i18n::t("개인키 파일 선택")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let prompt: Vec<u16> = dgl_core::i18n::t("가져오기")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        dialog.SetTitle(PCWSTR(title.as_ptr()))?;
        dialog.SetOkButtonLabel(PCWSTR(prompt.as_ptr()))?;
        // No extension filter: id_ed25519/id_rsa and other extensionless keys
        // remain selectable. SetFolder honors the user's explicit directory.
        if let Some(directory) = directory {
            let directory: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(directory.as_ptr()), None)?;
            dialog.SetFolder(&item)?;
        }
        if let Err(error) = dialog.Show(owner.map(|hwnd| HWND(hwnd as *mut _))) {
            if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error);
        }
        let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let result = PathBuf::from(std::ffi::OsString::from_wide(path.as_wide()));
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(result))
    }
}
