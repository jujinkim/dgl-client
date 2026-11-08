use super::*;
use dgl_core::input::{TerminalShortcut, move_shortcut};
use terminal::fonts::TerminalFace;

#[derive(Default)]
pub(super) struct SettingsState {
    main_offset: Point<Pixels>,
    pub restore: bool,
    pub entry: &'static str,
    pub dragging: Option<TerminalShortcut>,
    destination: Option<usize>,
    pointer: Option<Point<Pixels>>,
    rows: HashMap<usize, Bounds<Pixels>>,
    /// Installed monospace families, read when the font page first opens.
    fonts: Option<Vec<String>>,
    loading_fonts: bool,
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
                    .id(SharedString::from(format!("settings-title-{title}")))
                    .text_lg()
                    .text_color(rgb(self.palette.accent))
                    .native_tooltip(t(description))
                    .child(t(title)),
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
                (
                    if choice == value {
                        UiIcon::RadioSelected
                    } else {
                        UiIcon::Radio
                    },
                    label,
                ),
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
                (
                    if selected {
                        UiIcon::RadioSelected
                    } else {
                        UiIcon::Radio
                    },
                    label,
                ),
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
        let face = cx.try_global::<TerminalFace>().cloned().unwrap_or_default();
        let stored = &self.local.preferences.terminal_font_family;
        let family = if !face.selected.is_empty() {
            face.family.to_string()
        } else if stored.is_empty() {
            tr!("기본 ({0})", platform::TERMINAL_FONT)
        } else {
            tr!("{0} 없음 · 기본 글꼴 사용", stored)
        };
        let terminal = self
            .settings_card("터미널·입력", "터미널 글자와 입력 도구를 조정합니다.")
            .child(font)
            .child(self.button(
                "terminal-font",
                (UiIcon::ChevronRight, tr!("터미널 글꼴: {0}", family)),
                cx,
                |s, _, cx| s.open_terminal_fonts(cx),
            ))
            .child(
                div()
                    .id("scrollback-setting")
                    .text_xs()
                    .native_tooltip(tr!(
                        "스크롤백: {0}줄 · 터미널에서 스크롤·선택·복사",
                        self.local.preferences.scrollback
                    ))
                    .child(tr!("스크롤백: {0}줄", self.local.preferences.scrollback)),
            )
            .child(self.button(
                "edit-shortcuts",
                (
                    UiIcon::ChevronRight,
                    tr!(
                        "하단 단축키 · {0}개 · 순서 및 표시 편집",
                        self.local.preferences.terminal_shortcuts.len()
                    ),
                ),
                cx,
                |s, _, _| {
                    s.settings.main_offset = s.modal.scroll.offset();
                    s.settings.entry = "edit-shortcuts";
                    s.dialog = Some(Dialog::Shortcuts);
                },
            ));
        let power_mode =
            crate::power::Mode::from_preference(&self.local.preferences.desktop_power_saving);
        let mut power = div().flex().flex_wrap().gap_2();
        for (mode, label) in [
            (crate::power::Mode::System, t("OS 저전력 모드 따름")),
            (crate::power::Mode::Always, t("항상 절전")),
            (crate::power::Mode::Off, t("사용 안 함")),
        ] {
            power = power.child(self.button(
                format!("power-saving-{}", mode.preference()),
                (
                    if power_mode == mode {
                        UiIcon::RadioSelected
                    } else {
                        UiIcon::Radio
                    },
                    label,
                ),
                cx,
                move |s, _, _| {
                    s.local.preferences.desktop_power_saving = mode.preference().into();
                    s.save();
                },
            ));
        }
        let workspace = self
            .settings_card("작업 공간", "계층 표시와 화면 전환을 선택합니다.")
            .child(self.button(
                "show-all-hierarchy",
                (
                    if self.local.preferences.show_all_hierarchy {
                        UiIcon::CheckboxChecked
                    } else {
                        UiIcon::Checkbox
                    },
                    t("단일 창도 표시"),
                ),
                cx,
                |s, _, _| {
                    s.local.preferences.show_all_hierarchy =
                        !s.local.preferences.show_all_hierarchy;
                    s.save();
                },
            ))
            .child(
                div()
                    .id("power-saving-title")
                    .native_tooltip(t(
                        "절전 중에는 한가할 때 화면 확인 간격을 늘립니다. 입력·출력 중 반응 속도는 그대로입니다. 이 기기에만 저장합니다.",
                    ))
                    .child(t("절전 주기")),
            )
            .child(power);
        let alerts = self
            .settings_card("알림", "에이전트 알림과 OS 알림을 선택합니다.")
            .child(self.agent_settings(cx));
        let sync = self
            .settings_card("동기화", "기기 간 공통 설정과 호스트를 공유합니다.")
            .child(self.cloud_sync_settings(cx));
        let mut security = self
            .settings_card("보안·저장", platform::SHORTCUT_HELP)
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
        security = security.child(div().text_xs().child(platform::STORAGE_LABEL));
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
            .child(self.button("about-app", t("About app"), cx, |s, _, _| {
                s.settings.main_offset = s.modal.scroll.offset();
                s.settings.entry = "about-app";
                s.dialog = Some(Dialog::About);
            }))
    }
    pub(super) fn settings_back(&mut self) {
        self.settings.entry = match self.dialog {
            Some(Dialog::About) => "about-app",
            Some(Dialog::TerminalFont) => "terminal-font",
            _ => "edit-shortcuts",
        };
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
            .child(
                div()
                    .id("shortcut-settings-title")
                    .text_xl()
                    .child(t("하단 단축키"))
                    .native_tooltip(format!(
                        "{}\n{}",
                        t("핸들을 끌어 순서를 바꾸세요. 변경은 놓을 때 저장됩니다."),
                        t("Ctrl·Shift·Alt는 다음 키 입력에 한 번 적용됩니다.")
                    )),
            )
            .child(self.button("shortcuts-back", t("뒤로"), cx, |s, _, _| {
                s.settings_back()
            }))
            .child(div().text_lg().child(t("사용 중인 단축키")));
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
                            UiIcon::Grip,
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
                                cx.new(|_| workspace_ui::DragPreview(drag.0.label().into()))
                            },
                        ),
                    )
                    .child(div().flex_1().child(key.label()))
                    .child(
                        self.button(
                            format!("shortcut-up-{}", key.label()),
                            UiIcon::ArrowUp,
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
                            UiIcon::ArrowDown,
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
impl Desktop {
    /// Opens the family list; the system font collection is read off the UI thread.
    pub(super) fn open_terminal_fonts(&mut self, cx: &mut Context<Self>) {
        self.settings.main_offset = self.modal.scroll.offset();
        self.settings.entry = "terminal-font";
        self.dialog = Some(Dialog::TerminalFont);
        if self.settings.fonts.is_some() || self.settings.loading_fonts {
            return;
        }
        self.settings.loading_fonts = true;
        let fonts = cx
            .background_executor()
            .spawn(async { terminal::fonts::installed_monospace() });
        cx.spawn(async move |view, cx| {
            let fonts = fonts.await;
            let _ = view.update(cx, |s, cx| {
                s.settings.loading_fonts = false;
                s.settings.fonts = Some(fonts);
                cx.notify();
            });
        })
        .detach();
    }
    /// Empty selects the platform default. A family that no longer resolves to an
    /// installed monospace face is not stored.
    pub(super) fn set_terminal_font(&mut self, family: &str, cx: &mut Context<Self>) {
        let face = TerminalFace::resolve(family, cx.text_system());
        if !family.is_empty() && face.selected.is_empty() {
            self.snackbar.show_snackbar_with_duration(
                tr!("{0} 글꼴을 불러오지 못해 기본 글꼴을 사용합니다.", family),
                SnackbarDuration::Short,
            );
        }
        self.local.preferences.terminal_font_family = face.selected.to_string();
        cx.set_global(face);
        for tab in &self.tabs {
            tab.terminal.update(cx, |_, cx| cx.notify());
        }
        self.save();
    }
    pub(super) fn terminal_font_settings(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let face = cx.try_global::<TerminalFace>().cloned().unwrap_or_default();
        let mut page = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child(t("터미널 글꼴")))
            .child(self.button("terminal-font-back", t("뒤로"), cx, |s, _, _| {
                s.settings_back()
            }))
            .child(div().text_xs().child(t(
                "이 기기에 설치된 고정폭 글꼴만 표시합니다. 동기화하지 않습니다.",
            )))
            .child(
                div()
                    .id("terminal-font-preview")
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(p.line))
                    .bg(rgb(terminal::DEFAULT_BACKGROUND))
                    .text_color(rgb(0xdce1e8))
                    .font_family(face.family.clone())
                    .text_size(px(self.local.preferences.font_size as f32))
                    .child("$ ls -la ~/src  0O 1lI {}[]() 한글"),
            );
        let Some(fonts) = &self.settings.fonts else {
            return page.child(t("글꼴 목록을 불러오는 중…"));
        };
        let option =
            |id: String, label: String, selected: bool, family: String, cx: &mut Context<Self>| {
                self.button(
                    id,
                    (
                        if selected {
                            UiIcon::RadioSelected
                        } else {
                            UiIcon::Radio
                        },
                        label,
                    ),
                    cx,
                    move |s, _, cx| s.set_terminal_font(&family, cx),
                )
            };
        page = page.child(option(
            "terminal-font-default".into(),
            tr!("기본 ({0})", platform::TERMINAL_FONT),
            face.selected.is_empty(),
            String::new(),
            cx,
        ));
        // The default family is already the first choice.
        let installed: Vec<_> = fonts
            .iter()
            .filter(|f| f.as_str() != platform::TERMINAL_FONT)
            .collect();
        if installed.is_empty() {
            page = page.child(
                div()
                    .text_xs()
                    .child(t("다른 고정폭 글꼴을 찾지 못했습니다.")),
            );
        }
        for family in installed {
            page = page.child(option(
                format!("terminal-font-{family}"),
                family.clone(),
                face.selected.as_ref() == family.as_str(),
                family.clone(),
                cx,
            ));
        }
        page
    }
}
