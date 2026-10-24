use alacritty_terminal::term::TermMode;
use gpui::{Keystroke, Modifiers, MouseButton, ScrollDelta, TouchPhase};

// xterm's legacy keyboard protocol. Printable text and IME commits stay with GPUI.
pub(super) fn key_sequence(key: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let m = key.modifiers;
    if m.platform {
        return None;
    }
    let modifier = 1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.control);
    let final_char = match key.key.as_str() {
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
            } else if key.key.starts_with('f') || mode.contains(TermMode::APP_CURSOR) {
                format!("\x1bO{c}")
            } else {
                format!("\x1b[{c}")
            }
            .into_bytes(),
        );
    }
    let number = match key.key.as_str() {
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
    let mut bytes = match key.key.as_str() {
        "enter" => vec![b'\r'],
        "backspace" => vec![if m.control { 0x08 } else { 0x7f }],
        "tab" if m.shift => b"\x1b[Z".to_vec(),
        "tab" => vec![b'\t'],
        "escape" => vec![0x1b],
        key if m.control => vec![control_byte(key)?],
        "space" if m.alt => vec![b' '],
        key if m.alt && key.chars().count() == 1 => {
            if m.shift {
                key.to_uppercase().into_bytes()
            } else {
                key.as_bytes().to_vec()
            }
        }
        _ => return None,
    };
    if m.alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn control_byte(key: &str) -> Option<u8> {
    match key {
        "space" | " " | "2" | "@" => Some(0),
        "3" => Some(0x1b),
        "4" => Some(0x1c),
        "5" => Some(0x1d),
        "6" => Some(0x1e),
        "7" | "/" => Some(0x1f),
        "8" | "?" => Some(0x7f),
        key if key.len() == 1 => {
            let byte = key.as_bytes()[0].to_ascii_uppercase();
            (b'@'..=b'_').contains(&byte).then_some(byte & 31)
        }
        _ => None,
    }
}

pub(super) fn button_code(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

pub(super) fn mouse_modifiers(m: Modifiers) -> u8 {
    4 * u8::from(m.shift) + 8 * u8::from(m.alt) + 16 * u8::from(m.control)
}

pub(super) fn mouse_sequence(
    mode: TermMode,
    column: usize,
    row: usize,
    code: u8,
    release: bool,
    modifiers: Modifiers,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    let mods = mouse_modifiers(modifiers);
    if mode.contains(TermMode::SGR_MOUSE) {
        return Some(
            format!(
                "\x1b[<{};{};{}{}",
                code + mods,
                column + 1,
                row + 1,
                if release { 'm' } else { 'M' }
            )
            .into_bytes(),
        );
    }
    let utf8 = mode.contains(TermMode::UTF8_MOUSE);
    let limit = if utf8 { 2015 } else { 223 };
    if column >= limit || row >= limit {
        return None;
    }
    let mut bytes = b"\x1b[M".to_vec();
    let values = [
        usize::from(if release { 3 } else { code } + mods) + 32,
        column + 33,
        row + 33,
    ];
    for value in values {
        if utf8 {
            bytes.extend_from_slice(
                char::from_u32(value as u32)?
                    .encode_utf8(&mut [0; 4])
                    .as_bytes(),
            );
        } else {
            bytes.push(value as u8);
        }
    }
    Some(bytes)
}

#[derive(Default)]
pub(super) struct ScrollAccumulator {
    remainder: [f32; 2],
    context: Option<(TermMode, Modifiers)>,
}

impl ScrollAccumulator {
    pub fn steps(
        &mut self,
        delta: ScrollDelta,
        phase: TouchPhase,
        cell: (f32, f32),
        mode: TermMode,
        modifiers: Modifiers,
    ) -> [i32; 2] {
        let context = Some((mode, modifiers));
        if matches!(phase, TouchPhase::Started) || self.context != context {
            self.remainder = [0.; 2];
        }
        self.context = context;
        let amounts = match delta {
            ScrollDelta::Lines(p) => [p.x, p.y],
            ScrollDelta::Pixels(p) => [f32::from(p.x) / cell.0, f32::from(p.y) / cell.1],
        };
        let mut steps = [0; 2];
        for i in 0..2 {
            if !amounts[i].is_finite() {
                continue;
            }
            // Reversing direction must not first repay an old partial scroll.
            if amounts[i] * self.remainder[i] < 0. {
                self.remainder[i] = 0.;
            }
            let total = self.remainder[i] + amounts[i];
            steps[i] = total.trunc() as i32;
            self.remainder[i] = total.fract();
        }
        steps
    }
}
