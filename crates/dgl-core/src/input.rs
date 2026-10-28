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
        Self::CtrlJ,
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
/// Stable string serialization keeps existing shortcut lists readable, including empty lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TerminalShortcut {
    Modifier(Modifier),
    Key(TerminalKey),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}
impl Modifiers {
    pub fn union(self, other: Self) -> Self {
        Self {
            control: self.control || other.control,
            shift: self.shift || other.shift,
            alt: self.alt || other.alt,
        }
    }
    pub fn toggle(&mut self, modifier: Modifier) {
        let value = match modifier {
            Modifier::Ctrl => &mut self.control,
            Modifier::Shift => &mut self.shift,
            Modifier::Alt => &mut self.alt,
        };
        *value = !*value;
    }
    pub fn contains(self, modifier: Modifier) -> bool {
        match modifier {
            Modifier::Ctrl => self.control,
            Modifier::Shift => self.shift,
            Modifier::Alt => self.alt,
        }
    }
}
impl TerminalShortcut {
    pub fn defaults() -> Vec<Self> {
        [Modifier::Ctrl, Modifier::Shift, Modifier::Alt]
            .into_iter()
            .map(Self::Modifier)
            .chain(
                TerminalKey::DEFAULT_SHORTCUTS
                    .iter()
                    .copied()
                    .map(Self::Key),
            )
            .collect()
    }
    pub fn all() -> Vec<Self> {
        [Modifier::Ctrl, Modifier::Shift, Modifier::Alt]
            .into_iter()
            .map(Self::Modifier)
            .chain(TerminalKey::ALL.iter().copied().map(Self::Key))
            .collect()
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Modifier(Modifier::Ctrl) => "Ctrl",
            Self::Modifier(Modifier::Shift) => "Shift",
            Self::Modifier(Modifier::Alt) => "Alt",
            Self::Key(k) => k.label(),
        }
    }
}
impl TerminalKey {
    /// Decode the semantic key, never modify a byte sequence that may be mouse/paste data.
    pub fn encode(self, modifiers: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
        let name = format!("{self:?}");
        let (key, own) = if let Some(letter) = name.strip_prefix("Ctrl") {
            (
                letter.to_lowercase(),
                Modifiers {
                    control: true,
                    ..Default::default()
                },
            )
        } else if let Some(key) = name.strip_prefix("Shift") {
            (
                key.to_lowercase(),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
            )
        } else {
            (name.to_lowercase(), Modifiers::default())
        };
        encode_key(&key, None, modifiers.union(own), app_cursor)
    }
}
/// xterm legacy key encoding shared by GPUI and Android. Text is an unencoded
/// keyboard/IME commit; paste, macros, protocol responses and mouse bypass this API.
pub fn encode_key(
    key: &str,
    text: Option<&str>,
    m: Modifiers,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.control);
    let final_char = match key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        "f1" => Some('P'),
        "f2" => Some('Q'),
        "f3" => Some('R'),
        "f4" => Some('S'),
        _ => None,
    };
    if let Some(c) = final_char {
        return Some(
            if modifier > 1 {
                format!("\x1b[1;{modifier}{c}")
            } else if key.starts_with('f') || app_cursor {
                format!("\x1bO{c}")
            } else {
                format!("\x1b[{c}")
            }
            .into_bytes(),
        );
    }
    let number = match key {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(n) = number {
        return Some(
            if modifier > 1 {
                format!("\x1b[{n};{modifier}~")
            } else {
                format!("\x1b[{n}~")
            }
            .into_bytes(),
        );
    }
    let mut bytes = match key {
        "enter" => vec![b'\r'],
        "backspace" => vec![if m.control { 8 } else { 127 }],
        "tab" if m.shift => b"\x1b[Z".to_vec(),
        "tab" => vec![b'\t'],
        "escape" => vec![27],
        _ => {
            if text.is_none() && key != "space" && key.chars().count() != 1 {
                return None;
            }
            let value = text.unwrap_or(if key == "space" { " " } else { key });
            if value.is_empty() || value.len() > MAX_INPUT || value.chars().any(char::is_control) {
                return None;
            }
            if m.control {
                vec![control_byte(value)?]
            } else if m.shift {
                if key == "text" && !value.is_ascii() {
                    return None;
                }
                if value.is_ascii() {
                    shift_text(value).into_bytes()
                } else {
                    value.to_uppercase().into_bytes()
                }
            } else {
                value.as_bytes().to_vec()
            }
        }
    };
    if m.alt {
        bytes.insert(0, 27);
    }
    (!bytes.is_empty() && bytes.len() <= MAX_INPUT).then_some(bytes)
}
fn shift_text(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '1' => '!',
            '2' => '@',
            '3' => '#',
            '4' => '$',
            '5' => '%',
            '6' => '^',
            '7' => '&',
            '8' => '*',
            '9' => '(',
            '0' => ')',
            '-' => '_',
            '=' => '+',
            '[' => '{',
            ']' => '}',
            ';' => ':',
            '\'' => '"',
            ',' => '<',
            '.' => '>',
            '/' => '?',
            '\\' => '|',
            '`' => '~',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}
