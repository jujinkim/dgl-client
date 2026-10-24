use crate::{Error, Result};
use serde::{Deserialize, Serialize};
pub const MAX_INPUT: usize = 65536;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalKey {
    Escape,
    Tab,
    ShiftTab,
    ShiftLeft,
    CtrlA,
    CtrlB,
    CtrlC,
    CtrlD,
    CtrlE,
    CtrlF,
    CtrlG,
    CtrlH,
    CtrlI,
    CtrlJ,
    CtrlK,
    CtrlL,
    CtrlM,
    CtrlN,
    CtrlO,
    CtrlP,
    CtrlQ,
    CtrlR,
    CtrlS,
    CtrlT,
    CtrlU,
    CtrlV,
    CtrlW,
    CtrlX,
    CtrlY,
    CtrlZ,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Enter,
    Backspace,
    Delete,
    PageUp,
    PageDown,
}
impl TerminalKey {
    pub const ALL: &[Self] = &[
        Self::Escape,
        Self::Tab,
        Self::ShiftTab,
        Self::ShiftLeft,
        Self::CtrlA,
        Self::CtrlB,
        Self::CtrlC,
        Self::CtrlD,
        Self::CtrlE,
        Self::CtrlF,
        Self::CtrlG,
        Self::CtrlH,
        Self::CtrlI,
        Self::CtrlJ,
        Self::CtrlK,
        Self::CtrlL,
        Self::CtrlM,
        Self::CtrlN,
        Self::CtrlO,
        Self::CtrlP,
        Self::CtrlQ,
        Self::CtrlR,
        Self::CtrlS,
        Self::CtrlT,
        Self::CtrlU,
        Self::CtrlV,
        Self::CtrlW,
        Self::CtrlX,
        Self::CtrlY,
        Self::CtrlZ,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Home,
        Self::End,
        Self::Enter,
        Self::Backspace,
        Self::Delete,
        Self::PageUp,
        Self::PageDown,
    ];

    pub const DEFAULT_SHORTCUTS: &[Self] = &[
        Self::Escape,
        Self::Tab,
        Self::Enter,
        Self::ShiftTab,
        Self::ShiftLeft,
        Self::CtrlC,
        Self::CtrlD,
        Self::CtrlL,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Home,
        Self::End,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Escape => "Esc",
            Self::Tab => "Tab",
            Self::ShiftTab => "Shift+Tab",
            Self::ShiftLeft => "Shift+←",
            Self::CtrlA => "Ctrl+A",
            Self::CtrlB => "Ctrl+B",
            Self::CtrlC => "Ctrl+C",
            Self::CtrlD => "Ctrl+D",
            Self::CtrlE => "Ctrl+E",
            Self::CtrlF => "Ctrl+F",
            Self::CtrlG => "Ctrl+G",
            Self::CtrlH => "Ctrl+H",
            Self::CtrlI => "Ctrl+I",
            Self::CtrlJ => "Ctrl+J",
            Self::CtrlK => "Ctrl+K",
            Self::CtrlL => "Ctrl+L",
            Self::CtrlM => "Ctrl+M",
            Self::CtrlN => "Ctrl+N",
            Self::CtrlO => "Ctrl+O",
            Self::CtrlP => "Ctrl+P",
            Self::CtrlQ => "Ctrl+Q",
            Self::CtrlR => "Ctrl+R",
            Self::CtrlS => "Ctrl+S",
            Self::CtrlT => "Ctrl+T",
            Self::CtrlU => "Ctrl+U",
            Self::CtrlV => "Ctrl+V",
            Self::CtrlW => "Ctrl+W",
            Self::CtrlX => "Ctrl+X",
            Self::CtrlY => "Ctrl+Y",
            Self::CtrlZ => "Ctrl+Z",
            Self::Up => "↑",
            Self::Down => "↓",
            Self::Left => "←",
            Self::Right => "→",
            Self::Home => "Home",
            Self::End => "End",
            Self::Enter => "Enter",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Escape => b"\x1b",
            Self::Tab => b"\t",
            Self::ShiftTab => b"\x1b[Z",
            Self::ShiftLeft => b"\x1b[1;2D",
            Self::CtrlA => b"\x01",
            Self::CtrlB => b"\x02",
            Self::CtrlC => b"\x03",
            Self::CtrlD => b"\x04",
            Self::CtrlE => b"\x05",
            Self::CtrlF => b"\x06",
            Self::CtrlG => b"\x07",
            Self::CtrlH => b"\x08",
            Self::CtrlI => b"\x09",
            Self::CtrlJ => b"\x0a",
            Self::CtrlK => b"\x0b",
            Self::CtrlL => b"\x0c",
            Self::CtrlM => b"\x0d",
            Self::CtrlN => b"\x0e",
            Self::CtrlO => b"\x0f",
            Self::CtrlP => b"\x10",
            Self::CtrlQ => b"\x11",
            Self::CtrlR => b"\x12",
            Self::CtrlS => b"\x13",
            Self::CtrlT => b"\x14",
            Self::CtrlU => b"\x15",
            Self::CtrlV => b"\x16",
            Self::CtrlW => b"\x17",
            Self::CtrlX => b"\x18",
            Self::CtrlY => b"\x19",
            Self::CtrlZ => b"\x1a",
            Self::Up => b"\x1b[A",
            Self::Down => b"\x1b[B",
            Self::Left => b"\x1b[D",
            Self::Right => b"\x1b[C",
            Self::Home => b"\x1b[H",
            Self::End => b"\x1b[F",
            Self::Enter => b"\r",
            Self::Backspace => b"\x7f",
            Self::Delete => b"\x1b[3~",
            Self::PageUp => b"\x1b[5~",
            Self::PageDown => b"\x1b[6~",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MacroStep {
    Text { text: String },
    Key { key: TerminalKey },
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeAction {
    #[default]
    Default,
    Shortcut,
    Macro,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VolumeBinding {
    pub action: VolumeAction,
    pub steps: Vec<MacroStep>,
}
pub fn encode(steps: &[MacroStep]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for step in steps {
        let bytes = match step {
            MacroStep::Key { key } => key.bytes().to_vec(),
            MacroStep::Text { text } => text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', "\r")
                .into_bytes(),
        };
        if bytes.len() > MAX_INPUT - out.len() {
            return Err(Error::InvalidArgument);
        }
        out.extend(bytes);
    }
    if out.is_empty() {
        Err(Error::InvalidArgument)
    } else {
        Ok(out)
    }
}
impl VolumeBinding {
    pub fn bytes(&self) -> Result<Option<Vec<u8>>> {
        match self.action {
            VolumeAction::Default => Ok(None),
            VolumeAction::Shortcut => {
                if self.steps.len() != 1 || !matches!(self.steps[0], MacroStep::Key { .. }) {
                    return Err(Error::InvalidArgument);
                }
                Ok(Some(encode(&self.steps)?))
            }
            VolumeAction::Macro => Ok(Some(encode(&self.steps)?)),
        }
    }
}
