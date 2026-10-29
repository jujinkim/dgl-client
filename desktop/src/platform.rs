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

pub fn user_home() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let variable = "HOME";
    #[cfg(target_os = "windows")]
    let variable = "USERPROFILE";
    std::env::var_os(variable)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

pub fn ssh_directory_placeholder() -> String {
    #[cfg(target_os = "macos")]
    let (variable, fallback) = ("HOME", "~/.ssh/");
    #[cfg(target_os = "windows")]
    let (variable, fallback) = ("USERPROFILE", "%USERPROFILE%\\.ssh\\");
    std::env::var_os(variable)
        .filter(|path| !path.is_empty())
        .map(|path| {
            format!(
                "{}{}",
                PathBuf::from(path).join(".ssh").display(),
                std::path::MAIN_SEPARATOR
            )
        })
        .unwrap_or_else(|| fallback.into())
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
    "Windows DPAPI"
} else {
    "macOS Keychain"
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

/// The app's keyboard shortcuts, one table for every handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppShortcut {
    NewHost,
    NewSession,
    CloseTab,
    Settings,
    Refresh,
    Copy,
    Paste,
    /// Bound as a key action so it also works from dialogs; listed to keep it reserved.
    Quit,
    /// 1-based; only 1-9 exist.
    SelectTab(usize),
}

pub fn app_shortcut_for(keystroke: &gpui::Keystroke) -> Option<AppShortcut> {
    use AppShortcut::*;
    if !app_shortcut(&keystroke.modifiers) {
        return None;
    }
    let tab = |n: &str| {
        n.parse()
            .ok()
            .filter(|n| (1..=9).contains(n))
            .map(SelectTab)
    };
    let key = keystroke.key.as_str();
    if cfg!(target_os = "windows") {
        match key {
            "n" => Some(NewHost),
            "s" => Some(NewSession),
            "w" => Some(CloseTab),
            "," | "p" => Some(Settings),
            "r" => Some(Refresh),
            "c" => Some(Copy),
            "v" => Some(Paste),
            "q" => Some(Quit),
            _ => key.strip_prefix('f').and_then(tab),
        }
    } else {
        match key {
            "n" if keystroke.modifiers.shift => Some(NewSession),
            "n" => Some(NewHost),
            "w" => Some(CloseTab),
            "," => Some(Settings),
            "r" => Some(Refresh),
            "c" => Some(Copy),
            "v" => Some(Paste),
            "q" => Some(Quit),
            _ => tab(key),
        }
    }
}

/// Keys the terminal leaves to the app. macOS keeps every Command chord; Windows keeps
/// only the table's Ctrl+Shift chords, so arrows and F10-F12 reach the remote application.
pub fn terminal_app_shortcut(key: &gpui::Keystroke) -> bool {
    if cfg!(target_os = "windows") {
        app_shortcut_for(key).is_some()
    } else {
        app_shortcut(&key.modifiers)
    }
}

#[cfg(test)]
mod tests {
    use super::{AppShortcut::*, app_shortcut_for, terminal_app_shortcut};
    use gpui::Keystroke;
    fn key(text: &str) -> Keystroke {
        Keystroke::parse(text).unwrap()
    }
    #[test]
    fn one_table_reserves_tab_keys_one_to_nine_only() {
        let chord = if cfg!(target_os = "windows") {
            "ctrl-shift-"
        } else {
            "cmd-"
        };
        let tab = |n: usize| {
            if cfg!(target_os = "windows") {
                format!("{chord}f{n}")
            } else {
                format!("{chord}{n}")
            }
        };
        for n in 1..=9 {
            assert_eq!(app_shortcut_for(&key(&tab(n))), Some(SelectTab(n)));
            assert!(terminal_app_shortcut(&key(&tab(n))));
        }
        assert_eq!(app_shortcut_for(&key(&format!("{chord}r"))), Some(Refresh));
        assert_eq!(app_shortcut_for(&key(&format!("{chord}c"))), Some(Copy));
        // The remote application keeps its own Ctrl chords.
        assert_eq!(app_shortcut_for(&key("ctrl-c")), None);
        assert!(!terminal_app_shortcut(&key("ctrl-c")));
        if cfg!(target_os = "windows") {
            // F10-F12 are not tabs 10-12: they and Ctrl+Shift+arrows reach the remote.
            for k in ["f10", "f11", "f12", "left"] {
                let k = key(&format!("{chord}{k}"));
                assert_eq!(app_shortcut_for(&k), None);
                assert!(!terminal_app_shortcut(&k));
            }
            assert_eq!(app_shortcut_for(&key("ctrl-shift-s")), Some(NewSession));
            assert_eq!(app_shortcut_for(&key("ctrl-shift-p")), Some(Settings));
        } else {
            assert_eq!(app_shortcut_for(&key("cmd-0")), None);
            assert_eq!(app_shortcut_for(&key("cmd-shift-n")), Some(NewSession));
            assert_eq!(app_shortcut_for(&key("cmd-n")), Some(NewHost));
            // Every other Command chord stays with the app, never the remote.
            assert_eq!(app_shortcut_for(&key("cmd-k")), None);
            assert!(terminal_app_shortcut(&key("cmd-k")));
        }
    }
}
