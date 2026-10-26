use super::Region;
use gpui::{Pixels, Point};
use std::{
    mem::size_of,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    UI::{Controls::*, WindowsAndMessaging::*},
};

pub(super) struct Tooltips {
    parent: HWND,
    tooltip: HWND,
    text: Vec<u16>,
    tool: TTTOOLINFOW,
    registered: bool,
}
impl Tooltips {
    pub fn new(parent: isize) -> Option<Self> {
        unsafe {
            let classes = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES,
            };
            if InitCommonControlsEx(&classes) == 0 {
                return None;
            }
            let parent = parent as HWND;
            let tooltip = CreateWindowExW(
                WS_EX_TOPMOST,
                TOOLTIPS_CLASSW,
                null(),
                WS_POPUP | TTS_NOPREFIX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                parent,
                null_mut(),
                null_mut(),
                null(),
            );
            if tooltip.is_null() {
                return None;
            }
            SetWindowPos(
                tooltip,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            Some(Self {
                parent,
                tooltip,
                text: vec![],
                tool: TTTOOLINFOW {
                    cbSize: size_of::<TTTOOLINFOW>() as u32,
                    hwnd: parent,
                    ..std::mem::zeroed()
                },
                registered: false,
            })
        }
    }
    pub fn set(&mut self, region: Option<&Region>) {
        unsafe {
            if self.registered {
                SendMessageW(self.tooltip, TTM_POP, 0, 0);
                SendMessageW(
                    self.tooltip,
                    TTM_DELTOOLW,
                    0,
                    &self.tool as *const _ as isize,
                );
                self.registered = false;
            }
            self.text.clear();
            if let Some(r) = region {
                self.text
                    .extend(r.text.replace('\0', "").encode_utf16().chain(Some(0)));
                self.tool.uId = r.id as usize;
                self.tool.rect = RECT {
                    left: (f32::from(r.bounds.left()) * r.scale).floor() as i32,
                    top: (f32::from(r.bounds.top()) * r.scale).floor() as i32,
                    right: (f32::from(r.bounds.right()) * r.scale).ceil() as i32,
                    bottom: (f32::from(r.bounds.bottom()) * r.scale).ceil() as i32,
                };
                self.tool.lpszText = self.text.as_mut_ptr();
                // A width enables long/multiline text. Font, colors and all delay
                // values remain under the OS's control.
                SendMessageW(
                    self.tooltip,
                    TTM_SETMAXTIPWIDTH,
                    0,
                    (640. * r.scale) as isize,
                );
                self.registered = SendMessageW(
                    self.tooltip,
                    TTM_ADDTOOLW,
                    0,
                    &self.tool as *const _ as isize,
                ) != 0;
            }
        }
    }
    pub fn relay_move(&self, position: Point<Pixels>, scale: f32) {
        if !self.registered {
            return;
        }
        let x = (f32::from(position.x) * scale) as i32;
        let y = (f32::from(position.y) * scale) as i32;
        let mut event = MSG {
            hwnd: self.parent,
            message: WM_MOUSEMOVE,
            lParam: ((y as u32 & 0xffff) << 16 | (x as u32 & 0xffff)) as isize,
            ..unsafe { std::mem::zeroed() }
        };
        unsafe {
            event.time = GetMessageTime() as u32;
            GetCursorPos(&mut event.pt);
            SendMessageW(self.tooltip, TTM_RELAYEVENT, 0, &event as *const _ as isize);
        }
    }
}
impl Drop for Tooltips {
    fn drop(&mut self) {
        self.set(None);
        unsafe {
            DestroyWindow(self.tooltip);
        }
    }
}
