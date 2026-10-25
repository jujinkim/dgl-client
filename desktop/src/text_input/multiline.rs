use super::*;
use gpui::{ContentMask, ScrollWheelEvent, TextAlign, WrappedLine};

const LINE_HEIGHT: f32 = 24.;

#[derive(Default)]
pub(super) struct Multiline {
    lines: Vec<EditorLine>,
    bounds: Bounds<Pixels>,
    scroll: Pixels,
    height: Pixels,
    reveal: bool,
}

struct EditorLine {
    start: usize,
    y: Pixels,
    shaped: WrappedLine,
}

impl Multiline {
    fn position(&self, index: usize) -> Point<Pixels> {
        let Some(line) = self.lines.iter().rev().find(|line| index >= line.start) else {
            return point(px(0.), px(0.));
        };
        let mut p = line
            .shaped
            .position_for_index((index - line.start).min(line.shaped.len()), px(LINE_HEIGHT))
            .unwrap_or_default();
        p.y += line.y;
        p
    }

    fn local_index(&self, position: Point<Pixels>, len: usize) -> usize {
        let Some(line) = self.lines.iter().rev().find(|line| position.y >= line.y) else {
            return 0;
        };
        let index = line
            .shaped
            .closest_index_for_position(position - point(px(0.), line.y), px(LINE_HEIGHT))
            .unwrap_or_else(|index| index);
        (line.start + index).min(len)
    }

    pub(super) fn index_at(&self, position: Point<Pixels>, len: usize) -> usize {
        self.local_index(
            position - self.bounds.origin + point(px(0.), self.scroll),
            len,
        )
    }

    pub(super) fn bounds_for_range(&self, range: Range<usize>) -> Option<Bounds<Pixels>> {
        if self.lines.is_empty() {
            return None;
        }
        let start = self.position(range.start);
        let end = self.position(range.end);
        let origin = self.bounds.origin - point(px(0.), self.scroll);
        Some(Bounds::new(
            origin + start,
            size(
                if start.y == end.y {
                    (end.x - start.x).max(px(1.))
                } else {
                    px(1.)
                },
                px(LINE_HEIGHT),
            ),
        ))
    }
}

impl TextInput {
    pub(super) fn reveal_cursor(&mut self) {
        if let Some(multiline) = &mut self.multiline {
            multiline.reveal = true;
        }
    }

    pub(super) fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline.is_some() && !self.is_composing() {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn vertical(&mut self, direction: f32, select: bool, cx: &mut Context<Self>) {
        let Some(multiline) = &self.multiline else {
            return;
        };
        let mut p = multiline.position(self.cursor_offset());
        p.y += px(direction * LINE_HEIGHT + LINE_HEIGHT / 2.);
        let offset = multiline.local_index(p, self.content.len());
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }
    pub(super) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., false, cx);
    }
    pub(super) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., false, cx);
    }
    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., true, cx);
    }
    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., true, cx);
    }

    pub(super) fn scroll_multiline(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(multiline) = &mut self.multiline {
            multiline.scroll = (multiline.scroll - event.delta.pixel_delta(px(LINE_HEIGHT)).y)
                .clamp(
                    px(0.),
                    (multiline.height - multiline.bounds.size.height).max(px(0.)),
                );
            multiline.reveal = false;
            cx.stop_propagation();
            cx.notify();
        }
    }
}

