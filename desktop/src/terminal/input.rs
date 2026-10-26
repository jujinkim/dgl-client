use alacritty_terminal::term::TermMode;
use gpui::{Keystroke, Modifiers, MouseButton, ScrollDelta, TouchPhase};

pub(super) fn paste_bytes(text: &str, bracketed: bool, enter: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    let mut bytes = if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    };
    if enter {
        bytes.push(b'\r');
    }
    bytes
}

// xterm's legacy keyboard protocol. Printable text and IME commits stay with GPUI.
pub(super) fn key_sequence(key: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let m = key.modifiers;
    if m.platform {
        return None;
    }
    // Ordinary printable keys belong to GPUI's text/IME commit path.
    if !m.control && !m.alt && (key.key.chars().count() == 1 || key.key == "space") {
        return None;
    }
    dgl_core::input::encode_key(
        &key.key,
        None,
        dgl_core::input::Modifiers {
            control: m.control,
            shift: m.shift,
            alt: m.alt,
        },
        mode.contains(TermMode::APP_CURSOR),
    )
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
