use crate::snackbar::{SnackbarDuration, SnackbarRequest};
use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point as CellPoint, Side},
    selection::{Selection, SelectionType},
    term::{Config, TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape, Processor},
};
use dgl_core::connection::{Action, Connection};
use dgl_core::input::{Modifiers as KeyModifiers, TerminalShortcut};
use dgl_core::tr;
use gpui::{prelude::*, *};
use std::{collections::HashSet, ops::Range, sync::Arc};
mod input;
#[cfg(test)]
mod input_tests;
mod paint;
mod window;
use input::{ScrollAccumulator, button_code, key_sequence, mouse_modifiers, mouse_sequence};
use paint::{CellLayer, paint_grid};
pub use window::WindowTerminal;
#[derive(Clone)]
struct Listener(Arc<Connection>, Option<String>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event
            && let Some(t) = self
                .0
                .target()
                .filter(|t| self.1.as_ref().is_none_or(|id| id == &t.pane_id))
        {
            let _ = self.0.input(t, text.into_bytes());
        }
    }
}
struct GridSize(usize, usize);
impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.1
    }
    fn screen_lines(&self) -> usize {
        self.1
    }
    fn columns(&self) -> usize {
        self.0
    }
}
/// Fixed monospace cell ratios, shared by pane painting and the size requested from tmux.
fn cell_size_for(font_size: f32) -> (f32, f32) {
    (font_size * 0.61, font_size * 1.4)
}
pub struct Terminal {
    pub connection: Arc<Connection>,
    pane_id: Option<String>,
    term: Term<Listener>,
    parser: Processor,
    focus: FocusHandle,
    font_size: f32,
    font_owner: Option<WeakEntity<WindowTerminal>>,
    bounds: Bounds<Pixels>,
    marked: String,
    marked_target: Option<Option<(u64, u64, String, u64)>>,
    selection: Range<usize>,
    dragging: bool,
    mouse_button: Option<MouseButton>,
    last_mouse: Option<(CellPoint, u8)>,
    scroll: ScrollAccumulator,
    displayed_pane: Option<String>,
    snapshot_modes: Option<TermMode>,
    pub enabled: bool,
    pointer_enabled: bool,
    #[cfg(test)]
    pub sent: Vec<Vec<u8>>,
    pub notice: Option<SnackbarRequest>,
    input_requests: HashSet<u64>,
    pub modifiers: KeyModifiers,
    modifier_target: Option<(u64, u64, String, u64)>,
    modifier_phase: Option<dgl_core::connection::Phase>,
}
impl Terminal {
    #[cfg(test)]
    pub fn new(
        connection: Arc<Connection>,
        font_size: i32,
        scrollback: i32,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(connection, None, font_size, scrollback, cx)
    }
    fn build(
        connection: Arc<Connection>,
        pane_id: Option<String>,
        font_size: i32,
        scrollback: i32,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = Config {
            scrolling_history: scrollback.clamp(100, 10000) as usize,
            ..Default::default()
        };
        let term = Term::new(
            config,
            &GridSize(80, 24),
            Listener(connection.clone(), pane_id.clone()),
        );
        Self {
            connection,
            pane_id,
            term,
            parser: Processor::new(),
            focus: cx.focus_handle(),
            font_size: font_size.clamp(10, 28) as f32,
            font_owner: None,
            bounds: Bounds::default(),
            marked: String::new(),
            marked_target: None,
            selection: 0..0,
            dragging: false,
            mouse_button: None,
            last_mouse: None,
            scroll: ScrollAccumulator::default(),
            displayed_pane: None,
            snapshot_modes: None,
            enabled: true,
            pointer_enabled: true,
            #[cfg(test)]
            sent: Vec::new(),
            notice: None,
            input_requests: HashSet::new(),
            modifiers: KeyModifiers::default(),
            modifier_target: None,
            modifier_phase: None,
        }
    }
    fn input_modes(&self) -> TermMode {
        self.snapshot_modes.unwrap_or(*self.term.mode())
    }
    fn begin_snapshot(&mut self, pane: &str, reset: &[u8]) {
        if self.displayed_pane.as_deref() != Some(pane) {
            self.snapshot_modes = None;
        } else if self.snapshot_modes.is_none() {
            self.snapshot_modes = Some(*self.term.mode());
        }
        self.dragging = false;
        self.mouse_button = None;
        self.last_mouse = None;
        self.scroll = ScrollAccumulator::default();
        // A resize or moving a tab must not discard local history/scroll position.
        // Reset terminal modes, then restore the primary grid's history and clear only its viewport.
        let same_pane = self.displayed_pane.as_deref() == Some(pane);
        let saved = if same_pane {
            if self.term.mode().contains(TermMode::ALT_SCREEN) {
                self.term.swap_alt();
            }
            let empty = alacritty_terminal::grid::Grid::new(
                self.term.screen_lines(),
                self.term.columns(),
                0,
            );
            Some(std::mem::replace(self.term.grid_mut(), empty))
        } else {
            None
        };
        self.parser = Processor::new();
        self.parser.advance(&mut self.term, reset);
        if let Some(mut grid) = saved {
            grid.cursor = Default::default();
            grid.reset_region(Line(0)..);
            grid.saved_cursor = Default::default();
            *self.term.grid_mut() = grid;
        }
        self.displayed_pane = Some(pane.into());
    }
    pub fn send(&mut self, bytes: Vec<u8>) {
        if !self.enabled {
            return;
        }
        #[cfg(test)]
        self.sent.push(bytes.clone());
        if self.connection.target().is_none() {
            self.notice = Some(SnackbarRequest::new(
                dgl_core::i18n::t(
                    "연결 확인·복구 중에는 입력할 수 없습니다. 입력을 다시 전송하지 않습니다.",
                ),
                SnackbarDuration::Short,
            ));
        }
        if let Some(t) = self
            .connection
            .target()
            .filter(|t| self.pane_id.as_ref().is_none_or(|id| id == &t.pane_id))
        {
            match self.connection.input(t, bytes) {
                Ok(id) => {
                    self.input_requests.insert(id);
                }
                Err(e) => {
                    self.notice = Some(SnackbarRequest::new(
                        tr!("입력을 보내지 못했습니다: {0}", e),
                        SnackbarDuration::Short,
                    ))
                }
            }
        }
    }
    fn input_identity(&self) -> Option<(u64, u64, String, u64)> {
        self.connection.target().map(|t| {
            (
                t.connection,
                t.generation.connection,
                t.pane_id,
                t.generation.pane,
            )
        })
    }
    pub fn sync_modifiers(&mut self) {
        let next = self.input_identity();
        let state = self.connection.state();
        let size_change = state.phase == dgl_core::connection::Phase::Restoring && state.resizing;
        let phase = if size_change {
            self.modifier_phase.clone()
        } else {
            Some(state.phase.clone())
        };
        if self.modifier_target != next || self.modifier_phase != phase || !self.enabled {
            self.modifiers = KeyModifiers::default();
        }
        self.modifier_target = next;
        self.modifier_phase = phase;
    }
    pub fn shortcut(&mut self, shortcut: TerminalShortcut) {
        self.sync_modifiers();
        if !self.enabled {
            return;
        }
        match shortcut {
            TerminalShortcut::Modifier(m) => self.modifiers.toggle(m),
            TerminalShortcut::Key(k) => {
                if let Some(bytes) = k.encode(
                    self.modifiers,
                    self.input_modes().contains(TermMode::APP_CURSOR),
                ) {
                    self.modifiers = KeyModifiers::default();
                    self.send(bytes);
                }
            }
        }
    }
    fn commit_keyboard(&mut self, key: &str, text: Option<&str>, physical: KeyModifiers) {
        self.sync_modifiers();
        if !self.enabled {
            return;
        }
        if let Some(bytes) = dgl_core::input::encode_key(
            key,
            text,
            self.modifiers.union(physical),
            self.input_modes().contains(TermMode::APP_CURSOR),
        ) {
            self.modifiers = KeyModifiers::default();
            self.send(bytes);
        } else {
            self.notice = Some(SnackbarRequest::new(
                dgl_core::i18n::t(
                    "이 조합은 터미널 입력으로 표현할 수 없습니다. 조합키를 해제하거나 다른 키를 사용하세요.",
                ),
                SnackbarDuration::Short,
            ));
        }
    }
    pub fn complete(&mut self, id: u64) -> bool {
        self.input_requests.remove(&id)
    }
    pub fn set_font(&mut self, n: i32) {
        self.font_size = n.clamp(10, 28) as f32;
    }
    fn cell_size(&self) -> (f32, f32) {
        cell_size_for(self.font_size)
    }
    fn point(&self, p: Point<Pixels>) -> CellPoint {
        let (w, h) = self.cell_size();
        let col = ((f32::from(p.x - self.bounds.left()) / w).floor() as i32)
            .clamp(0, self.term.columns() as i32 - 1);
        let row = ((f32::from(p.y - self.bounds.top()) / h).floor() as i32)
            .clamp(0, self.term.screen_lines() as i32 - 1);
        CellPoint::new(
            Line(row - self.term.grid().display_offset() as i32),
            Column(col as usize),
        )
    }
    fn key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            cx.stop_propagation();
            return;
        }
        let k = &e.keystroke;
        if !self.marked.is_empty() {
            return;
        }
        if crate::platform::app_shortcut(&k.modifiers) && k.key == "c" {
            if let Some(text) = self.term.selection_to_string() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            cx.stop_propagation();
            return;
        }
        if crate::platform::app_shortcut(&k.modifiers) && k.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                let bytes = input::paste_bytes(
                    &text,
                    self.input_modes().contains(TermMode::BRACKETED_PASTE),
                );
                self.send(bytes);
            }
            cx.stop_propagation();
            return;
        }
        if crate::platform::terminal_app_shortcut(k) {
            // Let the containing app handle its shortcuts without sending a
            // control byte to the remote shell first.
            return;
        }
        self.sync_modifiers();
        if let Some(bytes) = key_sequence(k, self.input_modes()) {
            if self.modifiers == KeyModifiers::default() {
                self.send(bytes);
            } else {
                self.commit_keyboard(
                    &k.key,
                    k.key_char.as_deref(),
                    KeyModifiers {
                        control: k.modifiers.control,
                        shift: k.modifiers.shift,
                        alt: k.modifiers.alt,
                    },
                );
            }
            cx.stop_propagation();
            cx.notify();
        }
    }
    fn mouse_active(&self, modifiers: Modifiers) -> bool {
        self.enabled && !modifiers.shift && self.term.mode().intersects(TermMode::MOUSE_MODE)
    }
    fn mouse_report(&mut self, p: Point<Pixels>, code: u8, release: bool, modifiers: Modifiers) {
        if !self.enabled
            || !self.pointer_enabled
            || self.connection.state().resizing
            || self.connection.state().checking
        {
            return;
        }
        let mut cell = self.point(p);
        cell.line.0 += self.term.grid().display_offset() as i32;
        let state = (cell, (code & !32) + mouse_modifiers(modifiers));
        if code & 32 != 0 && self.last_mouse == Some(state) {
            return;
        }
        if let Some(bytes) = mouse_sequence(
            *self.term.mode(),
            cell.column.0,
            cell.line.0 as usize,
            code,
            release,
            modifiers,
        ) {
            self.send(bytes);
            self.last_mouse = if release { None } else { Some(state) };
        }
    }
    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled
            || !self.pointer_enabled
            || self.connection.state().resizing
            || self.connection.state().checking
        {
            return;
        }
        let Some(code) = button_code(e.button) else {
            return;
        };
        window.focus(&self.focus);
        if let Some(id) = self.pane_id.as_ref()
            && self.connection.state().pane.as_ref() != Some(id)
        {
            let _ = self
                .connection
                .action(Action::SelectPane { id: id.clone() });
            cx.notify();
            return;
        }
        if self.mouse_active(e.modifiers) {
            self.dragging = false;
            self.term.selection = None;
            self.mouse_button = Some(e.button);
            self.mouse_report(e.position, code, false, e.modifiers);
        } else if e.button == MouseButton::Left {
            self.dragging = true;
            self.term.selection = Some(Selection::new(
                SelectionType::Simple,
                self.point(e.position),
                Side::Left,
            ));
        }
        cx.notify();
    }
    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled
            || !self.pointer_enabled
            || self.connection.state().resizing
            || self.connection.state().checking
        {
            return;
        }
        if self.dragging && e.pressed_button == Some(MouseButton::Left) {
            let p = self.point(e.position);
            if let Some(s) = self.term.selection.as_mut() {
                s.update(p, Side::Right);
            }
            cx.notify();
            return;
        }
        if !self.bounds.contains(&e.position) {
            self.last_mouse = None;
            return;
        }
        if !self.mouse_active(e.modifiers) {
            self.last_mouse = None;
            return;
        }
        let mode = self.term.mode();
        let button = e.pressed_button.filter(|b| self.mouse_button == Some(*b));
        if e.pressed_button.is_some() && button.is_none() {
            return;
        }
        if mode.contains(TermMode::MOUSE_MOTION)
            || (mode.contains(TermMode::MOUSE_DRAG) && button.is_some())
        {
            self.mouse_report(
                e.position,
                32 + button.and_then(button_code).unwrap_or(3),
                false,
                e.modifiers,
            );
        }
    }
    fn mouse_up(&mut self, e: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        if e.button == MouseButton::Left {
            self.dragging = false;
        }
        if self.mouse_button == Some(e.button) {
            self.mouse_button = None;
            // Finish a remote press even if Shift was pressed or the pointer left the grid.
            if let Some(code) = button_code(e.button) {
                self.mouse_report(e.position, code, true, e.modifiers);
            }
        }
    }
    fn scroll_wheel(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(delta) = font_wheel_delta(e) {
            self.scroll = ScrollAccumulator::default();
            if let Some(owner) = &self.font_owner {
                let _ = owner.update(cx, |t, cx| {
                    t.font.adjust(delta);
                    cx.notify();
                });
            }
            cx.stop_propagation();
            return;
        }
        if !self.enabled
            || !self.pointer_enabled
            || self.connection.state().resizing
            || self.connection.state().checking
        {
            return;
        }
        let mode = *self.term.mode();
        let [horizontal, vertical] =
            self.scroll
                .steps(e.delta, e.touch_phase, self.cell_size(), mode, e.modifiers);
        if self.mouse_active(e.modifiers) {
            for (steps, positive, negative) in [(vertical, 64, 65), (horizontal, 66, 67)] {
                // One write per wheel event avoids flooding the connection request queue.
                let mut cell = self.point(e.position);
                cell.line.0 += self.term.grid().display_offset() as i32;
                if steps != 0
                    && let Some(bytes) = mouse_sequence(
                        mode,
                        cell.column.0,
                        cell.line.0 as usize,
                        if steps > 0 { positive } else { negative },
                        false,
                        e.modifiers,
                    )
                {
                    self.send(
                        bytes.repeat(
                            (steps.unsigned_abs() as usize)
                                .min(dgl_core::input::MAX_INPUT / bytes.len()),
                        ),
                    );
                }
            }
        } else if !e.modifiers.shift
            && mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
        {
            if vertical != 0 {
                let key = Keystroke {
                    key: if vertical > 0 { "up" } else { "down" }.into(),
                    modifiers: Modifiers::default(),
                    key_char: None,
                };
                let bytes = key_sequence(&key, mode).unwrap();
                self.send(
                    bytes.repeat(
                        (vertical.unsigned_abs() as usize)
                            .min(dgl_core::input::MAX_INPUT / bytes.len()),
                    ),
                );
            }
        } else {
            self.term.scroll_display(Scroll::Delta(vertical));
        }
        cx.stop_propagation();
        cx.notify();
    }
}
impl Focusable for Terminal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let chars: Vec<u16> = self.marked.encode_utf16().collect();
        let start = r.start.min(chars.len());
        let end = r.end.min(chars.len()).max(start);
        *actual = Some(start..end);
        Some(String::from_utf16_lossy(&chars[start..end]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if self.marked.is_empty() {
            None
        } else {
            Some(0..self.marked.encode_utf16().count())
        }
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked_target = None;
        self.marked.clear();
        self.selection = 0..0;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let origin = self.marked_target.take();
        self.marked.clear();
        self.selection = 0..0;
        if origin.is_some_and(|origin| origin != self.input_identity()) {
            self.sync_modifiers();
            self.notice = Some(SnackbarRequest::new(
                dgl_core::i18n::t("연결·세션·패널이 변경되어 입력을 취소했습니다."),
                SnackbarDuration::Short,
            ));
            cx.notify();
            return;
        }
        if !text.is_empty() {
            self.commit_keyboard("text", Some(text), KeyModifiers::default());
        }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marked_target.is_none() {
            self.marked_target = Some(self.input_identity());
        }
        self.marked = text.into();
        self.selection = selected.unwrap_or_else(|| {
            let n = text.encode_utf16().count();
            n..n
        });
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (w, h) = self.cell_size();
        let p = self.term.renderable_content().cursor.point;
        Some(Bounds::new(
            point(
                self.bounds.left() + px(p.column.0 as f32 * w),
                self.bounds.top() + px(p.line.0.max(0) as f32 * h),
            ),
            size(px(w.max(self.marked.chars().count() as f32 * w)), px(h)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}
fn font_wheel_delta(e: &ScrollWheelEvent) -> Option<i32> {
    let vertical = match e.delta {
        ScrollDelta::Pixels(p) => f32::from(p.y),
        ScrollDelta::Lines(p) => p.y,
    };
    (e.modifiers.control && vertical.is_finite() && vertical != 0.)
        .then(|| if vertical > 0. { 1 } else { -1 })
}
fn color(c: Color) -> Hsla {
    let palette = [
        0x11151b, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x8acdd0, 0xd8dee9, 0x596273,
        0xff7a85, 0xb2e28d, 0xffd68a, 0x82c7ff, 0xe29bff, 0xa8e5e7, 0xffffff,
    ];
    match c {
        Color::Spec(c) => rgb((c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32).into(),
        Color::Named(n) => {
            let i = n as usize;
            rgb(if i < 16 {
                palette[i]
            } else if i == 257 {
                0x11151b
            } else {
                0xd8dee9
            })
            .into()
        }
        Color::Indexed(i) => {
            let i = i as usize;
            if i < 16 {
                rgb(palette[i]).into()
            } else if i >= 232 {
                let v = 8 + (i - 232) * 10;
                rgb((v * 0x010101) as u32).into()
            } else {
                let i = i - 16;
                let n = |x: usize| if x == 0 { 0 } else { 55 + x * 40 };
                rgb(((n(i / 36) << 16) | (n(i / 6 % 6) << 8) | n(i % 6)) as u32).into()
            }
        }
    }
}
impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let paint_entity = entity.clone();
        div()
            .debug_selector(|| "terminal".into())
            .size_full()
            .min_h_0()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .bg(rgb(0x11151b))
            .overflow_hidden()
            .on_key_down(cx.listener(Self::key))
            .on_any_mouse_down(cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            this.bounds = bounds;
                        });
                    },
                    move |bounds, _, window, cx| {
                        paint_entity.update(cx, |this, cx| {
                            window.handle_input(
                                &this.focus,
                                ElementInputHandler::new(bounds, paint_entity.clone()),
                                cx,
                            );
                            let (w, h) = this.cell_size();
                            let content = this.term.renderable_content();
                            let cursor = content.cursor.point;
                            let offset = content.display_offset as i32;
                            paint_grid(&this.term, |layer, indexed| {
                                let cell = indexed.cell;
                                let row = indexed.point.line.0 + offset;
                                if row < 0 {
                                    return;
                                }
                                let origin = point(
                                    bounds.left() + px(indexed.point.column.0 as f32 * w),
                                    bounds.top() + px(row as f32 * h),
                                );
                                let mut fg = color(cell.fg);
                                let mut bg = color(cell.bg);
                                if cell.flags.contains(Flags::INVERSE) {
                                    std::mem::swap(&mut fg, &mut bg);
                                }
                                if content.selection.is_some_and(|s| s.contains(indexed.point)) {
                                    bg = rgb(0x36575e).into();
                                }
                                if indexed.point == cursor
                                    && content.cursor.shape != CursorShape::Hidden
                                {
                                    bg = rgb(0x8acdd0).into();
                                    fg = rgb(0x11151b).into();
                                }
                                if layer == CellLayer::Background {
                                    window.paint_quad(fill(
                                        Bounds::new(origin, size(px(w), px(h))),
                                        bg,
                                    ));
                                } else {
                                    let mut text = cell.c.to_string();
                                    if let Some(z) = cell.zerowidth() {
                                        text.extend(z);
                                    }
                                    let mut font = font(crate::platform::TERMINAL_FONT);
                                    if cell.flags.contains(Flags::BOLD) {
                                        font.weight = FontWeight::BOLD;
                                    }
                                    let line = window.text_system().shape_line(
                                        text.clone().into(),
                                        px(this.font_size),
                                        &[TextRun {
                                            len: text.len(),
                                            font,
                                            color: fg,
                                            background_color: None,
                                            underline: None,
                                            strikethrough: None,
                                        }],
                                        None,
                                    );
                                    let _ = line.paint(origin, px(h), window, cx);
                                }
                            });
                            if !this.marked.is_empty() {
                                let origin = point(
                                    bounds.left() + px(cursor.column.0 as f32 * w),
                                    bounds.top() + px(cursor.line.0.max(0) as f32 * h),
                                );
                                window.paint_quad(fill(
                                    Bounds::new(
                                        origin,
                                        size(px(this.marked.chars().count() as f32 * w), px(h)),
                                    ),
                                    rgb(0x36575e),
                                ));
                                let line = window.text_system().shape_line(
                                    this.marked.clone().into(),
                                    px(this.font_size),
                                    &[TextRun {
                                        len: this.marked.len(),
                                        font: font(crate::platform::TERMINAL_FONT),
                                        color: rgb(0xffffff).into(),
                                        background_color: None,
                                        underline: Some(UnderlineStyle {
                                            thickness: px(1.),
                                            color: None,
                                            wavy: false,
                                        }),
                                        strikethrough: None,
                                    }],
                                    None,
                                );
                                let _ = line.paint(origin, px(h), window, cx);
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{GridSize, Terminal};
    use alacritty_terminal::{
        grid::{Dimensions, Scroll},
        index::{Column, Line},
        term::TermMode,
    };
    use dgl_core::{
        connection::Connection,
        model::{AuthKind, Credential, HostProfile},
    };
    use gpui::{AppContext, TestAppContext};
    #[test]
    fn pane_and_window_share_one_cell_size() {
        assert_eq!(super::cell_size_for(10.0), (10.0 * 0.61, 10.0 * 1.4));
        assert_eq!(super::cell_size_for(28.0), (28.0 * 0.61, 28.0 * 1.4));
    }
    #[gpui::test]
    fn same_pane_snapshot_preserves_history(cx: &mut TestAppContext) {
        // This runtime deliberately stays unpolled: an in-flight request cannot complete.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = runtime.enter();
        let host = HostProfile {
            id: "fixture".into(),
            alias: "fixture".into(),
            hostname: "127.0.0.1".into(),
            port: 1,
            username: "test".into(),
            auth: AuthKind::Password,
            credential_ref: "fixture".into(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        };
        let connection = Connection::start(host, Credential::default(), None);
        let terminal = cx.new(|cx| Terminal::new(connection.clone(), 14, 5000, cx));
        terminal.update(cx, |t, _| {
            t.term.resize(GridSize(80, 10));
            t.begin_snapshot("%1", b"\x1bc");
            for _ in 0..80 {
                t.parser.advance(&mut t.term, b"retained history\r\n");
            }
            t.term.scroll_display(Scroll::Delta(20));
            let history = t.term.history_size();
            let offset = t.term.grid().display_offset();
            assert!(history > 20);
            t.snapshot_modes = None;
            t.parser.advance(&mut t.term, b"\x1b[?1h\x1b[?2004h");
            t.marked = "한".into();
            t.begin_snapshot("%1", b"\x1bc");
            assert!(
                t.input_modes()
                    .contains(TermMode::APP_CURSOR | TermMode::BRACKETED_PASTE)
            );
            assert_eq!(
                t.marked, "한",
                "size snapshots preserve the live IME composition"
            );
            assert_eq!(t.term.history_size(), history);
            assert_eq!(t.term.grid().display_offset(), offset);
            assert_eq!(t.term.grid()[Line(-1)][Column(0)].c, 'r');
            t.parser.advance(&mut t.term, b"\x1b[?1049hALT");
            assert!(t.term.mode().contains(TermMode::ALT_SCREEN));
            t.begin_snapshot("%1", b"\x1bc");
            assert!(!t.term.mode().contains(TermMode::ALT_SCREEN));
            assert_eq!(t.term.history_size(), history);
            t.begin_snapshot("%2", b"\x1bc");
            assert_eq!(t.term.history_size(), 0);
        });
        connection.close();
        runtime.block_on(connection.join());
    }
}
