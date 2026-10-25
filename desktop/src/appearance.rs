//! Native appearance is refreshed from a short UI timer on notification,
//! activation or theme changes. Rendering only reads the cached RGB value.
use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "windows")]
mod windows;

pub struct Appearance {
    #[cfg(target_os = "macos")]
    observer: *mut std::ffi::c_void,
    #[cfg(target_os = "windows")]
    native: windows::Appearance,
    gate: RefreshGate,
    pub dark: bool,
    pub accent: Option<u32>,
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn dgl_appearance_create() -> *mut std::ffi::c_void;
    fn dgl_appearance_dirty(observer: *mut std::ffi::c_void) -> bool;
    fn dgl_appearance_destroy(observer: *mut std::ffi::c_void);
    fn dgl_appearance_system_dark() -> bool;
    fn dgl_appearance_update(
        view: *mut std::ffi::c_void,
        dark: bool,
        system: bool,
        background: u32,
    ) -> i64;
}
impl Appearance {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            observer: unsafe { dgl_appearance_create() },
            #[cfg(target_os = "windows")]
            native: windows::Appearance::new(),
            gate: RefreshGate::default(),
            dark: true,
            accent: None,
        }
    }
    pub fn refresh(&mut self, window: &Window, theme: &str) -> bool {
        let active = window.is_window_active();
        #[cfg(target_os = "macos")]
        let dirty = unsafe { dgl_appearance_dirty(self.observer) };
        #[cfg(target_os = "windows")]
        let dirty = self.native.dirty();
        let window_dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        if !self.gate.changed(theme, active, window_dark, dirty) {
            return false;
        }
        #[cfg(target_os = "macos")]
        let system_dark = unsafe { dgl_appearance_system_dark() };
        #[cfg(target_os = "windows")]
        let system_dark = window_dark;
        let dark = match theme {
            "dark" => true,
            "light" => false,
            _ => system_dark,
        };
        let previous_dark = self.dark;
        self.dark = dark;
        let background = crate::theme::Palette::new(dark).bg;
        let previous = self.accent;
        if let Ok(handle) = HasWindowHandle::window_handle(window) {
            match handle.as_raw() {
                #[cfg(target_os = "macos")]
                RawWindowHandle::AppKit(h) => {
                    let value = unsafe {
                        dgl_appearance_update(
                            h.ns_view.as_ptr(),
                            dark,
                            theme == "system",
                            background,
                        )
                    };
                    self.accent = (value >= 0).then_some(value as u32);
                }
                #[cfg(target_os = "windows")]
                RawWindowHandle::Win32(h) => {
                    self.accent = self.native.refresh(h.hwnd.get(), dark, background)
                }
                _ => self.accent = None,
            }
        } else {
            self.accent = None;
        }
        previous != self.accent || previous_dark != dark
    }
}
#[cfg(target_os = "macos")]
impl Drop for Appearance {
    fn drop(&mut self) {
        unsafe { dgl_appearance_destroy(self.observer) };
    }
}

#[derive(Default)]
struct RefreshGate {
    last: Option<(String, bool, bool)>,
}
impl RefreshGate {
    fn changed(&mut self, theme: &str, active: bool, window_dark: bool, dirty: bool) -> bool {
        let previous = self.last.replace((theme.into(), active, window_dark));
        dirty || !previous.is_some_and(|(t, a, d)| t == theme && (a || !active) && d == window_dark)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refreshes_at_start_notification_theme_change_and_return_only() {
        let mut gate = RefreshGate::default();
        assert!(gate.changed("system", true, false, false));
        assert!(!gate.changed("system", true, false, false));
        assert!(gate.changed("system", true, false, true));
        assert!(!gate.changed("system", false, false, false));
        assert!(!gate.changed("system", false, false, false));
        assert!(gate.changed("system", true, false, false));
        assert!(gate.changed("dark", true, false, false));
        assert!(gate.changed("system", true, true, false));
        assert!(!gate.changed("system", true, true, false));
    }
}
