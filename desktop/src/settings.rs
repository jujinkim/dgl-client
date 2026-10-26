use super::*;
use dgl_core::input::{TerminalShortcut, move_shortcut};

#[derive(Default)]
pub(super) struct SettingsState {
    main_offset: Point<Pixels>,
    pub restore: bool,
    pub dragging: Option<TerminalShortcut>,
    destination: Option<usize>,
    pointer: Option<Point<Pixels>>,
    rows: HashMap<usize, Bounds<Pixels>>,
}
impl SettingsState {
    pub fn main_offset(&self) -> Point<Pixels> {
        self.main_offset
    }
}
#[derive(Clone)]
struct DragShortcut(TerminalShortcut);
impl Desktop {
    fn settings_card(&self, title: &'static str, description: &'static str) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgb(self.palette.line))
            .bg(rgb(self.palette.panel))
            .child(
                div()
                    .text_lg()
                    .text_color(rgb(self.palette.accent))
                    .child(t(title)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(self.palette.muted))
                    .child(t(description)),
            )
    }
    pub(super) fn settings_main(&mut self, cx: &mut Context<Self>) -> Div {
        let choice = match self.local.preferences.language.as_str() {
            "ko" => "ko",
            "en" => "en",
            _ => "system",
        };
        let mut languages = div().flex().flex_wrap().gap_2();
        for (value, label) in [("system", t("시스템")), ("ko", "한국어"), ("en", "English")] {
            languages = languages.child(self.button(
                format!("language-{value}"),
                format!("{} {label}", if choice == value { "●" } else { "○" }),
                cx,
                move |s, _, cx| s.language(value, cx),
            ));
        }
        let mut themes = div().flex().flex_wrap().gap_2();
        for (value, label) in [
            ("system", t("시스템")),
            ("light", t("밝게")),
            ("dark", t("어둡게")),
        ] {
            let selected = self.local.preferences.theme == value;
            themes = themes.child(self.button(
                format!("theme-{value}"),
                format!("{} {label}", if selected { "●" } else { "○" }),
                cx,
                move |s, _, cx| s.theme(value, cx),
            ));
        }
        let appearance = self
            .settings_card("화면·언어", "앱의 언어와 모양을 선택합니다.")
            .child(t("언어"))
            .child(languages)
            .child(t("테마"))
            .child(themes)
            .child(self.accent_settings(cx));
        let mut font = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(tr!(
                "터미널 글자 크기: {0}",
                self.local.preferences.font_size
            )));
        for (delta, label) in [(-1, t("작게")), (1, t("크게"))] {
            font = font.child(self.button(
                format!("font-size-{delta}"),
                label,
                cx,
                move |s, _, cx| {
                    s.local.preferences.font_size =
                        (s.local.preferences.font_size + delta).clamp(10, 28);
                    for tab in &s.tabs {
                        tab.terminal
                            .update(cx, |t, _| t.set_font(s.local.preferences.font_size));
                    }
                    s.save();
                },
            ));
        }
        let terminal = self
            .settings_card("터미널·입력", "터미널 글자와 입력 도구를 조정합니다.")
            .child(font)
            .child(div().text_xs().child(tr!(
                "스크롤백: {0}줄 · 터미널에서 스크롤·선택·복사",
                self.local.preferences.scrollback
            )))
            .child(self.button(
                "edit-shortcuts",
                tr!(
                    "하단 단축키 · {0}개 · 순서 및 표시 편집 ›",
                    self.local.preferences.terminal_shortcuts.len()
                ),
                cx,
                |s, _, _| {
                    s.settings.main_offset = s.modal.scroll.offset();
                    s.dialog = Some(Dialog::Shortcuts);
                },
            ));
        let workspace = self
            .settings_card("작업 공간", "계층 표시와 화면 전환을 선택합니다.")
            .child(self.button(
                "show-all-hierarchy",
                tr!(
                    "{0} 단일 창도 표시",
                    if self.local.preferences.show_all_hierarchy {
                        "☑"
                    } else {
                        "☐"
                    }
                ),
                cx,
                |s, _, _| {
                    s.local.preferences.show_all_hierarchy =
                        !s.local.preferences.show_all_hierarchy;
                    s.save();
                },
            ));
        let alerts = self
            .settings_card("알림", "에이전트 알림과 OS 알림을 선택합니다.")
            .child(self.agent_settings(cx));
        let sync = self
            .settings_card("동기화", "기기 간 공통 설정과 호스트를 공유합니다.")
            .child(self.cloud_sync_settings(cx));
        let mut security = self
            .settings_card(
                "보안·저장",
                "신뢰한 서버와 이 기기의 저장 위치를 확인합니다.",
            )
            .child(t("신뢰한 서버"));
        for (endpoint, fingerprint) in &self.local.trust {
            let e = endpoint.clone();
            security = security
                .child(format!("{endpoint}\n{fingerprint}"))
                .child(self.button(
                    format!("forget-{endpoint}"),
                    t("신뢰 삭제…"),
                    cx,
                    move |s, _, _| {
                        s.dialog = Some(Dialog::Delete {
                            title: tr!("{0} 신뢰 삭제", e),
                            operation: DeleteOp::Trust(e.clone()),
                        });
                    },
                ));
        }
        security = security
            .child(div().text_xs().child(platform::STORAGE_LABEL))
            .child(div().text_xs().child(t(platform::SHORTCUT_HELP)));
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child(t("설정")))
            .child(appearance)
            .child(terminal)
            .child(workspace)
            .child(alerts)
            .child(sync)
            .child(security)
    }
    pub(super) fn shortcuts_back(&mut self) {
        self.settings.dragging = None;
        self.settings.destination = None;
        self.settings.restore = true;
        self.dialog = Some(Dialog::Settings);
    }
    pub(super) fn tick_shortcut_drag(&mut self, cx: &mut Context<Self>) {
        if self.settings.dragging.is_none() {
            return;
        }
        if !cx.has_active_drag() || !matches!(self.dialog, Some(Dialog::Shortcuts)) {
            self.settings.dragging = None;
            self.settings.destination = None;
            self.settings.pointer = None;
            cx.notify();
            return;
        }
        let Some(pointer) = self.settings.pointer else {
            return;
        };
        let bounds = self.modal.scroll.bounds();
        let delta = if pointer.y < bounds.top() + px(48.) {
            10.
        } else if pointer.y > bounds.bottom() - px(48.) {
            -10.
        } else {
            0.
        };
        if delta != 0. {
            let mut offset = self.modal.scroll.offset();
            offset.y = px((f32::from(offset.y) + delta)
                .clamp(-f32::from(self.modal.scroll.max_offset().height), 0.));
            self.modal.scroll.set_offset(offset);
        }
        let count = self.local.preferences.terminal_shortcuts.len();
        let slot = (0..count)
            .find(|i| {
                self.settings
                    .rows
                    .get(i)
                    .is_some_and(|b| pointer.y < b.center().y)
            })
            .unwrap_or(count);
        let from = self
            .local
            .preferences
            .terminal_shortcuts
            .iter()
            .position(|k| Some(*k) == self.settings.dragging)
            .unwrap_or(0);
        self.settings.destination = count
            .checked_sub(1)
            .map(|last| slot.saturating_sub(usize::from(from < slot)).min(last));
        cx.notify();
    }
    pub(super) fn shortcut_settings(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let mut active = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child(t("하단 단축키")))
            .child(self.button("shortcuts-back", t("뒤로"), cx, |s, _, _| {
                s.shortcuts_back()
            }))
            .child(div().text_lg().child(t("사용 중인 단축키")))
            .child(
                div()
                    .text_xs()
                    .child(t("핸들을 끌어 순서를 바꾸세요. 변경은 놓을 때 저장됩니다.")),
            )
            .child(
                div()
                    .text_xs()
                    .child(t("Ctrl·Shift·Alt는 다음 키 입력에 한 번 적용됩니다.")),
            );
        let keys = &self.local.preferences.terminal_shortcuts;
        if keys.is_empty() {
            active = active.child(t("사용 중인 단축키가 없습니다."));
        }
        for (index, &key) in keys.iter().enumerate() {
            let view = cx.entity();
            let paint_view = view.clone();
            let insertion =
                self.settings.dragging.is_some() && self.settings.destination == Some(index);
            let dragging = self.settings.dragging == Some(key);
            let count = keys.len();
            active = active.child(
                div()
                    .id(SharedString::from(format!("shortcut-row-{index}")))
                    .debug_selector(move || format!("shortcut-row-{index}"))
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(p.line))
                    .bg(rgb(if dragging { p.selection } else { p.panel }))
                    .on_drag_move::<DragShortcut>(cx.listener(
                        move |s, e: &DragMoveEvent<DragShortcut>, _, cx| {
                            s.settings.pointer = Some(e.event.position);
                            if e.bounds.contains(&e.event.position) {
                                let from = s
                                    .local
                                    .preferences
                                    .terminal_shortcuts
                                    .iter()
                                    .position(|k| Some(*k) == s.settings.dragging)
                                    .unwrap_or(index);
                                let slot =
                                    index + usize::from(e.event.position.y >= e.bounds.center().y);
                                s.settings.destination = Some(
                                    slot.saturating_sub(usize::from(from < slot)).min(count - 1),
                                );
                            }
                            cx.notify();
                        },
                    ))
                    .on_drop(cx.listener(move |s, dragged: &DragShortcut, _, cx| {
                        if s.settings.dragging == Some(dragged.0)
                            && let Some(from) = s
                                .local
                                .preferences
                                .terminal_shortcuts
                                .iter()
                                .position(|k| *k == dragged.0)
                            && let Some(to) = s.settings.destination
                        {
                            s.local.preferences.terminal_shortcuts =
                                move_shortcut(&s.local.preferences.terminal_shortcuts, from, to);
                            s.save();
                        }
                        s.settings.dragging = None;
                        s.settings.destination = None;
                        s.settings.pointer = None;
                        cx.notify();
                        cx.stop_propagation();
                    }))
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                paint_view.update(cx, |s, _| {
                                    s.settings.rows.insert(index, bounds);
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        self.button(
                            format!("shortcut-drag-{}", key.label()),
                            "≡",
                            cx,
                            |_, _, _| {},
                        )
                        .cursor_move()
                        .on_drag(
                            DragShortcut(key),
                            move |drag, _, _, cx| {
                                view.update(cx, |s, cx| {
                                    s.settings.dragging = Some(drag.0);
                                    s.settings.pointer = None;
                                    s.settings.destination = Some(index);
                                    cx.notify();
                                });
                                cx.new(|_| workspace_ui::Tip(drag.0.label().into()))
                            },
                        ),
                    )
                    .child(div().flex_1().child(key.label()))
                    .child(
                        self.button(
                            format!("shortcut-up-{}", key.label()),
                            "↑",
                            cx,
                            move |s, _, _| {
                                if index > 0 && s.settings.dragging.is_none() {
                                    s.local.preferences.terminal_shortcuts = move_shortcut(
                                        &s.local.preferences.terminal_shortcuts,
                                        index,
                                        index - 1,
                                    );
                                    s.save();
                                }
                            },
                        )
                        .when(index == 0, |d| d.opacity(0.35)),
                    )
                    .child(
                        self.button(
                            format!("shortcut-down-{}", key.label()),
                            "↓",
                            cx,
                            move |s, _, _| {
                                if index + 1 < count && s.settings.dragging.is_none() {
                                    s.local.preferences.terminal_shortcuts = move_shortcut(
                                        &s.local.preferences.terminal_shortcuts,
                                        index,
                                        index + 1,
                                    );
                                    s.save();
                                }
                            },
                        )
                        .when(index + 1 == count, |d| d.opacity(0.35)),
                    )
                    .child(self.button(
                        format!("shortcut-setting-{}", key.label()),
                        t("제외"),
                        cx,
                        move |s, _, _| {
                            if s.settings.dragging.is_some() {
                                return;
                            }
                            s.local.preferences.terminal_shortcuts.retain(|k| *k != key);
                            s.save();
                        },
                    ))
                    .when(insertion, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .h(px(3.))
                                .bg(rgb(p.accent)),
                        )
                    }),
            );
        }
        active = active.child(div().mt_4().text_lg().child(t("제외된 단축키")));
        for key in TerminalShortcut::all()
            .into_iter()
            .filter(|k| !keys.contains(k))
        {
            active = active.child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().child(key.label()))
                    .child(self.button(
                        format!("shortcut-setting-{}", key.label()),
                        t("추가"),
                        cx,
                        move |s, _, _| {
                            if s.settings.dragging.is_some() {
                                return;
                            }
                            s.local.preferences.terminal_shortcuts.push(key);
                            s.save();
                        },
                    )),
            );
        }
        active.child(self.button(
            "reset-shortcuts",
            t("단축키 기본값 복원"),
            cx,
            |s, _, _| {
                if s.settings.dragging.is_some() {
                    return;
                }
                s.local.preferences.terminal_shortcuts = TerminalShortcut::defaults();
                s.save();
            },
        ))
    }
}
