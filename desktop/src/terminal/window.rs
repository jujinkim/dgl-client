//! One subscription and viewport per SSH connection; one persistent emulator per pane.
use super::*;
use crate::native_tooltips::NativeTooltip;
use dgl_core::terminal_font::TerminalFontSize;
use dgl_core::{command::Operation, flow::OutputEvent, flow::PaneTarget, model::RemotePane};
use dgl_core::{flow::OutputKind, model::Size};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub struct WindowTerminal {
    pub connection: Arc<Connection>,
    subscription: u64,
    panes: BTreeMap<String, Entity<Terminal>>,
    focus: FocusHandle,
    pub(super) font: TerminalFontSize,
    scrollback: i32,
    requested: Size,
    applied_size: Option<Size>,
    resize_request: Option<(u64, Size)>,
    resize_due: Instant,
    last_display: Option<Arc<dgl_core::connection::State>>,
    rejection_shown: bool,
    drag: Option<(RemotePane, Point<Pixels>, bool, PaneTarget)>,
    pub enabled: bool,
    pub notice: Option<SnackbarRequest>,
    #[cfg(test)]
    pub sent: Vec<Vec<u8>>,
}
impl WindowTerminal {
    pub fn new(
        connection: Arc<Connection>,
        font_size: i32,
        scrollback: i32,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = connection.output().subscribe().unwrap();
        let _ = connection.action(Action::Snapshot);
        let owner = cx.entity().downgrade();
        let shell = cx.new(|cx| {
            let mut terminal = Terminal::build(
                connection.clone(),
                Some("shell".into()),
                font_size,
                scrollback,
                cx,
            );
            terminal.font_owner = Some(owner);
            terminal
        });
        Self {
            connection,
            subscription,
            panes: BTreeMap::from([("shell".into(), shell)]),
            focus: cx.focus_handle(),
            font: TerminalFontSize::new(font_size),
            scrollback,
            requested: Size::default(),
            applied_size: None,
            resize_request: None,
            resize_due: Instant::now(),
            last_display: None,
            rejection_shown: false,
            drag: None,
            enabled: true,
            notice: None,
            #[cfg(test)]
            sent: Vec::new(),
        }
    }
    fn pane(&mut self, id: &str, cx: &mut Context<Self>) -> Entity<Terminal> {
        let owner = cx.entity().downgrade();
        self.panes
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    let mut terminal = Terminal::build(
                        self.connection.clone(),
                        Some(id.to_owned()),
                        self.font.effective(),
                        self.scrollback,
                        cx,
                    );
                    terminal.font_owner = Some(owner);
                    terminal
                })
            })
            .clone()
    }
    fn accept(&mut self, e: OutputEvent, cx: &mut Context<Self>) {
        if !self.connection.state().accepts_output(&e.target) {
            return;
        }
        let terminal = self.pane(&e.target.pane_id, cx);
        terminal.update(cx, |t, cx| {
            match e.kind {
                OutputKind::Resize => t
                    .term
                    .resize(GridSize(e.size.columns as usize, e.size.rows as usize)),
                OutputKind::ResetBegin => t.begin_snapshot(&e.target.pane_id, &e.bytes),
                OutputKind::ResetEnd => {
                    t.parser.advance(&mut t.term, &e.bytes);
                    t.snapshot_modes = None;
                }
                _ => t.parser.advance(&mut t.term, &e.bytes),
            }
            cx.notify();
        });
    }
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        if self.connection.state().accepts_input() {
            self.rejection_shown = false;
        }
        if self.resize_request.is_none()
            && self.applied_size != Some(self.requested)
            && Instant::now() >= self.resize_due
        {
            match self.connection.action(Action::Resize {
                columns: self.requested.columns,
                rows: self.requested.rows,
            }) {
                Ok(id) => {
                    self.resize_request = Some((id, self.requested));
                    self.resize_due = Instant::now() + Duration::from_millis(50);
                }
                Err(_) => self.resize_due = Instant::now() + Duration::from_millis(150),
            }
        }
        let queue = self.connection.output();
        for _ in 0..128 {
            let Ok(Some(e)) = queue.poll(self.subscription) else {
                break;
            };
            let sequence = e.sequence;
            self.accept(e, cx);
            let _ = queue.ack(self.subscription, sequence);
            cx.notify();
        }
        for t in self.panes.values() {
            t.update(cx, |t, _| {
                t.enabled = self.enabled;
                t.pointer_enabled =
                    self.resize_request.is_none() && self.applied_size == Some(self.requested);
                t.sync_modifiers();
                if let Some(n) = t.notice.take() {
                    if !self.rejection_shown {
                        self.notice = Some(n);
                        self.rejection_shown = true;
                    }
                }
            });
        }
    }
    pub fn reject_input(&mut self, error: dgl_core::Error) {
        if self.rejection_shown {
            return;
        }
        self.rejection_shown = true;
        self.notice = Some(SnackbarRequest::new(
            tr!("입력을 보내지 못했습니다: {0}", error),
            SnackbarDuration::Short,
        ));
    }
    pub fn send(&mut self, bytes: Vec<u8>) {
        if !self.enabled {
            return;
        }
        #[cfg(test)]
        self.sent.push(bytes.clone());
        if self.connection.target().is_none() {
            self.reject_input(dgl_core::Error::NotReady);
        }
        if let Some(target) = self.connection.target()
            && let Err(e) = self.connection.input(target, bytes)
        {
            self.reject_input(e);
        }
    }
    pub fn clear_modifiers(&mut self, cx: &mut App) {
        for pane in self.panes.values() {
            pane.update(cx, |t, _| t.modifiers = KeyModifiers::default());
        }
    }
    pub fn modifiers(&self, cx: &App) -> KeyModifiers {
        self.connection
            .state()
            .pane
            .as_deref()
            .or(Some("shell"))
            .and_then(|id| self.panes.get(id))
            .map(|t| t.read(cx).modifiers)
            .unwrap_or_default()
    }
    pub fn shortcut(&mut self, key: TerminalShortcut, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let id = self
            .connection
            .state()
            .pane
            .clone()
            .unwrap_or_else(|| "shell".into());
        self.pane(&id, cx).update(cx, |t, cx| {
            t.shortcut(key);
            cx.notify();
        });
        cx.notify();
    }
    pub fn prepare_paste(
        &self,
        target: &PaneTarget,
        text: &str,
        cx: &App,
    ) -> Result<Vec<u8>, String> {
        let pane = self.panes.get(&target.pane_id).ok_or(dgl_core::i18n::t(
            "현재 패널의 입력 모드를 확인하지 못했습니다. 잠시 후 다시 시도하세요.",
        ))?;
        let bracketed = pane
            .read(cx)
            .input_modes()
            .contains(TermMode::BRACKETED_PASTE);
        let bytes = input::paste_bytes(text, bracketed);
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(dgl_core::i18n::t(
                "입력은 붙여넣기 제어 문자를 포함하여 UTF-8 64 KiB 이하여야 합니다.",
            )
            .into());
        }
        Ok(bytes)
    }
    pub fn complete(&mut self, id: u64, succeeded: bool, cx: &mut App) -> bool {
        if let Some((request, size)) = self.resize_request
            && request == id
        {
            self.applied_size = succeeded.then_some(size);
            self.resize_request = None;
            if !succeeded {
                self.resize_due = Instant::now() + Duration::from_millis(150);
            }
        }
        let mut input = false;
        for pane in self.panes.values() {
            input |= pane.update(cx, |t, _| t.complete(id));
        }
        input
    }
    #[cfg(test)]
    pub fn captured_inputs(&self, cx: &App) -> Vec<Vec<u8>> {
        let mut all = self.sent.clone();
        for t in self.panes.values() {
            all.extend(t.read(cx).sent.clone());
        }
        all
    }
    #[cfg(test)]
    pub fn clear_captured_inputs(&mut self, cx: &mut App) {
        self.sent.clear();
        for t in self.panes.values() {
            t.update(cx, |t, _| t.sent.clear());
        }
    }
    pub fn set_font(&mut self, n: i32) {
        self.font.configure(n);
    }
}
impl Drop for WindowTerminal {
    fn drop(&mut self) {
        self.connection.output().unsubscribe(self.subscription);
    }
}
impl Focusable for WindowTerminal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.connection
            .state()
            .pane
            .clone()
            .or(Some("shell".into()))
            .and_then(|id| self.panes.get(&id))
            .map(|t| t.focus_handle(cx))
            .unwrap_or_else(|| self.focus.clone())
    }
}
impl Render for WindowTerminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.connection.state();
        if current.pane.is_some()
            && matches!(
                current.phase,
                dgl_core::connection::Phase::Ready
                    | dgl_core::connection::Phase::Restoring
                    | dgl_core::connection::Phase::Shell
            )
        {
            self.last_display = Some(current.clone());
        }
        // Keep the previous pane layout while transport recovery clears its input target.
        let state = if current.pane.is_none() && current.can_retry_now() {
            self.last_display.clone().unwrap_or(current)
        } else {
            current
        };
        let font_size = self.font.effective();
        let cell = super::cell_size_for(font_size as f32);
        let remote_window = state.remote.windows.iter().find(|w| {
            Some(&w.id) == state.window.as_ref() && Some(&w.session) == state.session.as_ref()
        });
        let panes: Vec<_> = state
            .remote
            .panes
            .iter()
            .filter(|p| remote_window.is_some_and(|w| w.id == p.window && (!w.zoomed || p.active)))
            .cloned()
            .collect();
        let columns = remote_window
            .map_or(self.requested.columns, |w| w.width)
            .max(2);
        let rows = remote_window
            .map_or(self.requested.rows, |w| w.height)
            .max(2);
        let mut content = div()
            .relative()
            .w(px(columns as f32 * cell.0))
            .h(px(rows as f32 * cell.1))
            .flex_shrink_0();
        if state.pane.is_none() || state.pane.as_deref() == Some("shell") {
            let t = self.pane("shell", cx);
            t.update(cx, |t, _| {
                t.set_font(font_size);
                t.enabled = self.enabled;
                t.pointer_enabled =
                    self.resize_request.is_none() && self.applied_size == Some(self.requested);
                t.sync_modifiers();
            });
            content = content.child(t);
        }
        for p in &panes {
            let terminal = self.pane(&p.id, cx);
            terminal.update(cx, |t, _| {
                t.set_font(font_size);
                t.enabled = self.enabled;
                t.pointer_enabled =
                    self.resize_request.is_none() && self.applied_size == Some(self.requested);
                t.sync_modifiers();
            });
            content = content.child(
                div()
                    .absolute()
                    .left(px(p.left as f32 * cell.0))
                    .top(px(p.top as f32 * cell.1))
                    .w(px(p.width as f32 * cell.0))
                    .h(px(p.height as f32 * cell.1))
                    .child(terminal),
            );
            for horizontal in [true, false] {
                let boundary = if horizontal {
                    p.left + p.width
                } else {
                    p.top + p.height
                };
                let limit = if horizontal { columns } else { rows };
                if boundary >= limit {
                    continue;
                }
                let pane = p.clone();
                let generation = state.render_targets.get(&p.id).cloned();
                let mut separator = div()
                    .id(SharedString::from(format!(
                        "boundary-{}-{horizontal}",
                        p.id
                    )))
                    .absolute()
                    .bg(rgb(0x39434f));
                separator = if horizontal {
                    separator
                        .left(px(boundary as f32 * cell.0))
                        .top(px(p.top as f32 * cell.1))
                        .w(px(cell.0))
                        .h(px(p.height as f32 * cell.1))
                        .cursor_col_resize()
                } else {
                    separator
                        .left(px(p.left as f32 * cell.0))
                        .top(px(boundary as f32 * cell.1))
                        .w(px(p.width as f32 * cell.0))
                        .h(px(cell.1))
                        .cursor_row_resize()
                };
                content = content.child(separator.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        if this.enabled
                            && this.connection.state().accepts_pointer()
                            && this.resize_request.is_none()
                            && this.applied_size == Some(this.requested)
                            && let Some(generation) = generation.clone()
                        {
                            this.drag = Some((pane.clone(), e.position, horizontal, generation));
                            cx.stop_propagation();
                        }
                    }),
                ));
            }
        }
        // Follow shared tmux focus only while a terminal already owns keyboard focus.
        // A background SSH update must not take focus from a dialog or host editor.
        if self
            .panes
            .values()
            .any(|t| t.read(cx).focus.is_focused(window))
        {
            let focus = self.focus_handle(cx);
            if !focus.is_focused(window) {
                window.focus(&focus);
            }
        }
        // Transfer focus before dropping the old active pane after an external deletion.
        self.panes
            .retain(|id, _| id == "shell" || state.remote.panes.iter().any(|p| &p.id == id));
        let entity = cx.entity();
        let viewport = div()
            .id("window-terminal")
            .relative()
            .size_full()
            .min_h_0()
            .track_focus(&self.focus)
            .overflow_scroll()
            .bg(rgb(0x11151b))
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                if let Some(delta) = font_wheel_delta(event) {
                    this.font.adjust(delta);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseUpEvent, _, cx| {
                    if let Some((pane, origin, horizontal, generation)) = this.drag.take() {
                        if !this.connection.state().accepts_pointer()
                            || this.resize_request.is_some()
                            || this.applied_size != Some(this.requested)
                            || this.connection.state().render_targets.get(&pane.id)
                                != Some(&generation)
                        {
                            return;
                        }
                        let delta = if horizontal {
                            f32::from(e.position.x - origin.x) / cell.0
                        } else {
                            f32::from(e.position.y - origin.y) / cell.1
                        };
                        if delta.round() == 0. {
                            return;
                        }
                        let columns = (pane.width as f32
                            + if horizontal { delta.round() } else { 0. })
                        .clamp(1., 65535.) as u16;
                        let rows = (pane.height as f32
                            + if horizontal { 0. } else { delta.round() })
                        .clamp(1., 65535.) as u16;
                        let _ = this.connection.action(Action::Manage {
                            operation: Operation::ResizePaneTo {
                                target: pane.id,
                                columns,
                                rows,
                            },
                        });
                        cx.notify();
                    }
                }),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| {
                            let requested = Size {
                                columns: (f32::from(bounds.size.width) / cell.0)
                                    .floor()
                                    .clamp(2., 500.)
                                    as u16,
                                rows: (f32::from(bounds.size.height) / cell.1)
                                    .floor()
                                    .clamp(2., 250.) as u16,
                            };
                            if this.requested != requested {
                                this.requested = requested;
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(content);
        div()
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(viewport)
            .when(self.font.override_size().is_some(), |view| {
                view.child(
                    div()
                        .id("terminal-font-reset")
                        .debug_selector(|| "terminal-font-reset".into())
                        .absolute()
                        .top(px(8.))
                        .right(px(8.))
                        .h(px(28.))
                        .px(px(10.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.))
                        .bg(rgb(0x39434f))
                        .text_color(rgb(0xdce1e8))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .cursor_pointer()
                        .child(crate::ui_icons::UiIcon::FontSize.render(0xdce1e8))
                        .child(self.font.effective().to_string())
                        .child(crate::ui_icons::UiIcon::Refresh.render(0xdce1e8))
                        .native_tooltip(dgl_core::i18n::t("세션 글자 크기 초기화"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.font.reset();
                            cx.notify();
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{Connection, Size, WindowTerminal};
    use dgl_core::model::{Credential, HostProfile};
    use gpui::{AppContext, TestAppContext};
    use gpui::{
        Context, Entity, Focusable, IntoElement, Modifiers, Render, ScrollDelta, ScrollWheelEvent,
        TouchPhase, VisualTestContext, Window, div, point, prelude::*, px, size,
    };

    struct FontFixture {
        first: Entity<WindowTerminal>,
        second: Entity<WindowTerminal>,
        split: bool,
    }
    impl Render for FontFixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .size_full()
                .child(
                    div()
                        .debug_selector(|| "font-first-area".into())
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.first.clone()),
                )
                .when(self.split, |view| {
                    view.child(
                        div()
                            .debug_selector(|| "font-second-area".into())
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.second.clone()),
                    )
                })
        }
    }

    #[gpui::test]
    fn ctrl_wheel_and_reset_fab_preserve_focus_and_isolate_split_sessions(cx: &mut TestAppContext) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = runtime.enter();
        let host: HostProfile = serde_json::from_value(serde_json::json!({
            "id":"fixture", "alias":"fixture", "hostname":"127.0.0.1", "port":1,
            "username":"test", "credentialRef":"fixture"
        }))
        .unwrap();
        let connection = Connection::start(host.clone(), Credential::default(), None);
        let other = Connection::start(host, Credential::default(), None);
        let first = cx.new(|cx| WindowTerminal::new(connection.clone(), 14, 5000, cx));
        let second = cx.new(|cx| WindowTerminal::new(other.clone(), 14, 5000, cx));
        let (fixture, ui) = cx.add_window_view(|_, _| FontFixture {
            first: first.clone(),
            second: second.clone(),
            split: false,
        });
        for (width, split) in [(360., false), (1200., true)] {
            ui.simulate_resize(size(px(width), px(500.)));
            fixture.update(ui, |f, cx| {
                f.split = split;
                cx.notify();
            });
            first.update(ui, |t, cx| {
                t.font.reset();
                for pane in t.panes.values() {
                    pane.update(cx, |p, _| {
                        p.parser.advance(&mut p.term, b"\x1b[?1000h\x1b[?1006h")
                    });
                }
                cx.notify();
            });
            ui.run_until_parked();
            let focus = first.read_with(ui, |t, cx| t.focus_handle(cx));
            ui.update(|w, _| w.focus(&focus));
            let area = ui.debug_bounds("font-first-area").unwrap();
            let wheel = |ui: &mut VisualTestContext, delta, modifiers| {
                ui.simulate_event(ScrollWheelEvent {
                    position: area.center(),
                    delta,
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                });
                ui.run_until_parked();
            };
            let ctrl = Modifiers {
                control: true,
                ..Default::default()
            };
            wheel(ui, ScrollDelta::Lines(point(0., 1.)), ctrl);
            assert_eq!(first.read_with(ui, |t, _| t.font.override_size()), Some(15));
            assert_eq!(second.read_with(ui, |t, _| t.font.override_size()), None);
            wheel(ui, ScrollDelta::Pixels(point(px(0.), px(-20.))), ctrl);
            assert_eq!(first.read_with(ui, |t, _| t.font.effective()), 14);
            // Sticky Ctrl alone must not trigger zoom; the physical event owns it.
            first.update(ui, |t, cx| {
                t.shortcut(
                    dgl_core::input::TerminalShortcut::Modifier(dgl_core::input::Modifier::Ctrl),
                    cx,
                );
            });
            wheel(ui, ScrollDelta::Lines(point(0., 1.)), Modifiers::default());
            assert_eq!(first.read_with(ui, |t, _| t.font.effective()), 14);
            first.update(ui, |t, cx| {
                t.clear_captured_inputs(cx);
                t.set_font(18);
                cx.notify();
            });
            ui.run_until_parked();
            let reset = ui.debug_bounds("terminal-font-reset").unwrap();
            assert_eq!(reset.right(), area.right() - px(8.));
            assert_eq!(reset.top(), area.top() + px(8.));
            assert_eq!(reset.size.height, px(28.));
            assert!(reset.size.width > px(40.));
            assert!(reset.left() >= area.left() + px(8.));
            ui.simulate_click(reset.center(), Modifiers::default());
            ui.run_until_parked();
            assert_eq!(first.read_with(ui, |t, _| t.font.override_size()), None);
            assert_eq!(first.read_with(ui, |t, _| t.font.effective()), 18);
            assert!(ui.update(|w, _| focus.is_focused(w)));
            assert!(first.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
            first.update(ui, |t, _| t.set_font(14));
        }
        connection.close();
        other.close();
        runtime.block_on(async {
            connection.join().await;
            other.join().await;
        });
    }
    #[gpui::test]
    fn viewport_coalesces_at_window_scope(cx: &mut TestAppContext) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = runtime.enter();
        let host: HostProfile = serde_json::from_value(serde_json::json!({
            "id":"fixture", "alias":"fixture", "hostname":"127.0.0.1", "port":1,
            "username":"test", "credentialRef":"fixture"
        }))
        .unwrap();
        let connection = Connection::start(host, Credential::default(), None);
        let terminal = cx.new(|cx| WindowTerminal::new(connection.clone(), 14, 5000, cx));
        terminal.update(cx, |t, cx| {
            t.requested = Size::new(90, 30).unwrap();
            t.tick(cx);
            let request = t.resize_request.unwrap();
            for columns in 91..150 {
                t.requested = Size::new(columns, 31).unwrap();
                t.tick(cx);
                assert_eq!(t.resize_request, Some(request));
            }
            assert_eq!(request.1.columns, 90);
            assert_eq!(t.applied_size, None, "submission is not success");
            t.complete(request.0 + 100, true, cx);
            assert_eq!(t.resize_request, Some(request));
            t.complete(request.0, true, cx);
            t.tick(cx);
            assert!(t.resize_request.is_none(), "minimum 50 ms start interval");
            t.resize_due = std::time::Instant::now();
            t.tick(cx);
            assert_ne!(t.resize_request, Some(request));
            let latest = t.resize_request.unwrap();
            assert_eq!(latest.1, Size::new(149, 31).unwrap());
            t.complete(latest.0, false, cx);
            t.tick(cx);
            assert_eq!(t.resize_request, None, "failure retries are debounced");
            t.resize_due = std::time::Instant::now();
            t.tick(cx);
            let retry = t.resize_request.unwrap();
            assert_eq!(retry.1, latest.1, "the same failed size must be retried");
            t.complete(retry.0, true, cx);
            t.tick(cx);
            assert_eq!(t.resize_request, None);
            assert_eq!(t.applied_size, Some(latest.1));
        });
        connection.close();
        runtime.block_on(connection.join());
    }
    #[gpui::test]
    fn composer_uses_current_pane_mode_and_counts_control_bytes_in_limit(
        cx: &mut gpui::TestAppContext,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = runtime.enter();
        let host: HostProfile = serde_json::from_value(serde_json::json!({
            "id":"fixture", "alias":"fixture", "hostname":"127.0.0.1", "port":1,
            "username":"test", "credentialRef":"fixture"
        }))
        .unwrap();
        let connection = Connection::start(host, Credential::default(), None);
        let terminal = cx.new(|cx| WindowTerminal::new(connection.clone(), 14, 5000, cx));
        terminal.update(cx, |t, cx| {
            let target = dgl_core::flow::PaneTarget {
                connection: 0,
                pane_id: "shell".into(),
                generation: dgl_core::flow::Generation {
                    connection: 0,
                    pane: 0,
                    size: 0,
                },
            };
            assert_eq!(
                t.prepare_paste(&target, "한글\r\n둘\n끝", cx).unwrap(),
                "한글\r둘\r끝".as_bytes()
            );
            assert_eq!(
                t.prepare_paste(&target, &"a".repeat(65536), cx)
                    .unwrap()
                    .len(),
                65536
            );
            assert!(t.prepare_paste(&target, &"a".repeat(65537), cx).is_err());
            let pane = t.panes["shell"].clone();
            pane.update(cx, |pane, _| {
                pane.parser.advance(&mut pane.term, b"\x1b[?2004h")
            });
            assert_eq!(
                t.prepare_paste(&target, "한\n\x1b끝", cx).unwrap(),
                "\x1b[200~한\r끝\x1b[201~".as_bytes()
            );
            assert_eq!(
                t.prepare_paste(&target, &"a".repeat(65524), cx)
                    .unwrap()
                    .len(),
                65536
            );
            assert!(t.prepare_paste(&target, &"a".repeat(65525), cx).is_err());
            let mut missing = target.clone();
            missing.pane_id = "%99".into();
            assert!(t.prepare_paste(&missing, "hello", cx).is_err());
        });
        connection.close();
        runtime.block_on(connection.join());
    }
}
