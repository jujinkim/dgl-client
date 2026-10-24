//! Keep IME-owned keystrokes out of GPUI 0.2.2's normal key translation.
//!
//! GPUI maps VK_PROCESSKEY back through ImmGetVirtualKey and ToUnicode before
//! deciding whether to call TranslateMessage. That reprocesses a key which the
//! IME already owns, including the first key before a marked range exists.
//! Subclass only our window; ordinary shortcuts and GPUI's composition callbacks
//! remain in place. Remove this shim when the pinned GPUI handles these cases.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::{Ime::ISC_SHOWUICOMPOSITIONWINDOW, KeyboardAndMouse::VK_PROCESSKEY},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            GetMessagePos, GetMessageTime, MSG, TranslateMessage, WM_IME_SETCONTEXT, WM_KEYDOWN,
            WM_KEYUP, WM_NCDESTROY,
        },
    },
};

const SUBCLASS_ID: usize = 0x44474c49; // DGLI

pub fn install(window: &gpui::Window) -> std::io::Result<()> {
    let raw = HasWindowHandle::window_handle(window)
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .as_raw();
    let RawWindowHandle::Win32(handle) = raw else {
        return Err(std::io::Error::other("expected a Win32 window"));
    };
    // SAFETY: Called in the window's creation callback on its owning UI thread.
    // The callback has no borrowed state and removes itself on WM_NCDESTROY.
    unsafe { install_hwnd(handle.hwnd.get() as HWND) }
}

unsafe fn install_hwnd(hwnd: HWND) -> std::io::Result<()> {
    if unsafe { SetWindowSubclass(hwnd, Some(ime_subclass), SUBCLASS_ID, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn ime_owns_key(message: u32, key: WPARAM) -> bool {
    matches!(message, WM_KEYDOWN | WM_KEYUP) && key == VK_PROCESSKEY as usize
}

fn context_flags(flags: LPARAM) -> LPARAM {
    // GPUI draws preedit text itself. Let Windows keep the candidate UI, but
    // don't also activate the default composition UI for that same text.
    // https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-setcontext
    flags & !(ISC_SHOWUICOMPOSITIONWINDOW as LPARAM)
}

unsafe extern "system" fn ime_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    // SAFETY: The OS invokes this callback on the owning window thread and all
    // native handles/parameters are forwarded unchanged unless noted below.
    unsafe {
        if ime_owns_key(message, wparam) {
            if message == WM_KEYDOWN {
                let position = GetMessagePos();
                let msg = MSG {
                    hwnd,
                    message,
                    wParam: wparam,
                    lParam: lparam,
                    time: GetMessageTime() as u32,
                    pt: windows_sys::Win32::Foundation::POINT {
                        x: position as u16 as i16 as i32,
                        y: (position >> 16) as u16 as i16 as i32,
                    },
                };
                // Exactly one translation; do not redispatch as an app shortcut
                // or let GPUI translate the original IME-owned key a second time.
                TranslateMessage(&msg);
            }
            return 0;
        }
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(ime_subclass), id);
        }
        let lparam = if message == WM_IME_SETCONTEXT {
            context_flags(lparam)
        } else {
            lparam
        };
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, ptr::null_mut};
    use windows_sys::Win32::UI::{
        Input::Ime::{ISC_SHOWUIALL, ISC_SHOWUICANDIDATEWINDOW},
        Shell::GetWindowSubclass,
        WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, WM_IME_COMPOSITION, WS_POPUP,
        },
    };

    thread_local! {
        static RECEIVED: Cell<(u32, WPARAM, LPARAM)> = const { Cell::new((0, 0, 0)) };
    }

    unsafe extern "system" fn record(
        _: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _: usize,
        _: usize,
    ) -> LRESULT {
        RECEIVED.set((message, wparam, lparam));
        17
    }

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    // Exercise the real subclass chain, rather than only testing a copy of the
    // message-routing condition. No active desktop focus or user profile needed.
    #[test]
    fn native_ime_routing_preserves_composition_candidates_and_shortcuts() {
        unsafe {
            let window = TestWindow(CreateWindowExW(
                0,
                [83u16, 84, 65, 84, 73, 67, 0].as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                100,
                100,
                null_mut(),
                null_mut(),
                null_mut(),
                std::ptr::null(),
            ));
            assert!(!window.0.is_null());
            assert_ne!(SetWindowSubclass(window.0, Some(record), 1, 0), 0);
            install_hwnd(window.0).unwrap();
            // Installing twice must not introduce another translation layer.
            install_hwnd(window.0).unwrap();
            for message in [WM_KEYDOWN, WM_KEYUP] {
                RECEIVED.set((0, 0, 0));
                assert_eq!(
                    SendMessageW(window.0, message, VK_PROCESSKEY as usize, 0),
                    0
                );
                assert_eq!(RECEIVED.get(), (0, 0, 0));
                assert_eq!(SendMessageW(window.0, message, b'A' as usize, 0), 17);
                assert_eq!(RECEIVED.get(), (message, b'A' as usize, 0));
            }
            SendMessageW(window.0, WM_IME_SETCONTEXT, 1, ISC_SHOWUIALL as LPARAM);
            let flags = RECEIVED.get().2;
            assert_eq!(flags & ISC_SHOWUICOMPOSITIONWINDOW as LPARAM, 0);
            assert_ne!(flags & ISC_SHOWUICANDIDATEWINDOW as LPARAM, 0);
            assert_eq!(
                flags,
                (ISC_SHOWUIALL & !ISC_SHOWUICOMPOSITIONWINDOW) as LPARAM
            );
            SendMessageW(window.0, WM_IME_COMPOSITION, 0, 0x808);
            assert_eq!(RECEIVED.get(), (WM_IME_COMPOSITION, 0, 0x808));
            SendMessageW(window.0, WM_NCDESTROY, 0, 0);
            let mut data = 0;
            assert_eq!(
                GetWindowSubclass(window.0, Some(ime_subclass), SUBCLASS_ID, &mut data),
                0
            );
            RemoveWindowSubclass(window.0, Some(record), 1);
        }
    }
}
