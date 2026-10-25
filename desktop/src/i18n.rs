//! Platform UI language discovery; translations live in the shared engine catalog.
#[cfg(target_os = "macos")]
pub fn system_locale() -> String {
    unsafe extern "C" {
        fn dgl_system_language(buffer: *mut std::ffi::c_char, length: usize);
    }
    let mut buffer = [0u8; 128];
    unsafe {
        dgl_system_language(buffer.as_mut_ptr().cast(), buffer.len());
    }
    String::from_utf8_lossy(&buffer[..buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len())])
        .into_owned()
}
#[cfg(target_os = "windows")]
pub fn system_locale() -> String {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    let mut count = 0;
    let mut length = 0;
    unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut length,
        );
        let mut buffer = vec![0u16; length as usize];
        if length > 0
            && GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                buffer.as_mut_ptr(),
                &mut length,
            ) != 0
        {
            return String::from_utf16_lossy(
                &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
            );
        }
    }
    "en".into()
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "en".into())
}
