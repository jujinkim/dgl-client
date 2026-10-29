use std::ffi::c_void;

unsafe extern "C" {
    fn dgl_appearance_create() -> *mut c_void;
    fn dgl_appearance_dirty(observer: *mut c_void) -> bool;
    fn dgl_appearance_destroy(observer: *mut c_void);
    fn dgl_appearance_system_dark() -> bool;
    fn dgl_appearance_update(view: *mut c_void, dark: bool, system: bool, background: u32) -> i64;
}

/// Owns the AppKit appearance/accent observer for the window's lifetime.
pub struct Appearance {
    observer: *mut c_void,
}
impl Appearance {
    pub fn new() -> Self {
        Self {
            observer: unsafe { dgl_appearance_create() },
        }
    }
    pub fn dirty(&self) -> bool {
        unsafe { dgl_appearance_dirty(self.observer) }
    }
    pub fn system_dark(&self) -> bool {
        unsafe { dgl_appearance_system_dark() }
    }
    /// Applies the window appearance and returns the system accent, if any.
    pub fn refresh(
        &self,
        view: *mut c_void,
        dark: bool,
        system: bool,
        background: u32,
    ) -> Option<u32> {
        let value = unsafe { dgl_appearance_update(view, dark, system, background) };
        (value >= 0).then_some(value as u32)
    }
}
impl Drop for Appearance {
    fn drop(&mut self) {
        unsafe { dgl_appearance_destroy(self.observer) };
    }
}
