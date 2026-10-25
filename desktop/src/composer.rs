//! Memory-only drafts. The core owns queueing and validates the captured destination again.
use super::*;
use dgl_core::{
    connection::{Completion, CompletionOutcome},
    flow::PaneTarget,
};

pub(super) struct Composer {
    pub input: Entity<TextInput>,
    pub include_enter: bool,
    target: Option<Destination>,
    pub pending: Option<u64>,
    pub error: Option<String>,
}

pub(super) fn complete(
    composers: &mut HashMap<TabId, Composer>,
    dialog: &mut Option<Dialog>,
    id: TabId,
    result: &Completion,
    cx: &mut App,
) -> bool {
    let Some(composer) = composers.get_mut(&id) else {
        return false;
    };
    if !composer.complete(result, cx) {
        return false;
    }
    if composer.error.is_none() {
        composers.remove(&id);
        if matches!(dialog, Some(Dialog::Compose(tab)) if *tab == id) {
            *dialog = None;
        }
    }
    true
}

#[derive(Clone)]
struct Destination {
    target: PaneTarget,
    session: Option<String>,
}
impl Destination {
    fn capture(connection: &Connection) -> Option<Self> {
        Some(Self {
            target: connection.target()?,
            session: connection.state().session,
        })
    }
    fn matches(&self, current: &Self) -> bool {
        self.session == current.session && self.target.same_input_destination(&current.target)
    }
}

impl Composer {
    fn submit(
        &mut self,
        text: &str,
        current: Option<Destination>,
        selected: bool,
        send: impl FnOnce(&PaneTarget, &str, bool) -> Result<u64, String>,
    ) {
        if self.pending.is_some() {
            return;
        }
        let result = (|| {
            if text.is_empty() {
                return Err(t("입력할 텍스트를 작성하세요.").into());
            }
            if text.len() > 65536 {
                return Err(t("입력은 UTF-8 64 KiB 이하여야 합니다.").into());
            }
            let destination = self
                .target
                .as_ref()
                .filter(|destination| {
                    selected
                        && current
                            .as_ref()
                            .is_some_and(|current| destination.matches(current))
                })
                .ok_or(t(
                    "연결·세션·패널이 변경되어 전송을 취소했습니다. 초안을 확인한 뒤 다시 여세요.",
                ))?;
            send(&destination.target, text, self.include_enter)
        })();
        match result {
            Ok(request) => {
                self.pending = Some(request);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub fn complete(&mut self, result: &Completion, cx: &mut App) -> bool {
        if self.pending != Some(result.request_id) {
            return false;
        }
        self.pending = None;
        self.input.update(cx, |input, cx| {
            input.read_only = false;
            cx.notify();
        });
        self.error = if result.outcome == CompletionOutcome::Sent && result.error.is_none() {
            self.input.update(cx, |input, cx| input.set_content("", cx));
            self.include_enter = false;
            self.target = None;
            None
        } else {
            Some(tr!(
                "전송 실패: {0}. 초안은 유지되며 자동 재전송하지 않습니다.",
                result
                    .error
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| t("전송 결과를 확인하지 못했습니다").into())
            ))
        };
        true
    }
}

impl Desktop {
    pub(super) fn open_composer(&mut self, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            return;
        }
        let Some(tab) = self.active_tab() else {
            return;
        };
        let id = tab.id;
        let target = Destination::capture(&tab.connection);
        let composer = self.composers.entry(id).or_insert_with(|| Composer {
            input: cx.new(TextInput::multiline),
            include_enter: false,
            target: None,
            pending: None,
            error: None,
        });
        if composer.pending.is_none() {
            composer.target = target;
        }
        self.launcher.opened = None;
        self.menu = None;
        self.dialog = Some(Dialog::Compose(id));
    }

    pub(super) fn dismiss_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(Dialog::Compose(id)) = self.dialog
            && let Some(composer) = self.composers.get(&id)
        {
            // Preserve the visible draft without carrying an abandoned IME session into reopening.
            composer
                .input
                .update(cx, |input, cx| input.end_composition(cx));
        }
        self.dialog = None;
    }

