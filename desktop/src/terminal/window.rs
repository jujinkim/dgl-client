//! One subscription and viewport per SSH connection; one persistent emulator per pane.
use super::*;
use dgl_core::{command::Operation, flow::OutputEvent, flow::PaneTarget, model::RemotePane};
use dgl_core::{flow::OutputKind, model::Size};
use std::collections::BTreeMap;

pub struct WindowTerminal {
    pub connection: Arc<Connection>,
    subscription: u64,
    panes: BTreeMap<String, Entity<Terminal>>,
    focus: FocusHandle,
    font_size: i32,
    scrollback: i32,
    requested: Size,
    sent_size: Option<Size>,
    resize_request: Option<u64>,
    drag: Option<(RemotePane, Point<Pixels>, bool, PaneTarget)>,
    pub enabled: bool,
    pub notice: Option<String>,
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
        let shell = cx.new(|cx| {
            Terminal::build(
                connection.clone(),
                Some("shell".into()),
                font_size,
                scrollback,
                cx,
            )
        });
        Self {
            connection,
            subscription,
            panes: BTreeMap::from([("shell".into(), shell)]),
            focus: cx.focus_handle(),
            font_size,
            scrollback,
            requested: Size::default(),
            sent_size: None,
            resize_request: None,
            drag: None,
            enabled: true,
            notice: None,
            #[cfg(test)]
            sent: Vec::new(),
        }
    }
    fn pane(&mut self, id: &str, cx: &mut Context<Self>) -> Entity<Terminal> {
        self.panes
            .entry(id.to_owned())
            .or_insert_with(|| {
                cx.new(|cx| {
                    Terminal::build(
                        self.connection.clone(),
                        Some(id.to_owned()),
                        self.font_size,
                        self.scrollback,
                        cx,
                    )
                })
            })
            .clone()
    }
    fn accept(&mut self, e: OutputEvent, cx: &mut Context<Self>) {
        let state = self.connection.state();
        let valid = if e.target.pane_id == "shell" {
            self.connection.target().as_ref() == Some(&e.target)
        } else {
            state.render_targets.get(&e.target.pane_id) == Some(&e.target)
                && matches!(
                    state.phase,
                    dgl_core::connection::Phase::Ready | dgl_core::connection::Phase::Restoring
                )
        };
        if !valid {
            return;
        }
        let terminal = self.pane(&e.target.pane_id, cx);
        terminal.update(cx, |t, cx| {
            match e.kind {
                OutputKind::Resize => t
                    .term
                    .resize(GridSize(e.size.columns as usize, e.size.rows as usize)),
                OutputKind::ResetBegin => t.begin_snapshot(&e.target.pane_id, &e.bytes),
                _ => t.parser.advance(&mut t.term, &e.bytes),
            }
            cx.notify();
        });
    }
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        if self.resize_request.is_none()
            && self.sent_size != Some(self.requested)
            && let Ok(id) = self.connection.action(Action::Resize {
                columns: self.requested.columns,
                rows: self.requested.rows,
            })
        {
            self.resize_request = Some(id);
            self.sent_size = Some(self.requested);
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
                if let Some(n) = t.notice.take() {
                    self.notice = Some(n);
                }
            });
        }
    }
    pub fn send(&mut self, bytes: Vec<u8>) {
        if !self.enabled {
            return;
        }
        #[cfg(test)]
        self.sent.push(bytes.clone());
        if let Some(target) = self.connection.target()
            && let Err(e) = self.connection.input(target, bytes)
        {
            self.notice = Some(format!("입력을 보내지 못했습니다: {e}"));
        }
    }
    pub fn prepare_paste(
        &self,
        target: &PaneTarget,
        text: &str,
        enter: bool,
        cx: &App,
    ) -> Result<Vec<u8>, String> {
        let pane = self
            .panes
            .get(&target.pane_id)
            .ok_or("현재 패널의 입력 모드를 확인하지 못했습니다. 잠시 후 다시 시도하세요.")?;
        let bracketed = pane
            .read(cx)
            .term
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        let bytes = input::paste_bytes(text, bracketed, enter);
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(
                "입력은 붙여넣기 제어 문자와 Enter를 포함하여 UTF-8 64 KiB 이하여야 합니다.".into(),
            );
        }
        Ok(bytes)
    }
    pub fn complete(&mut self, id: u64, cx: &mut App) -> bool {
        if self.resize_request == Some(id) {
            self.resize_request = None;
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
        self.font_size = n.clamp(10, 28);
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
            .or(Some("shell".into()))
            .and_then(|id| self.panes.get(&id))
            .map(|t| t.focus_handle(cx))
            .unwrap_or_else(|| self.focus.clone())
    }
}
impl Render for WindowTerminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.connection.state();
        let cell = (self.font_size as f32 * 0.61, self.font_size as f32 * 1.4);
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
                t.set_font(self.font_size);
                t.enabled = self.enabled;
            });
            content = content.child(t);
        }
        for p in &panes {
            let terminal = self.pane(&p.id, cx);
            terminal.update(cx, |t, _| {
                t.set_font(self.font_size);
                t.enabled = self.enabled;
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
        div()
            .id("window-terminal")
            .relative()
            .size_full()
            .min_h_0()
            .track_focus(&self.focus)
            .overflow_scroll()
            .bg(rgb(0x11151b))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseUpEvent, _, cx| {
                    if let Some((pane, origin, horizontal, generation)) = this.drag.take() {
                        if this.connection.state().render_targets.get(&pane.id) != Some(&generation)
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
                            this.requested = Size {
                                columns: (f32::from(bounds.size.width) / cell.0)
                                    .floor()
                                    .clamp(2., 500.)
                                    as u16,
                                rows: (f32::from(bounds.size.height) / cell.1)
                                    .floor()
                                    .clamp(2., 250.) as u16,
                            };
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::{Connection, Size, WindowTerminal};
    use dgl_core::model::{Credential, HostProfile};
    use gpui::{AppContext, TestAppContext};
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
            assert_eq!(t.sent_size.unwrap().columns, 90);
            t.complete(request, cx);
            t.tick(cx);
            assert_ne!(t.resize_request, Some(request));
            assert_eq!(t.sent_size, Some(Size::new(149, 31).unwrap()));
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
                t.prepare_paste(&target, "한글\r\n둘\n끝", true, cx)
                    .unwrap(),
                "한글\r둘\r끝\r".as_bytes()
            );
            assert_eq!(
                t.prepare_paste(&target, &"a".repeat(65536), false, cx)
                    .unwrap()
                    .len(),
                65536
            );
            assert!(
                t.prepare_paste(&target, &"a".repeat(65536), true, cx)
                    .is_err()
            );
            let pane = t.panes["shell"].clone();
            pane.update(cx, |pane, _| {
                pane.parser.advance(&mut pane.term, b"\x1b[?2004h")
            });
            assert_eq!(
                t.prepare_paste(&target, "한\n\x1b끝", true, cx).unwrap(),
                "\x1b[200~한\r끝\x1b[201~\r".as_bytes()
            );
            assert_eq!(
                t.prepare_paste(&target, &"a".repeat(65523), true, cx)
                    .unwrap()
                    .len(),
                65536
            );
            assert!(
                t.prepare_paste(&target, &"a".repeat(65524), true, cx)
                    .is_err()
            );
            let mut missing = target.clone();
            missing.pane_id = "%99".into();
            assert!(t.prepare_paste(&missing, "hello", false, cx).is_err());
        });
        connection.close();
        runtime.block_on(connection.join());
    }
}