fn control_byte(key: &str) -> Option<u8> {
    match key {
        " " | "2" | "@" => Some(0),
        "3" => Some(27),
        "4" => Some(28),
        "5" => Some(29),
        "6" => Some(30),
        "7" | "/" => Some(31),
        "8" | "?" => Some(127),
        key if key.len() == 1 => {
            let b = key.as_bytes()[0].to_ascii_uppercase();
            (b'@'..=b'_').contains(&b).then_some(b & 31)
        }
        _ => None,
    }
}
/// A drag preview is transient. Only the caller's successful drop persists its result.
pub fn move_shortcut(keys: &[TerminalShortcut], from: usize, to: usize) -> Vec<TerminalShortcut> {
    let mut next = keys.to_vec();
    if from < next.len() && to < next.len() {
        let key = next.remove(from);
        next.insert(to, key);
    }
    next
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

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn shortcuts_keep_legacy_names_order_empty_and_new_defaults() {
        for value in [
            r#"["Right","CtrlC","Escape"]"#,
            "[]",
            r#"["Alt","Ctrl","Shift","ShiftLeft"]"#,
        ] {
            let keys: Vec<TerminalShortcut> = serde_json::from_str(value).unwrap();
            assert_eq!(serde_json::to_string(&keys).unwrap(), value);
        }
        for key in TerminalKey::ALL {
            assert_eq!(
                key.encode(Modifiers::default(), false).unwrap(),
                key.bytes()
            );
        }
        let defaults = TerminalShortcut::defaults();
        assert_eq!(
            &defaults[..3],
            &[
                TerminalShortcut::Modifier(Modifier::Ctrl),
                TerminalShortcut::Modifier(Modifier::Shift),
                TerminalShortcut::Modifier(Modifier::Alt)
            ]
        );
        let preview = move_shortcut(&defaults, 2, 0);
        assert_eq!(defaults[0], TerminalShortcut::Modifier(Modifier::Ctrl));
        assert_eq!(preview[0], TerminalShortcut::Modifier(Modifier::Alt));
        assert_eq!(move_shortcut(&defaults, usize::MAX, 0), defaults);
    }
    #[test]
    fn semantic_encoder_combines_all_modifiers_and_keeps_modes() {
        for control in [false, true] {
            for alt in [false, true] {
                for shift in [false, true] {
                    let m = Modifiers {
                        control,
                        shift,
                        alt,
                    };
                    let n = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(control);
                    for app in [false, true] {
                        let expected = if n > 1 {
                            format!("\x1b[1;{n}D")
                        } else if app {
                            "\x1bOD".into()
                        } else {
                            "\x1b[D".into()
                        };
                        assert_eq!(
                            TerminalKey::Left.encode(m, app).unwrap(),
                            expected.as_bytes()
                        );
                    }
                }
            }
        }
        assert_eq!(
            encode_key(
                "é",
                Some("É"),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
                false
            )
            .unwrap(),
            "É".as_bytes()
        );
        assert!(
            encode_key(
                "text",
                Some("한"),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
                false
            )
            .is_none()
        );
        let ctrl_alt = Modifiers {
            control: true,
            alt: true,
            shift: false,
        };
        assert_eq!(
            encode_key("c", Some("c"), ctrl_alt, false).unwrap(),
            b"\x1b\x03"
        );
        assert_eq!(
            TerminalKey::CtrlC.encode(ctrl_alt, false).unwrap(),
            b"\x1b\x03"
        );
        assert_eq!(
            TerminalKey::ShiftLeft.encode(ctrl_alt, true).unwrap(),
            b"\x1b[1;8D"
        );
        assert_eq!(
            encode_key(
                "text",
                Some("a1"),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
                false
            )
            .unwrap(),
            b"A!"
        );
        assert_eq!(encode_key("text", Some("한"), ctrl_alt, false), None);
        assert_eq!(encode_key("text", Some("ab"), ctrl_alt, false), None);
        assert_eq!(encode_key("Dead", None, Modifiers::default(), false), None);
        assert_eq!(
            encode_key("text", Some("\x1b[A"), Modifiers::default(), false),
            None
        );
        assert_eq!(
            encode_key(
                "text",
                Some(&"a".repeat(MAX_INPUT)),
                Modifiers::default(),
                false
            )
            .unwrap()
            .len(),
            MAX_INPUT
        );
        assert!(encode_key("text", Some(&"a".repeat(MAX_INPUT)), ctrl_alt, false).is_none());
    }
}