    pub(super) fn send_composer(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Compose(id)) = self.dialog else {
            return;
        };
        let selected = self.workspace.selected() == Some(id);
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let composer = self.composers.get_mut(&id).unwrap();
        if composer.pending.is_some() || composer.input.read(cx).is_composing() {
            return;
        }
        let text = composer.input.read(cx).content.to_string();
        composer.submit(
            &text,
            Destination::capture(&tab.connection),
            selected,
            |target, text, enter| {
                let bytes = tab
                    .terminal
                    .update(cx, |t, cx| t.prepare_paste(target, text, enter, cx))?;
                tab.connection
                    .input(target.clone(), bytes)
                    .map_err(|e| tr!("전송 실패: {0}. 초안은 유지됩니다.", e))
            },
        );
        composer.input.update(cx, |input, cx| {
            input.read_only = composer.pending.is_some();
            cx.notify();
        });
        cx.notify();
    }

    pub(super) fn render_composer(&self, id: TabId, cx: &mut Context<Self>) -> Div {
        let composer = &self.composers[&id];
        let pending = composer.pending.is_some();
        let checked = composer.include_enter;
        let palette = self.palette;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child(t("텍스트 작성")))
            .child(composer.input.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(self.palette.muted))
                    .child(t("Enter는 줄바꿈 · 전송 버튼으로 보내기")),
            )
            .child(
                self.button_base("composer-enter", cx, move |s, _, _| {
                    if let Some(composer) = s.composers.get_mut(&id)
                        && composer.pending.is_none()
                    {
                        composer.include_enter = !composer.include_enter;
                    }
                })
                .flex()
                .items_center()
                .gap_2()
                .h(px(36.))
                .child(
                    div()
                        .debug_selector(|| "composer-enter-box".into())
                        .size(px(18.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(if checked {
                            palette.accent
                        } else {
                            palette.muted
                        }))
                        .bg(rgb(if checked {
                            palette.accent_fill
                        } else {
                            palette.input
                        }))
                        .when(checked, |d| {
                            d.child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        let mut check = PathBuilder::stroke(px(2.));
                                        check.move_to(bounds.origin + point(px(1.), px(6.)));
                                        check.line_to(bounds.origin + point(px(4.5), px(9.5)));
                                        check.line_to(bounds.origin + point(px(11.), px(2.5)));
                                        if let Ok(path) = check.build() {
                                            window.paint_path(path, rgb(palette.on_accent));
                                        }
                                    },
                                )
                                .size(px(12.)),
                            )
                        }),
                )
                .child(
                    div()
                        .debug_selector(|| "composer-enter-label".into())
                        .child(t("Enter 함께 보내기")),
                )
                .when(pending, |d| d.opacity(0.35).cursor_default()),
            )
            .when_some(composer.error.clone(), |d, error| {
                d.child(div().text_color(rgb(0xd63751)).child(error))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{Composer, Destination, TextInput};
    use dgl_core::{
        Error,
        connection::{Completion, CompletionOutcome},
        flow::{Generation, PaneTarget},
    };
    use gpui::{App, AppContext};

    fn destination() -> Destination {
        Destination {
            session: Some("$1".into()),
            target: PaneTarget {
                connection: 1,
                pane_id: "%2".into(),
                generation: Generation {
                    connection: 1,
                    pane: 3,
                    size: 4,
                },
            },
        }
    }
    fn draft(cx: &mut App) -> Composer {
        Composer {
            input: cx.new(|cx| {
                let mut i = TextInput::multiline(cx);
                i.set_content("한글\n초안 😀", cx);
                i
            }),
            include_enter: true,
            target: Some(destination()),
            pending: None,
            error: None,
        }
    }
    fn result(request_id: u64, error: Option<Error>) -> Completion {
        Completion {
            request_id,
            outcome: if error.is_some() {
                CompletionOutcome::Failed
            } else {
                CompletionOutcome::Sent
            },
            error,
        }
    }

    #[gpui::test]
    fn rejects_empty_oversized_changed_and_missing_destinations_without_submitting(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            let mut composer = draft(cx);
            let original = destination();
            let mut mutations = vec![None];
            for field in 0..6 {
                let mut changed = original.clone();
                match field {
                    0 => changed.session = Some("$2".into()),
                    1 => changed.target.connection += 1,
                    2 => changed.target.generation.connection += 1,
                    3 => changed.target.generation.pane += 1,
                    4 => changed.target.pane_id = "%9".into(),
                    _ => changed.session = None,
                }
                mutations.push(Some(changed));
            }
            for current in mutations {
                composer.submit("hello", current, true, |_, _, _| {
                    panic!("stale draft entered input queue")
                });
                assert!(composer.error.is_some());
                assert!(composer.pending.is_none());
            }
            composer.submit("hello", Some(original.clone()), false, |_, _, _| {
                panic!("inactive tab submitted")
            });
            for text in [String::new(), "한".repeat(21846)] {
                composer.submit(&text, Some(original.clone()), true, |_, _, _| {
                    panic!("invalid size submitted")
                });
                assert!(composer.error.is_some());
            }
            assert_eq!(composer.input.read(cx).content.as_ref(), "한글\n초안 😀");
        });
    }

    #[gpui::test]
    fn resize_allows_one_submission_and_only_matching_success_clears_the_draft(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            let mut composer = draft(cx);
            let mut resized = destination();
            resized.target.generation.size += 9;
            let text = composer.input.read(cx).content.to_string();
            let mut calls = 0;
            composer.submit(&text, Some(resized.clone()), true, |_, text, enter| {
                calls += 1;
                assert_eq!(text, "한글\n초안 😀");
                assert!(enter);
                Ok(17)
            });
            composer.submit(&text, Some(resized), true, |_, _, _| {
                panic!("duplicate submitted")
            });
            assert_eq!(calls, 1);
            assert_eq!(composer.pending, Some(17));
            assert!(!composer.complete(&result(16, None), cx));
            assert_eq!(composer.input.read(cx).content.as_ref(), text);
            assert!(composer.complete(&result(17, Some(Error::Network)), cx));
            assert!(composer.error.is_some());
            assert_eq!(composer.input.read(cx).content.as_ref(), text);
            assert!(composer.include_enter);
            assert!(composer.pending.is_none());
            // A manual retry can fail synchronously; neither failure retries itself.
            composer.submit(&text, Some(destination()), true, |_, _, _| {
                Err("queue full".into())
            });
            assert_eq!(composer.error.as_deref(), Some("queue full"));
            assert!(composer.pending.is_none());
            composer.submit(&text, Some(destination()), true, |_, _, _| Ok(18));
            assert!(composer.complete(&result(18, None), cx));
            assert!(composer.input.read(cx).content.is_empty());
            assert!(!composer.include_enter);
            assert!(composer.error.is_none());
        });
    }
}
