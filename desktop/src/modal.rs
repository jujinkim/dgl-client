use gpui::{App, FocusHandle, Window};
use std::{cell::RefCell, collections::HashMap};

/// An explicit focus ring keeps background controls out of modal Tab navigation.
#[derive(Default)]
pub struct Modal {
    pub building: bool,
    previous: Option<FocusHandle>,
    kind: Option<&'static str>,
    buttons: RefCell<HashMap<String, FocusHandle>>,
    ring: RefCell<Vec<FocusHandle>>,
    initial: bool,
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
