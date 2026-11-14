use super::*;
use dgl_core::i18n::Language;
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
        let choice =
            Language::from_code(&self.local.preferences.language).map_or("system", Language::code);
        let mut languages = div().flex().flex_wrap().gap_2();
        let options = std::iter::once(("system", t("common.system")))
            .chain(Language::all().map(|language| (language.code(), language.native_name())));
        for (value, label) in options {
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
            ("system", t("common.system")),
            ("light", t("settings.theme_light")),
            ("dark", t("settings.theme_dark")),
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
            .settings_card(
                "settings.appearance_language",
                "settings.appearance_description",
            )
            .child(t("settings.language"))
            .child(languages)
            .child(t("settings.theme"))
            .child(themes)
            .child(self.accent_settings(cx));
        let assist_mode = dgl_core::scroll::ScrollAssist::from_preference(
            &self.local.preferences.terminal_scroll_assist,
        );
        let mut scroll_assist = div().flex().flex_wrap().gap_2();
        for (mode, label) in [
            (
                dgl_core::scroll::ScrollAssist::Auto,
                t("settings.scroll_assist_auto"),
            ),
            (
                dgl_core::scroll::ScrollAssist::On,
                t("settings.scroll_assist_on"),
            ),
            (
                dgl_core::scroll::ScrollAssist::Off,
                t("settings.scroll_assist_off"),
            ),
        ] {
            scroll_assist = scroll_assist.child(self.button(
                format!("scroll-assist-{}", mode.preference()),
                (
                    if assist_mode == mode {
                        UiIcon::RadioSelected
                    } else {
                        UiIcon::Radio
                    },
                    label,
                ),
                cx,
                move |s, _, _| {
                    s.local.preferences.terminal_scroll_assist = mode.preference().into();
                    s.save();
                },
            ));
        }
        let mut font = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(tr!(
                "settings.terminal_font_size",
                self.local.preferences.font_size
            )));
        for (delta, label) in [(-1, t("settings.smaller")), (1, t("settings.larger"))] {
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
            tr!("font.default", platform::TERMINAL_FONT)
        } else {
            tr!("font.missing_using_default", stored)
        };
        let terminal = self
            .settings_card(
                "settings.terminal_input",
                "settings.terminal_input_description",
            )
            .child(font)
            .child(self.button(
                "terminal-font",
                (
                    UiIcon::ChevronRight,
                    tr!("font.terminal_font_value", family),
                ),
                cx,
                |s, _, cx| s.open_terminal_fonts(cx),
            ))
            .child(
                div()
                    .id("scrollback-setting")
                    .text_xs()
                    .native_tooltip(tr!(
                        "settings.scrollback_description",
                        self.local.preferences.scrollback
                    ))
                    .child(tr!(
                        "settings.scrollback_lines",
                        self.local.preferences.scrollback
                    )),
            )
            .child(
                self.button(
                    "tmux-buffer-clipboard",
                    (
                        if self.local.preferences.desktop_tmux_buffer_clipboard {
                            UiIcon::CheckboxChecked
                        } else {
                            UiIcon::Checkbox
                        },
                        t("settings.copy_tmux_buffers"),
                    ),
                    cx,
                    |s, _, _| {
                        s.local.preferences.desktop_tmux_buffer_clipboard =
                            !s.local.preferences.desktop_tmux_buffer_clipboard;
                        s.save();
                    },
                )
                .native_tooltip(t("settings.copy_tmux_buffers_description")),
            )
            .child(
                div()
                    .id("scroll-assist-title")
                    .native_tooltip(t("settings.scroll_assist_description"))
                    .child(t("settings.scroll_assist_title")),
            )
            .child(scroll_assist)
            .child(self.button(
                "edit-shortcuts",
                (
                    UiIcon::ChevronRight,
                    tr!(
                        "settings.bottom_shortcuts_summary",
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
            (
                crate::power::Mode::System,
                t("settings.power_saving_follow_os"),
            ),
            (
                crate::power::Mode::Always,
                t("settings.power_saving_always"),
            ),
            (crate::power::Mode::Off, t("settings.power_saving_off")),
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
            .settings_card("settings.workspace", "settings.workspace_description")
            .child(self.button(
                "show-all-hierarchy",
                (
                    if self.local.preferences.show_all_hierarchy {
                        UiIcon::CheckboxChecked
                    } else {
                        UiIcon::Checkbox
                    },
                    t("settings.show_single_windows_too"),
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
                    .native_tooltip(t("settings.power_saving_description"))
                    .child(t("settings.power_saving_pacing")),
            )
            .child(power);
        let alerts = self
            .settings_card("common.notifications", "settings.notifications_description")
            .child(self.agent_settings(cx));
        let sync = self
            .button(
                "export-sync",
                (UiIcon::ChevronRight, t("sync.export_and_sync")),
                cx,
                |s, _, _| {
                    s.settings.main_offset = s.modal.scroll.offset();
                    s.settings.entry = "export-sync";
                    s.dialog = Some(Dialog::ExportSync);
                },
            )
            .native_tooltip(t("settings.sync_description"));
        let mut security = self
            .settings_card("settings.security_storage", platform::SHORTCUT_HELP)
            .child(t("settings.trusted_servers"));
        for (endpoint, fingerprint) in &self.local.trust {
            let e = endpoint.clone();
            security = security
                .child(format!("{endpoint}\n{fingerprint}"))
                .child(self.button(
                    format!("forget-{endpoint}"),
                    t("settings.remove_trust"),
                    cx,
                    move |s, _, _| {
                        s.dialog = Some(Dialog::Delete {
                            title: tr!("settings.remove_trust_named", e),
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
            .child(div().text_xl().child(t("common.settings")))
            .child(appearance)
            .child(terminal)
            .child(workspace)
            .child(alerts)
            .child(sync)
            .child(security)
            .child(
                self.button("about-app", t("settings.about"), cx, |s, _, _| {
                    s.settings.main_offset = s.modal.scroll.offset();
                    s.settings.entry = "about-app";
                    s.dialog = Some(Dialog::About);
                }),
            )
    }
    pub(super) fn settings_back(&mut self) {
        self.cancel_import_preview();
        self.settings.entry = match self.dialog {
            Some(Dialog::About) => "about-app",
            Some(Dialog::ExportSync) => "export-sync",
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
                    .child(t("settings.bottom_shortcuts"))
                    .native_tooltip(format!(
                        "{}\n{}",
                        t("shortcut.drag_hint"),
                        t("shortcut.modifiers_once")
                    )),
            )
            .child(
                self.button("shortcuts-back", t("common.back"), cx, |s, _, _| {
                    s.settings_back()
                }),
            )
            .child(div().text_lg().child(t("shortcut.active")));
        let keys = &self.local.preferences.terminal_shortcuts;
        if keys.is_empty() {
            active = active.child(t("shortcut.none_active"));
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
                        t("shortcut.exclude"),
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
        active = active.child(div().mt_4().text_lg().child(t("shortcut.excluded")));
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
                        t("common.add"),
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
        active.child(
            self.button("reset-shortcuts", t("shortcut.reset"), cx, |s, _, _| {
                if s.settings.dragging.is_some() {
                    return;
                }
                s.local.preferences.terminal_shortcuts = TerminalShortcut::defaults();
                s.save();
            }),
        )
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
                tr!("font.load_failed_using_default", family),
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
            .child(div().text_xl().child(t("font.terminal_font")))
            .child(
                self.button("terminal-font-back", t("common.back"), cx, |s, _, _| {
                    s.settings_back()
                }),
            )
            .child(div().text_xs().child(t("font.description")))
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
            return page.child(t("font.loading_fonts"));
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
            tr!("font.default", platform::TERMINAL_FONT),
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
            page = page.child(div().text_xs().child(t("font.none_found")));
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
