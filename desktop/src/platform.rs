use dgl_core::{Error, Result};
use std::path::PathBuf;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
compile_error!("dgl-desktop currently supports macOS and Windows build targets");

pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("DGL_CANDIDATE_DIR").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    let (variable, suffix) = ("HOME", "Library/Application Support/dgl-rust-candidate-v1");
    #[cfg(target_os = "windows")]
    let (variable, suffix) = ("LOCALAPPDATA", "dgl-rust-candidate-v1");
    std::env::var_os(variable)
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(p).join(suffix))
        .ok_or(Error::Storage)
}

pub const UI_FONT: &str = if cfg!(target_os = "windows") {
    "Segoe UI"
} else {
    ".AppleSystemUIFont"
};
pub const TERMINAL_FONT: &str = if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "Menlo"
};
pub const STORAGE_LABEL: &str = if cfg!(target_os = "windows") {
    "Windows DPAPI · 후보 전용 저장소"
} else {
    "macOS Keychain · 후보 전용 저장소"
};
pub const EDIT_MODIFIER: &str = if cfg!(target_os = "windows") {
    "ctrl"
} else {
    "cmd"
};
pub const QUIT_KEY: &str = if cfg!(target_os = "windows") {
    "ctrl-shift-q"
} else {
    "cmd-q"
};
pub const SHORTCUT_HELP: &str = if cfg!(target_os = "windows") {
    "Ctrl+Shift+N 호스트 · Ctrl+Shift+S 세션 · Ctrl+Shift+W 연결 종료 · Ctrl+Shift+F1–F9 연결 전환 · Ctrl+Shift+R 새로고침"
} else {
    "⌘N 호스트 · ⇧⌘N 세션 · ⌘W 연결 종료 · ⌘1–9 연결 전환 · ⌘R 새로고침"
};

// Preserve Ctrl+C, Ctrl+V, Ctrl+R, etc. for the remote terminal on Windows.
pub fn app_shortcut(modifiers: &gpui::Modifiers) -> bool {
    if cfg!(target_os = "windows") {
        modifiers.control && modifiers.shift && !modifiers.alt && !modifiers.platform
    } else {
        modifiers.platform
    }
}
