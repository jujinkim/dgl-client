use gpui::{App, Bounds, FocusHandle, Pixels, ScrollHandle, Window, px};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(PartialEq)]
struct RevealedFocus {
    focus: FocusHandle,
    viewport: Bounds<Pixels>,
}

/// The direction a scroll container moves to reveal its focused control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RevealAxis {
    Vertical,
    Horizontal,
}

/// Which focused control a scroll container last revealed, so it is revealed once per
/// focus and viewport instead of fighting the user's own scrolling.
#[derive(Clone, Default)]
pub struct RevealState(Rc<RefCell<Option<RevealedFocus>>>);
impl RevealState {
    pub fn reset(&self) {
        *self.0.borrow_mut() = None;
    }
}

/// Room kept around a revealed control. Child bounds leave out the control's own
/// padding and 1px focus border (settings rows and categories pad by up to 12px),
/// so they are added back.
const REVEAL_MARGIN: f32 = 14.;

/// Scrolls `scroll` along `axis` so the control whose children prepaint at `bounds`
/// is fully visible once it gains focus.
pub fn reveal_on_focus_in(
    scroll: ScrollHandle,
    state: RevealState,
    focus: FocusHandle,
    axis: RevealAxis,
) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static {
    move |bounds, window, _| {
        let viewport = scroll.bounds();
        let current = RevealedFocus {
            focus: focus.clone(),
            viewport,
        };
        if !focus.is_focused(window) || state.0.borrow().as_ref() == Some(&current) {
            return;
        }
        *state.0.borrow_mut() = Some(current);
        let Some((start, end)) = bounds
            .iter()
            .map(|b| match axis {
                RevealAxis::Vertical => (b.top(), b.bottom()),
                RevealAxis::Horizontal => (b.left(), b.right()),
            })
            .reduce(|(s, e), (bs, be)| (s.min(bs), e.max(be)))
        else {
            return;
        };
        let (view_start, view_end) = match axis {
            RevealAxis::Vertical => (viewport.top(), viewport.bottom()),
            RevealAxis::Horizontal => (viewport.left(), viewport.right()),
        };
        let margin = px(REVEAL_MARGIN);
        let (start, end) = (start - margin, end + margin);
        // A control longer than the viewport shows its start.
        let delta = if start < view_start || end - start > view_end - view_start {
            view_start - start
        } else if end > view_end {
            view_end - end
        } else {
            return;
        };
        if delta == px(0.) {
            return;
        }
        let mut offset = scroll.offset();
        match axis {
            RevealAxis::Vertical => offset.y += delta,
            RevealAxis::Horizontal => offset.x += delta,
        }
        scroll.set_offset(offset);
        window.refresh();
    }
}

/// An explicit focus ring keeps background controls out of modal Tab navigation.
#[derive(Default)]
pub struct Modal {
    pub building: bool,
    previous: Option<FocusHandle>,
    kind: Option<&'static str>,
    buttons: RefCell<HashMap<String, FocusHandle>>,
    ring: RefCell<Vec<FocusHandle>>,
    initial: bool,
    pub scroll: ScrollHandle,
    revealed: RevealState,
}
impl Modal {
    pub fn begin(
        &mut self,
        kind: &'static str,
        inputs: Vec<FocusHandle>,
        window: &Window,
        cx: &App,
    ) {
        if self.kind.is_none() {
            self.previous = window.focused(cx);
        }
        self.initial = self.kind != Some(kind);
        if self.initial {
            self.scroll.set_offset(gpui::point(px(0.), px(0.)));
            self.revealed.reset();
        }
        self.kind = Some(kind);
        *self.ring.borrow_mut() = inputs;
        self.building = true;
    }
    pub fn button(&self, id: &str, cx: &mut App) -> FocusHandle {
        let handle = self
            .buttons
            .borrow_mut()
            .entry(id.into())
            .or_insert_with(|| cx.focus_handle())
            .clone();
        self.ring.borrow_mut().push(handle.clone());
        handle
    }
    /// Whether the last built frame registered the button `id`. Test debug bounds
    /// outlive removed elements, so absence is checked here.
    #[cfg(test)]
    pub fn built(&self, id: &str) -> bool {
        self.buttons
            .borrow()
            .get(id)
            .is_some_and(|focus| self.ring.borrow().contains(focus))
    }
    pub fn input(&self, handle: FocusHandle) {
        self.ring.borrow_mut().push(handle);
    }
    /// Reveals a focused control in the tracked modal scroll.
    pub fn reveal_on_focus(
        &self,
        focus: FocusHandle,
    ) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static {
        reveal_on_focus_in(
            self.scroll.clone(),
            self.revealed.clone(),
            focus,
            RevealAxis::Vertical,
        )
    }
    pub fn restore_button(&mut self, id: &str, window: &mut Window) {
        if let Some(focus) = self.buttons.borrow().get(id) {
            window.focus(focus);
            self.initial = false;
        }
    }
    pub fn finish(&mut self, window: &mut Window) {
        self.building = false;
        let ring = self.ring.borrow();
        if (self.initial || !ring.iter().any(|f| f.is_focused(window)))
            && let Some(first) = ring.first()
        {
            window.focus(first);
        }
    }
    pub fn restore(&mut self, window: &mut Window) {
        if self.kind.take().is_some()
            && let Some(previous) = self.previous.take()
        {
            window.focus(&previous);
        }
        self.ring.borrow_mut().clear();
        self.buttons.borrow_mut().clear();
    }
    pub fn cycle(&self, backwards: bool, window: &mut Window) {
        let ring = self.ring.borrow();
        if ring.is_empty() {
            return;
        }
        let current = ring.iter().position(|f| f.is_focused(window)).unwrap_or(0);
        let next = if backwards {
            (current + ring.len() - 1) % ring.len()
        } else {
            (current + 1) % ring.len()
        };
        window.focus(&ring[next]);
    }
}
