use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point as CellPoint, Side},
    selection::{Selection, SelectionType},
    term::{Config, TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape, Processor},
};
use dgl_core::{
    connection::{Action, Connection},
    flow::OutputKind,
    model::Size,
};
use gpui::{prelude::*, *};
use std::{collections::HashSet, ops::Range, sync::Arc};
mod paint;
use paint::{CellLayer, paint_grid};
#[derive(Clone)]
struct Listener(Arc<Connection>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event
            && let Some(t) = self.0.target()
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
pub struct Terminal {
    pub connection: Arc<Connection>,
    subscription: u64,
    term: Term<Listener>,
    parser: Processor,
    focus: FocusHandle,
    font_size: f32,
    bounds: Bounds<Pixels>,
    marked: String,
    selection: Range<usize>,
    dragging: bool,
    requested: Size,
    sent_size: Size,
    resize_request: Option<u64>,
    displayed_pane: Option<String>,
    pub enabled: bool,
    #[cfg(test)]
    pub sent: Vec<Vec<u8>>,
    pub notice: Option<String>,
    input_requests: HashSet<u64>,
}
impl Terminal {
    pub fn new(
        connection: Arc<Connection>,
        font_size: i32,
        scrollback: i32,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = connection.output().subscribe().unwrap();
        let config = Config {
            scrolling_history: scrollback.clamp(100, 10000) as usize,
            ..Default::default()
        };
        let term = Term::new(config, &GridSize(80, 24), Listener(connection.clone()));
        let _ = connection.action(Action::Snapshot);
        Self {
            connection,
            subscription,
            term,
            parser: Processor::new(),
            focus: cx.focus_handle(),
            font_size: font_size.clamp(10, 28) as f32,
            bounds: Bounds::default(),
            marked: String::new(),
            selection: 0..0,
            dragging: false,
            requested: Size::default(),
            sent_size: Size::default(),
            resize_request: None,
            displayed_pane: None,
            enabled: true,
            #[cfg(test)]
            sent: Vec::new(),
            notice: None,
            input_requests: HashSet::new(),
        }
    }
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        if self.resize_request.is_none()
            && self.requested != self.sent_size
            && let Ok(id) = self.connection.action(Action::Resize {
                columns: self.requested.columns,
                rows: self.requested.rows,
            })
        {
            self.resize_request = Some(id);
            self.sent_size = self.requested;
        }
        let output = self.connection.output();
        for _ in 0..64 {
            let Ok(Some(e)) = output.poll(self.subscription) else {
                break;
            };
            if self.connection.target().as_ref() == Some(&e.target) {
                match e.kind {
                    OutputKind::Resize => self
                        .term
                        .resize(GridSize(e.size.columns as usize, e.size.rows as usize)),
                    OutputKind::ResetBegin => {
                        self.begin_snapshot(&e.target.pane_id, &e.bytes);
                    }
                    _ => self.parser.advance(&mut self.term, &e.bytes),
                }
            }
            let _ = output.ack(self.subscription, e.sequence);
            cx.notify();
        }
    }
    fn begin_snapshot(&mut self, pane: &str, reset: &[u8]) {
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
        if let Some(t) = self.connection.target() {
            match self.connection.input(t, bytes) {
                Ok(id) => {
                    self.input_requests.insert(id);
                }
                Err(e) => self.notice = Some(format!("입력을 보내지 못했습니다: {e}")),
            }
        }
    }
    pub fn complete(&mut self, id: u64) -> bool {
        if self.resize_request == Some(id) {
            self.resize_request = None;
        }
        self.input_requests.remove(&id)
    }
    pub fn set_font(&mut self, n: i32) {
        self.font_size = n.clamp(10, 28) as f32;
    }
    fn cell_size(&self) -> (f32, f32) {
        (self.font_size * 0.61, self.font_size * 1.4)
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
                let text = text.replace("\r\n", "\n").replace('\n', "\r");
                let bytes = if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
                    format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
                } else {
                    text.into_bytes()
                };
                self.send(bytes);
            }
            cx.stop_propagation();
            return;
        }
        if crate::platform::app_shortcut(&k.modifiers) {
            // Let the containing app handle its shortcuts without sending a
            // control byte to the remote shell first.
            return;
        }
        let application = self.term.mode().contains(TermMode::APP_CURSOR);
        let sequence = match k.key.as_str() {
            "enter" => Some("\r"),
            "backspace" => Some("\x7f"),
            "tab" => Some(if k.modifiers.shift { "\x1b[Z" } else { "\t" }),
            "escape" => Some("\x1b"),
            "up" => Some(if application { "\x1bOA" } else { "\x1b[A" }),
            "down" => Some(if application { "\x1bOB" } else { "\x1b[B" }),
            "right" => Some(if application { "\x1bOC" } else { "\x1b[C" }),
            "left" => Some(if application { "\x1bOD" } else { "\x1b[D" }),
            "home" => Some("\x1b[H"),
            "end" => Some("\x1b[F"),
            "delete" => Some("\x1b[3~"),
            "f1" => Some("\x1bOP"),
            "f2" => Some("\x1bOQ"),
            "f3" => Some("\x1bOR"),
            "f4" => Some("\x1bOS"),
            "f5" => Some("\x1b[15~"),
            "f6" => Some("\x1b[17~"),
            "f7" => Some("\x1b[18~"),
            "f8" => Some("\x1b[19~"),
            "f9" => Some("\x1b[20~"),
            "f10" => Some("\x1b[21~"),
            "f11" => Some("\x1b[23~"),
            "f12" => Some("\x1b[24~"),
            "pageup" => Some("\x1b[5~"),
            "pagedown" => Some("\x1b[6~"),
            _ => None,
        };
        if let Some(seq) = sequence {
            self.send(seq.as_bytes().to_vec());
            cx.stop_propagation();
        } else if k.modifiers.control && k.key.len() == 1 {
            let b = k.key.as_bytes()[0].to_ascii_uppercase();
            if (b'@'..=b'_').contains(&b) {
                self.send(vec![b & 31]);
                cx.stop_propagation();
            }
        } else if k.modifiers.alt && k.key.len() == 1 {
            self.send(format!("\x1b{}", k.key).into_bytes());
            cx.stop_propagation();
        }
    }
    fn mouse_report(&mut self, p: Point<Pixels>, code: u8, release: bool) -> bool {
        if self.term.mode().intersects(TermMode::MOUSE_MODE)
            && self.term.mode().contains(TermMode::SGR_MOUSE)
        {
            let cell = self.point(p);
            self.send(
                format!(
                    "\x1b[<{code};{};{}{}",
                    cell.column.0 + 1,
                    (cell.line.0 + self.term.grid().display_offset() as i32 + 1).max(1),
                    if release { 'm' } else { 'M' }
                )
                .into_bytes(),
            );
            true
        } else {
            false
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.connection.output().unsubscribe(self.subscription);
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
        self.marked.clear();
        self.selection = 0..0;
        if !text.is_empty() {
            self.send(text.as_bytes().to_vec());
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    if !this.enabled {
                        return;
                    }
                    window.focus(&this.focus);
                    this.dragging = true;
                    if e.modifiers.shift || !this.mouse_report(e.position, 0, false) {
                        this.term.selection = Some(Selection::new(
                            SelectionType::Simple,
                            this.point(e.position),
                            Side::Left,
                        ));
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.dragging {
                    let p = this.point(e.position);
                    if let Some(s) = this.term.selection.as_mut() {
                        s.update(p, Side::Right);
                    } else {
                        this.mouse_report(e.position, 32, false);
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, _| {
                    this.dragging = false;
                    if this.term.selection.is_none() {
                        this.mouse_report(e.position, 0, true);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                if !this.enabled {
                    return;
                }
                let delta = e.delta.pixel_delta(px(this.font_size));
                let lines = (f32::from(delta.y) / this.font_size).round() as i32;
                if !this.mouse_report(e.position, if lines > 0 { 64 } else { 65 }, false) {
                    this.term.scroll_display(Scroll::Delta(lines));
                }
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            this.bounds = bounds;
                            let (w, h) = this.cell_size();
                            let requested = Size::new(
                                (f32::from(bounds.size.width) / w).floor().clamp(2., 500.) as u16,
                                (f32::from(bounds.size.height) / h).floor().clamp(2., 250.) as u16,
                            )
                            .unwrap();
                            if this.requested != requested {
                                this.requested = requested;
                            }
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
        model::{AuthKind, Credential, HostProfile, Size},
    };
    use gpui::{AppContext, TestAppContext};
    #[gpui::test]
    fn resize_coalesces_and_same_pane_snapshot_preserves_history(cx: &mut TestAppContext) {
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
        terminal.update(cx, |t, cx| {
            t.requested = Size::new(90, 30).unwrap();
            t.tick(cx);
            let request = t.resize_request.unwrap();
            for columns in 91..150 {
                t.requested = Size::new(columns, 31).unwrap();
                t.tick(cx);
                assert_eq!(t.resize_request, Some(request));
            }
            assert_eq!(t.sent_size.columns, 90);
            t.complete(request);
            t.tick(cx);
            assert_ne!(t.resize_request, Some(request));
            assert_eq!(t.sent_size, Size::new(149, 31).unwrap());
            t.term.resize(GridSize(80, 10));
            t.begin_snapshot("%1", b"\x1bc");
            for _ in 0..80 {
                t.parser.advance(&mut t.term, b"retained history\r\n");
            }
            t.term.scroll_display(Scroll::Delta(20));
            let history = t.term.history_size();
            let offset = t.term.grid().display_offset();
            assert!(history > 20);
            t.begin_snapshot("%1", b"\x1bc");
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
