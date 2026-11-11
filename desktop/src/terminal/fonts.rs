//! Installed monospace families and the face the terminals paint with.
use gpui::{Font, Global, SharedString, TextSystem, font, px};

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{CStr, c_char};
    unsafe extern "C" {
        fn dgl_terminal_fonts_copy() -> *mut c_char;
        fn dgl_terminal_fonts_free(names: *mut c_char);
    }
    pub fn installed_families() -> Vec<String> {
        let names = unsafe { dgl_terminal_fonts_copy() };
        if names.is_null() {
            return Vec::new();
        }
        let list = unsafe { CStr::from_ptr(names) }
            .to_string_lossy()
            .lines()
            .map(str::to_owned)
            .collect();
        unsafe { dgl_terminal_fonts_free(names) };
        list
    }
}
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows as native;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod native {
    pub fn installed_families() -> Vec<String> {
        let mut command = std::process::Command::new("fc-list");
        if std::env::var_os("APPDIR").is_some() || std::env::var_os("APPIMAGE").is_some() {
            command
                .env_remove("LD_LIBRARY_PATH")
                .env_remove("LD_PRELOAD");
        }
        command
            .args([":spacing=mono", "family"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .flat_map(|s| s.split(',').map(str::trim))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Cell width per point of font size when the face cannot be measured.
pub const DEFAULT_WIDTH_RATIO: f32 = 0.61;
/// Cell height per point of font size, shared by every face.
pub const HEIGHT_RATIO: f32 = 1.4;

/// Sorted, distinct installed families whose printable ASCII shares one advance.
/// Enumerates the system font collection; call it off the UI thread.
pub fn installed_monospace() -> Vec<String> {
    let mut names: Vec<String> = native::installed_families()
        .into_iter()
        .filter(|name| valid_family(name))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

pub fn valid_family(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
}

/// One advance (within 1%) for every measured glyph.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn same_advance(advances: impl IntoIterator<Item = f64>) -> bool {
    let mut advances = advances.into_iter();
    let Some(first) = advances.next().filter(|w| *w > 0.) else {
        return false;
    };
    advances.all(|w| (w - first).abs() <= first * 0.01)
}

/// The family every terminal paints with and its measured cell width. Not synced:
/// another device may lack the family, so each device resolves its own setting.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalFace {
    pub family: SharedString,
    /// Empty when the platform default is in use.
    pub selected: SharedString,
    pub width_ratio: f32,
}
impl Global for TerminalFace {}
impl Default for TerminalFace {
    fn default() -> Self {
        Self {
            family: crate::platform::TERMINAL_FONT.into(),
            selected: SharedString::default(),
            width_ratio: DEFAULT_WIDTH_RATIO,
        }
    }
}
impl TerminalFace {
    /// The stored family when it is installed and monospace; otherwise the default.
    pub fn resolve(family: &str, text: &TextSystem) -> Self {
        if family.is_empty() || !valid_family(family) {
            return Self::default();
        }
        let requested = font(SharedString::from(family.to_owned()));
        let id = text.resolve_font(&requested);
        // A missing family resolves to a fallback; that is not the user's choice.
        if text
            .get_font_for_id(id)
            .is_none_or(|found| found.family != requested.family)
        {
            return Self::default();
        }
        let size = px(100.);
        let advance = |c| {
            text.advance(id, size, c)
                .map(|s| f64::from(f32::from(s.width)))
        };
        let measured: Result<Vec<f64>, _> =
            ['i', 'M', 'W', '0', '.'].into_iter().map(advance).collect();
        let Ok(measured) = measured else {
            return Self::default();
        };
        if !same_advance(measured.iter().copied()) {
            return Self::default();
        }
        let ratio = (measured[1] / 100.) as f32;
        if !(0.3..=1.2).contains(&ratio) {
            return Self::default();
        }
        Self {
            family: requested.family,
            selected: family.to_owned().into(),
            width_ratio: ratio,
        }
    }
    pub fn font(&self) -> Font {
        font(self.family.clone())
    }
    /// Cell width and height for this face at `font_size`.
    pub fn cell_size(&self, font_size: f32) -> (f32, f32) {
        (font_size * self.width_ratio, font_size * HEIGHT_RATIO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advances_must_match_within_one_percent() {
        assert!(same_advance([600., 600., 604.]));
        assert!(!same_advance([600., 600., 620.]));
        assert!(!same_advance([0., 0.]));
        assert!(!same_advance(std::iter::empty()));
    }
    #[test]
    fn family_names_reject_blank_and_control_text() {
        assert!(valid_family("JetBrains Mono"));
        assert!(!valid_family(" "));
        assert!(!valid_family("a\nb"));
        assert!(!valid_family(&"x".repeat(257)));
    }
    #[test]
    fn default_face_keeps_the_fixed_cell_ratio() {
        let face = TerminalFace::default();
        assert_eq!(face.cell_size(10.), (10. * 0.61, 10. * 1.4));
        assert_eq!(face.cell_size(28.), (28. * 0.61, 28. * 1.4));
        assert!(face.selected.is_empty());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_lists_bundled_monospace_families_only() {
        let fonts = installed_monospace();
        for family in ["Menlo", "Monaco"] {
            assert!(fonts.iter().any(|f| f == family), "{family}");
        }
        for family in ["Helvetica", "Times", "Apple Color Emoji", "Symbol"] {
            assert!(!fonts.iter().any(|f| f == family), "{family}");
        }
        assert!(
            fonts
                .windows(2)
                .all(|w| w[0].to_lowercase() <= w[1].to_lowercase())
        );
    }
}
