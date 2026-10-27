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
    pub pending: Option<Pending>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    Text { request: u64, enter: bool },
    Enter { request: u64 },
}
impl Pending {
    fn request(self) -> u64 {
        match self {
            Self::Text { request, .. } | Self::Enter { request } => request,
        }
    }
}

pub(super) fn complete(
    composers: &mut HashMap<TabId, Composer>,
    dialog: &mut Option<Dialog>,
    tab: &Tab,
    selected: bool,
    result: &Completion,
    cx: &mut App,
) -> bool {
    let id = tab.id;
    let Some(composer) = composers.get_mut(&id) else {
        return false;
    };
    if !composer.complete(
        result,
        Destination::capture(&tab.connection),
        selected,
        |target, bytes| {
            tab.connection
                .input(target.clone(), bytes)
                .map_err(|e| e.to_string())
        },
        cx,
    ) {
        return false;
    }
    if composer.pending.is_none() && composer.error.is_none() {
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
        send: impl FnOnce(&PaneTarget, &str) -> Result<u64, String>,
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
            send(&destination.target, text)
        })();
        match result {
            Ok(request) => {
                self.pending = Some(Pending::Text {
                    request,
                    enter: self.include_enter,
                });
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn complete(
        &mut self,
        result: &Completion,
        current: Option<Destination>,
        selected: bool,
        send_enter: impl FnOnce(&PaneTarget, Vec<u8>) -> Result<u64, String>,
        cx: &mut App,
    ) -> bool {
        let Some(pending) = self.pending.filter(|p| p.request() == result.request_id) else {
            return false;
        };
        let sent = result.outcome == CompletionOutcome::Sent && result.error.is_none();
        let failure = || {
            result
                .error
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| t("전송 결과를 확인하지 못했습니다").into())
        };
        self.pending = None;
        let error = if sent && matches!(pending, Pending::Text { enter: true, .. }) {
            // The paste has completed. Enter is a separate key request to the same destination.
            let next = current
                .as_ref()
                .filter(|current| {
                    selected
                        && self
                            .target
                            .as_ref()
                            .is_some_and(|original| original.matches(current))
                })
                .ok_or_else(|| t("연결·세션·패널이 변경되어 입력을 취소했습니다.").to_string())
                .and_then(|destination| {
                    let bytes = dgl_core::input::TerminalKey::Enter
                        .encode(dgl_core::input::Modifiers::default(), false)
                        .expect("Enter must be encodable");
                    send_enter(&destination.target, bytes)
                });
            match next {
                Ok(request) => {
                    self.pending = Some(Pending::Enter { request });
                    return true;
                }
                Err(error) => Some(Self::enter_error(error)),
            }
        } else if sent {
            None
        } else if matches!(pending, Pending::Enter { .. }) {
            Some(Self::enter_error(failure()))
        } else {
            Some(tr!(
                "전송 실패: {0}. 초안은 유지되며 자동 재전송하지 않습니다.",
                failure()
            ))
        };
        self.input.update(cx, |input, cx| {
            input.read_only = false;
            cx.notify();
        });
        if error.is_none() {
            self.input.update(cx, |input, cx| input.set_content("", cx));
            self.include_enter = false;
            self.target = None;
        }
        self.error = error;
        true
    }

    fn enter_error(error: String) -> String {
        tr!(
            "텍스트는 전송되었지만 Enter 전송에 실패했습니다: {0}. 초안은 유지되며 자동 재전송하지 않습니다.",
            error
        )
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
        if matches!(self.dialog, Some(Dialog::Shortcuts | Dialog::About)) {
            self.settings_back();
            cx.notify();
            return;
        }
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
            |target, text| {
                let bytes = tab
                    .terminal
                    .update(cx, |t, cx| t.prepare_paste(target, text, cx))?;
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
            .child(
                div()
                    .id("composer-title")
                    .text_xl()
                    .child(t("텍스트 작성"))
                    .native_tooltip(t("Enter는 줄바꿈 · 전송 버튼으로 보내기")),
            )
            .child(composer.input.clone())
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
                            d.child(UiIcon::Check.render(palette.on_accent))
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
    use super::{Composer, Destination, Pending, TextInput};
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
    fn complete_without_enter(composer: &mut Composer, result: &Completion, cx: &mut App) -> bool {
        composer.complete(
            result,
            Some(destination()),
            true,
            |_, _| panic!("unexpected Enter"),
            cx,
        )
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
                composer.submit("hello", current, true, |_, _| {
                    panic!("stale draft entered input queue")
                });
                assert!(composer.error.is_some());
                assert!(composer.pending.is_none());
            }
            composer.submit("hello", Some(original.clone()), false, |_, _| {
                panic!("inactive tab submitted")
            });
            for text in [String::new(), "한".repeat(21846)] {
                composer.submit(&text, Some(original.clone()), true, |_, _| {
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
            composer.submit(&text, Some(resized.clone()), true, |_, text| {
                calls += 1;
                assert_eq!(text, "한글\n초안 😀");
                Ok(17)
            });
            composer.submit(&text, Some(resized), true, |_, _| {
                panic!("duplicate submitted")
            });
            assert_eq!(calls, 1);
            assert_eq!(
                composer.pending,
                Some(Pending::Text {
                    request: 17,
                    enter: true
                })
            );
            assert!(!complete_without_enter(
                &mut composer,
                &result(16, None),
                cx
            ));
            assert_eq!(composer.input.read(cx).content.as_ref(), text);
            assert!(complete_without_enter(
                &mut composer,
                &result(17, Some(Error::Network)),
                cx
            ));
            assert!(composer.error.is_some());
            assert_eq!(composer.input.read(cx).content.as_ref(), text);
            assert!(composer.include_enter);
            assert!(composer.pending.is_none());
            // A manual retry can fail synchronously; neither failure retries itself.
            composer.submit(&text, Some(destination()), true, |_, _| {
                Err("queue full".into())
            });
            assert_eq!(composer.error.as_deref(), Some("queue full"));
            assert!(composer.pending.is_none());
            composer.include_enter = false;
            composer.submit(&text, Some(destination()), true, |_, _| Ok(18));
            assert!(complete_without_enter(&mut composer, &result(18, None), cx));
            assert!(composer.input.read(cx).content.is_empty());
            assert!(!composer.include_enter);
            assert!(composer.error.is_none());
        });
    }

    #[gpui::test]
    fn enter_waits_for_matching_text_success_and_its_own_completion(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let mut composer = draft(cx);
            let text = composer.input.read(cx).content.to_string();
            composer.submit(&text, Some(destination()), true, |_, sent| {
                assert_eq!(sent, text);
                Ok(17)
            });
            composer.input.update(cx, |i, _| i.read_only = true);
            assert!(!complete_without_enter(
                &mut composer,
                &result(16, None),
                cx
            ));
            let mut resized = destination();
            resized.target.generation.size += 1;
            let mut enters = 0;
            assert!(composer.complete(
                &result(17, None),
                Some(resized.clone()),
                true,
                |target, bytes| {
                    enters += 1;
                    assert_eq!(target, &resized.target);
                    assert_eq!(bytes, b"\r");
                    Ok(18)
                },
                cx
            ));
            assert_eq!(enters, 1);
            assert_eq!(composer.pending, Some(Pending::Enter { request: 18 }));
            assert_eq!(composer.input.read(cx).content.as_ref(), text);
            assert!(composer.input.read(cx).read_only);
            assert!(composer.include_enter);
            assert!(!complete_without_enter(
                &mut composer,
                &result(17, None),
                cx
            ));
            composer.submit(&text, Some(destination()), true, |_, _| {
                panic!("duplicate text")
            });
            assert!(complete_without_enter(&mut composer, &result(18, None), cx));
            assert!(composer.pending.is_none());
            assert!(composer.input.read(cx).content.is_empty());
            assert!(!composer.input.read(cx).read_only);
            assert!(!composer.include_enter);
        });
    }

    #[gpui::test]
    fn enter_failure_preserves_delivered_text_without_replaying(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            for failure in 0..3 {
                let mut composer = draft(cx);
                let text = composer.input.read(cx).content.to_string();
                composer.submit(&text, Some(destination()), true, |_, _| Ok(17));
                composer.input.update(cx, |i, _| i.read_only = true);
                assert!(composer.complete(
                    &result(17, None),
                    Some(destination()),
                    true,
                    |_, _| if failure == 0 {
                        Err("queue full".into())
                    } else {
                        Ok(18)
                    },
                    cx
                ));
                if failure != 0 {
                    let completion = if failure == 1 {
                        result(18, Some(Error::Network))
                    } else {
                        Completion {
                            request_id: 18,
                            outcome: CompletionOutcome::Cancelled,
                            error: None,
                        }
                    };
                    assert!(complete_without_enter(&mut composer, &completion, cx));
                }
                assert!(composer.pending.is_none());
                assert_eq!(composer.input.read(cx).content.as_ref(), text);
                assert!(!composer.input.read(cx).read_only);
                assert!(composer.include_enter);
                assert!(composer.error.as_ref().unwrap().contains("Enter"));
                assert!(!complete_without_enter(
                    &mut composer,
                    &result(17, None),
                    cx
                ));
                assert!(!complete_without_enter(
                    &mut composer,
                    &result(18, None),
                    cx
                ));
            }
        });
    }

    #[gpui::test]
    fn changed_destinations_after_text_and_unsent_completions_never_send_enter(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            for change in 0..8 {
                let mut composer = draft(cx);
                let text = composer.input.read(cx).content.to_string();
                composer.submit(&text, Some(destination()), true, |_, _| Ok(17));
                let mut changed = destination();
                match change {
                    0 => changed.session = Some("$2".into()),
                    1 => changed.target.connection += 1,
                    2 => changed.target.generation.connection += 1,
                    3 => changed.target.generation.pane += 1,
                    4 => changed.target.pane_id = "%9".into(),
                    _ => {}
                }
                let mut completion = result(17, None);
                if change == 7 {
                    completion.outcome = CompletionOutcome::Cancelled;
                }
                assert!(composer.complete(
                    &completion,
                    (change != 5).then_some(changed),
                    change != 6,
                    |_, _| panic!("Enter sent to changed destination or after cancellation"),
                    cx
                ));
                assert!(composer.pending.is_none());
                assert_eq!(composer.input.read(cx).content.as_ref(), text);
                assert!(composer.error.is_some());
            }
        });
    }
}
