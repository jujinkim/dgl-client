use ::windows::{
    Foundation::TypedEventHandler,
    UI::ViewManagement::{AccessibilitySettings, UIColorType, UISettings},
    Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        },
        System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Appearance {
    settings: Option<UISettings>,
    accessibility: Option<AccessibilitySettings>,
    color_token: Option<i64>,
    contrast_token: Option<i64>,
    dirty: Arc<AtomicBool>,
    runtime_initialized: bool,
}
impl Appearance {
    pub fn new() -> Self {
        let dirty = Arc::new(AtomicBool::new(true));
        // GPUI initializes OLE as STA. Initialize WinRT in the same apartment
        // and balance even S_FALSE, without taking ownership of GPUI's OLE init.
        let runtime_initialized = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.is_ok();
        let settings = runtime_initialized
            .then(|| UISettings::new().ok())
            .flatten();
        let accessibility = runtime_initialized
            .then(|| AccessibilitySettings::new().ok())
            .flatten();
        let flag = dirty.clone();
        let color_token = settings.as_ref().and_then(|s| {
            s.ColorValuesChanged(&TypedEventHandler::new(move |_, _| {
                flag.store(true, Ordering::Release);
                Ok(())
            }))
            .ok()
        });
        let flag = dirty.clone();
        let contrast_token = accessibility.as_ref().and_then(|s| {
            s.HighContrastChanged(&TypedEventHandler::new(move |_, _| {
                flag.store(true, Ordering::Release);
                Ok(())
            }))
            .ok()
        });
        Self {
            settings,
            accessibility,
            color_token,
            contrast_token,
            dirty,
            runtime_initialized,
        }
    }
    pub fn dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
    pub fn refresh(&self, hwnd: isize, dark: bool, background: u32) -> Option<u32> {
        let high_contrast = self
            .accessibility
            .as_ref()
            .and_then(|s| s.HighContrast().ok())
            .unwrap_or(true);
        // Windows 11 supports these attributes. Windows 10 returns E_INVALIDARG
        // for unsupported attributes and retains its native frame unchanged.
        let default = 0xffffffffu32; // DWMWA_COLOR_DEFAULT, restores forced colors.
        let bgr = ((background & 255) << 16) | (background & 0xff00) | (background >> 16);
        let color = if high_contrast { default } else { bgr };
        let text = if high_contrast {
            default
        } else if dark {
            0x00e0dcda
        } else {
            0x002b2624
        };
        let dark: i32 = i32::from(dark && !high_contrast);
        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as _,
                4,
            );
            for (attribute, value) in [
                (DWMWA_CAPTION_COLOR, color),
                (DWMWA_BORDER_COLOR, color),
                (DWMWA_TEXT_COLOR, text),
            ] {
                let _ = DwmSetWindowAttribute(hwnd, attribute, &value as *const _ as _, 4);
            }
        }
        let c = self
            .settings
            .as_ref()?
            .GetColorValue(UIColorType::Accent)
            .ok()?;
        Some(((c.R as u32) << 16) | ((c.G as u32) << 8) | c.B as u32)
    }
}
impl Drop for Appearance {
    fn drop(&mut self) {
        if let (Some(s), Some(t)) = (&self.settings, self.color_token) {
            let _ = s.RemoveColorValuesChanged(t);
        }
        if let (Some(s), Some(t)) = (&self.accessibility, self.contrast_token) {
            let _ = s.RemoveHighContrastChanged(t);
        }
        self.settings.take();
        self.accessibility.take();
        if self.runtime_initialized {
            unsafe { RoUninitialize() };
        }
    }
}
