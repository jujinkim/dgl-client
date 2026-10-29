use super::Region;
use gpui::Bounds;
use std::ffi::{CString, c_char, c_void};

unsafe extern "C" {
    fn dgl_tooltip_create(view: *mut c_void) -> *mut c_void;
    fn dgl_tooltip_set(manager: *mut c_void, x: f64, y: f64, w: f64, h: f64, text: *const c_char);
    fn dgl_tooltip_destroy(manager: *mut c_void);
}

pub(super) struct Tooltips(*mut c_void);
impl Tooltips {
    pub fn new(view: *mut c_void) -> Option<Self> {
        let manager = unsafe { dgl_tooltip_create(view) };
        (!manager.is_null()).then_some(Self(manager))
    }
    pub fn set(&mut self, region: Option<&Region>) {
        let text = region.map(|r| CString::new(r.text.replace('\0', "")).unwrap());
        let bounds = region.map_or(Bounds::default(), |r| r.bounds);
        unsafe {
            dgl_tooltip_set(
                self.0,
                f32::from(bounds.left()) as f64,
                f32::from(bounds.top()) as f64,
                f32::from(bounds.size.width) as f64,
                f32::from(bounds.size.height) as f64,
                text.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            );
        }
    }
}
impl Drop for Tooltips {
    fn drop(&mut self) {
        unsafe { dgl_tooltip_destroy(self.0) };
    }
}
