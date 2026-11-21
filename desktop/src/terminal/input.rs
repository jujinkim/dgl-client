use alacritty_terminal::term::TermMode;
use gpui::{Keystroke, Modifiers, MouseButton, ScrollDelta, TouchPhase};

pub(super) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
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

/// The core's test for input that carries grid coordinates (`Connection::input`):
/// such input is admitted only for the exact size generation it was encoded for.
pub(super) fn pointer_report(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x1b[M") || bytes.starts_with(b"\x1b[<")
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

/// Input requests one pane keeps in flight before holding keys back. The core admits
/// at most 33 requests per connection and refuses the rest (`QueueFull`), so a burst
/// faster than the connection answers (key repeat, a text expander) used to lose keys.
/// Below this limit keys are pipelined as before, so slow links add no typing latency.
pub(super) const INPUT_IN_FLIGHT: usize = 8;
/// `Connection::input` refuses anything larger.
const INPUT_MAX: usize = 65536;

pub(super) enum Admission {
    /// Nothing is held: send these bytes now.
    Send(Vec<u8>),
    /// Joined the held keys; they go out together when a request completes.
    Held,
    /// A pointer report during a burst. Its grid coordinates must be checked alone,
    /// and sending it now would pass the keys held before it.
    Skipped,
    /// The held keys already fill one request.
    Overflow,
}

/// Keys held back while `INPUT_IN_FLIGHT` requests are unanswered. They are not
/// failed input: they were never sent. A failure or a new destination discards them,
/// so nothing typed after a failed key reaches the pane and nothing is resent.
#[derive(Default)]
pub(super) struct InputBacklog(Option<(dgl_core::flow::PaneTarget, Vec<u8>, bool)>);

impl InputBacklog {
    pub(super) fn admit(
        &mut self,
        target: &dgl_core::flow::PaneTarget,
        bytes: Vec<u8>,
        claim: bool,
        in_flight: usize,
    ) -> Admission {
        if self
            .0
            .as_ref()
            .is_some_and(|(t, ..)| !t.same_input_destination(target))
        {
            self.0 = None;
        }
        if self.0.is_none() && in_flight < INPUT_IN_FLIGHT {
            return Admission::Send(bytes);
        }
        if pointer_report(&bytes) {
            return Admission::Skipped;
        }
        match &mut self.0 {
            Some((_, held, _)) if held.len() + bytes.len() > INPUT_MAX => Admission::Overflow,
            Some((_, held, held_claim)) => {
                held.extend(bytes);
                *held_claim |= claim;
                Admission::Held
            }
            None => {
                self.0 = Some((target.clone(), bytes, claim));
                Admission::Held
            }
        }
    }

    /// After a request succeeded: the held keys and whether they operate the screen,
    /// when they still belong to `target`. Otherwise they are dropped and `Err` says
    /// that keys were lost.
    pub(super) fn release(
        &mut self,
        target: Option<&dgl_core::flow::PaneTarget>,
        in_flight: usize,
    ) -> Result<Option<(Vec<u8>, bool)>, ()> {
        if in_flight >= INPUT_IN_FLIGHT {
            return Ok(None);
        }
        match self.0.take() {
            None => Ok(None),
            Some((held_target, bytes, claim))
                if target.is_some_and(|t| held_target.same_input_destination(t)) =>
            {
                Ok(Some((bytes, claim)))
            }
            Some(_) => Err(()),
        }
    }

    /// An earlier request failed: keys typed after it must not reach the pane.
    pub(super) fn discard(&mut self) -> bool {
        self.0.take().is_some()
    }
}