pub(super) struct MultilineElement {
    pub input: Entity<TextInput>,
}
impl IntoElement for MultilineElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MultilineElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = window.text_style();
        self.input.update(cx, |input, _| {
            let text = if input.content.is_empty() {
                input.placeholder.clone()
            } else {
                input.content.clone()
            };
            let run = TextRun {
                len: text.len(),
                font: style.font(),
                color: rgb(if input.content.is_empty() {
                    input.palette.muted
                } else {
                    input.palette.fg
                })
                .into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = if let Some(marked) = &input.marked_range {
                vec![
                    TextRun {
                        len: marked.start,
                        ..run.clone()
                    },
                    TextRun {
                        len: marked.len(),
                        underline: Some(UnderlineStyle {
                            color: Some(run.color),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len() - marked.end,
                        ..run
                    },
                ]
            } else {
                vec![run]
            };
            let shaped = window
                .text_system()
                .shape_text(
                    text,
                    style.font_size.to_pixels(window.rem_size()),
                    &runs,
                    Some(bounds.size.width.max(px(1.))),
                    None,
                )
                .unwrap();
            let cursor = input.cursor_offset();
            let multiline = input.multiline.as_mut().unwrap();
            if multiline.bounds.size != bounds.size {
                multiline.reveal = true;
            }
            multiline.bounds = bounds;
            let mut start = 0;
            let mut y = px(0.);
            multiline.lines = shaped
                .into_iter()
                .map(|shaped| {
                    let line = EditorLine { start, y, shaped };
                    start += line.shaped.len() + 1;
                    y += line.shaped.size(px(LINE_HEIGHT)).height;
                    line
                })
                .collect();
            multiline.height = y;
            if multiline.reveal {
                let p = multiline.position(cursor);
                if p.y < multiline.scroll {
                    multiline.scroll = p.y;
                }
                if p.y + px(LINE_HEIGHT) > multiline.scroll + bounds.size.height {
                    multiline.scroll = p.y + px(LINE_HEIGHT) - bounds.size.height;
                }
                multiline.reveal = false;
            }
            multiline.scroll = multiline
                .scroll
                .clamp(px(0.), (y - bounds.size.height).max(px(0.)));
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        self.input.update(cx, |input, cx| {
            let multiline = input.multiline.as_ref().unwrap();
            let origin = bounds.origin - point(px(0.), multiline.scroll);
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                if !input.selected_range.is_empty() {
                    let start = multiline.position(input.selected_range.start);
                    let end = multiline.position(input.selected_range.end);
                    let mut y = start.y;
                    while y <= end.y {
                        let left = if y == start.y { start.x } else { px(0.) };
                        let right = if y == end.y { end.x } else { bounds.size.width };
                        window.paint_quad(fill(
                            Bounds::new(
                                origin + point(left, y),
                                size((right - left).max(px(0.)), px(LINE_HEIGHT)),
                            ),
                            rgba(input.palette.selection << 8 | 0xff),
                        ));
                        y += px(LINE_HEIGHT);
                    }
                }
                for line in &multiline.lines {
                    let p = origin + point(px(0.), line.y);
                    if p.y + line.shaped.size(px(LINE_HEIGHT)).height >= bounds.top()
                        && p.y <= bounds.bottom()
                    {
                        line.shaped
                            .paint(p, px(LINE_HEIGHT), TextAlign::Left, None, window, cx)
                            .unwrap();
                    }
                }
                if input.focus_handle.is_focused(window) {
                    let p = origin + multiline.position(input.cursor_offset());
                    window.paint_quad(fill(
                        Bounds::new(p, size(px(1.), px(LINE_HEIGHT))),
                        rgb(input.palette.fg),
                    ));
                }
                if multiline.height > bounds.size.height {
                    let height = (bounds.size.height
                        * (f32::from(bounds.size.height) / f32::from(multiline.height)))
                    .max(px(8.));
                    let y = (bounds.size.height - height)
                        * (f32::from(multiline.scroll)
                            / f32::from(multiline.height - bounds.size.height));
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.right() - px(3.), bounds.top() + y),
                            size(px(3.), height),
                        ),
                        rgb(input.palette.muted),
                    ));
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, EntityInputHandler, Modifiers, TestAppContext};

    struct Editor(Entity<TextInput>);
    impl Render for Editor {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(220.)).child(self.0.clone())
        }
    }

    #[gpui::test]
    fn multiline_wrap_navigation_selection_clipboard_ime_and_scroll(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (view, cx) = cx.add_window_view(|_, cx| Editor(cx.new(TextInput::multiline)));
        let input = view.read_with(cx, |view, _| view.0.clone());
        cx.run_until_parked();
        cx.update(|w, cx| w.focus(&input.focus_handle(cx)));
        cx.simulate_input("한글 😀");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("둘째 줄");
        cx.simulate_keystrokes("home shift-right shift-right");
        cx.simulate_keystrokes(&format!("{}-c", crate::platform::EDIT_MODIFIER));
        cx.update(|_, cx| assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "둘째"));
        cx.simulate_keystrokes("end enter");
        cx.simulate_keystrokes(&format!("{}-v", crate::platform::EDIT_MODIFIER));
        assert_eq!(
            input.read_with(cx, |i, _| i.content.to_string()),
            "한글 😀\n둘째 줄\n둘째"
        );
        cx.run_until_parked();
        cx.simulate_keystrokes("up home");
        assert_eq!(
            input.read_with(cx, |i, _| i.cursor_offset()),
            "한글 😀\n".len()
        );
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                i.replace_and_mark_text_in_range(None, "ㅎ", Some(1..1), w, cx);
                i.replace_and_mark_text_in_range(None, "한", Some(1..1), w, cx);
            })
        });
        cx.run_until_parked();
        let (caret, index) = cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                let range = i.selected_text_range(false, w, cx).unwrap().range;
                let bounds = i.multiline.as_ref().unwrap().bounds;
                let caret = i.bounds_for_range(range, bounds, w, cx).unwrap();
                (
                    caret,
                    i.character_index_for_point(caret.center(), w, cx).unwrap(),
                )
            })
        });
        assert!(caret.size.height > px(0.));
        assert_eq!(index, "한글 😀\n한".encode_utf16().count());
        cx.simulate_keystrokes("enter");
        assert!(input.read_with(cx, |i, _| i.is_composing()));
        cx.update(|w, cx| input.update(cx, |i, cx| i.replace_text_in_range(None, "한", w, cx)));
        assert!(!input.read_with(cx, |i, _| i.is_composing()));
        cx.update(|_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("붙여넣기\r\n다음\r끝".into()));
        });
        cx.simulate_keystrokes(&format!("{}-v", crate::platform::EDIT_MODIFIER));
        assert!(input.read_with(cx, |i, _| i.content.contains("붙여넣기\n다음\n끝")));
        input.update(cx, |i, cx| {
            i.set_content(
                &format!(
                    "{}\n{}",
                    "자동 줄바꿈 한글 😀 ".repeat(20),
                    "줄\n".repeat(30)
                ),
                cx,
            )
        });
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        input.read_with(cx, |i, _| {
            let m = i.multiline.as_ref().unwrap();
            assert!(!m.lines[0].shaped.wrap_boundaries().is_empty());
            assert!(m.height > m.bounds.size.height);
            assert!(m.scroll > px(0.));
        });
        let bounds = input.read_with(cx, |i, _| i.multiline.as_ref().unwrap().bounds);
        cx.simulate_event(ScrollWheelEvent {
            position: bounds.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(2000.))),
            modifiers: Modifiers::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();
        assert_eq!(
            input.read_with(cx, |i, _| i.multiline.as_ref().unwrap().scroll),
            px(0.)
        );
        let point = bounds.origin + point(px(3.), px(4.));
        cx.simulate_click(point, Modifiers::default());
        assert!(input.read_with(cx, |i, _| i.cursor_offset()) < 10);
    }

    #[gpui::test]
    fn pending_editor_rejects_every_mutation_and_single_line_behavior_is_preserved(
        cx: &mut TestAppContext,
    ) {
        cx.update(bind_keys);
        let (view, cx) = cx.add_window_view(|_, cx| Editor(cx.new(TextInput::multiline)));
        let input = view.read_with(cx, |view, _| view.0.clone());
        input.update(cx, |i, cx| {
            i.set_content("초안\n😀", cx);
            i.read_only = true;
        });
        cx.update(|w, cx| {
            w.focus(&input.focus_handle(cx));
            cx.write_to_clipboard(ClipboardItem::new_string("붙여넣기".into()));
        });
        cx.simulate_keystrokes(&format!(
            "{}-a backspace delete enter {}-x {}-v",
            crate::platform::EDIT_MODIFIER,
            crate::platform::EDIT_MODIFIER,
            crate::platform::EDIT_MODIFIER
        ));
        cx.simulate_input("입력");
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                i.replace_and_mark_text_in_range(None, "ㅎ", None, w, cx)
            })
        });
        assert_eq!(
            input.read_with(cx, |i, _| i.content.to_string()),
            "초안\n😀"
        );
        input.update(cx, |i, cx| {
            *i = TextInput::new("single", "", false, cx);
        });
        cx.update(|w, cx| {
            w.focus(&input.focus_handle(cx));
            input.update(cx, |i, cx| i.replace_text_in_range(None, "\n", w, cx));
            cx.write_to_clipboard(ClipboardItem::new_string("a\nb".into()));
        });
        cx.run_until_parked();
        cx.simulate_keystrokes(&format!("{}-v", crate::platform::EDIT_MODIFIER));
        assert_eq!(
            input.read_with(cx, |i, _| i.content.to_string()),
            "singlea b"
        );
    }
}
