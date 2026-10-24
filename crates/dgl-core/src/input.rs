use crate::{Error, Result};
use serde::{Deserialize, Serialize};
pub const MAX_INPUT: usize = 65536;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalKey {
    Escape,
    Tab,
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
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Escape => b"\x1b",
            Self::Tab => b"\t",
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
