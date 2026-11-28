use super::*;
use dgl_core::i18n::Language;
use dgl_core::input::{TerminalShortcut, move_shortcut};
use settings_ui::{ActionKind, NAV_STRIP_H, NAV_WIDTH, PAGE_GAP};
use terminal::fonts::TerminalFace;

/// The settings categories in navigation order. Each one is a detail page of the
/// single settings dialog; only the font list and the launch editor go deeper.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum SettingsCategory {
    #[default]
    General,
    Terminal,
    Shortcuts,
    Power,
    Agents,
    Launch,
    Sync,
    Security,
    About,
}
impl SettingsCategory {
    pub const ALL: [Self; 9] = [
        Self::General,
        Self::Terminal,
        Self::Shortcuts,
        Self::Power,
        Self::Agents,
        Self::Launch,
        Self::Sync,
        Self::Security,
        Self::About,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Terminal => "terminal",
            Self::Shortcuts => "shortcuts",
            Self::Power => "power",
            Self::Agents => "agents",
            Self::Launch => "launch",
            Self::Sync => "sync",
            Self::Security => "security",
            Self::About => "about",
        }
    }
    /// The navigation button's element and focus id.
    pub fn nav_id(self) -> &'static str {
        match self {
            Self::General => "settings-nav-general",
            Self::Terminal => "settings-nav-terminal",
            Self::Shortcuts => "settings-nav-shortcuts",
            Self::Power => "settings-nav-power",
            Self::Agents => "settings-nav-agents",
            Self::Launch => "settings-nav-launch",
            Self::Sync => "settings-nav-sync",
            Self::Security => "settings-nav-security",
            Self::About => "settings-nav-about",
        }
    }
    /// The modal kind; a new kind starts the detail scroll at the top.
    pub fn kind(self) -> &'static str {
        match self {
            Self::General => "settings-general",
            Self::Terminal => "settings-terminal",
            Self::Shortcuts => "settings-shortcuts",
            Self::Power => "settings-power",
            Self::Agents => "settings-agents",
            Self::Launch => "settings-launch",
            Self::Sync => "settings-sync",
            Self::Security => "settings-security",
            Self::About => "settings-about",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "settings.category_general",
            Self::Terminal => "settings.category_terminal",
            Self::Shortcuts => "settings.category_shortcuts",
            Self::Power => "settings.category_power",
            Self::Agents => "settings.category_agents",
            Self::Launch => "settings.category_launch",
            Self::Sync => "sync.export_and_sync",
            Self::Security => "settings.security_storage",
            Self::About => "settings.about",
        }
    }
    pub fn description(self) -> Option<&'static str> {
        match self {
            Self::General => Some("settings.general_description"),
            Self::Terminal => Some("settings.terminal_description"),
            Self::Shortcuts => Some("settings.shortcuts_description"),
            Self::Power => Some("settings.power_saving_description"),
            Self::Agents => Some("settings.agents_description"),
            Self::Launch => Some("launcher.settings"),
            Self::Sync => Some("sync.description"),
            Self::Security => Some("settings.security_description"),
            Self::About => None,
        }
    }
    /// Whether a spacer separates this category from the previous cluster.
    pub fn cluster_start(self) -> bool {
        matches!(self, Self::Agents | Self::Sync)
    }
}

#[derive(Default)]
pub(super) struct SettingsState {
    pub category: SettingsCategory,
    /// Detail scroll offset to restore when a second-level page returns.
    main_offset: Point<Pixels>,
    pub restore: bool,
    /// The control focused again when `restore` is set.
    pub entry: SharedString,
    pub dragging: Option<TerminalShortcut>,
    destination: Option<usize>,
    pointer: Option<Point<Pixels>>,
    rows: HashMap<usize, Bounds<Pixels>>,
    /// Installed monospace families, read when the font page first opens.
    fonts: Option<Vec<String>>,
    loading_fonts: bool,
    /// The category list keeps its own scroll; the tracked modal scroll is the detail.
    nav: ScrollHandle,
    nav_reveal: crate::modal::RevealState,
    /// Whether the last build laid the category list out as a horizontal strip.
    nav_narrow: bool,
}
impl SettingsState {
    pub fn main_offset(&self) -> Point<Pixels> {
        self.main_offset
    }
    /// Scrolls the category list so its focused item is visible: sideways in the
    /// narrow strip, vertically in the column.
    pub fn reveal_nav_on_focus(
        &self,
        focus: FocusHandle,
    ) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + 'static {
        crate::modal::reveal_on_focus_in(
            self.nav.clone(),
            self.nav_reveal.clone(),
            focus,
            if self.nav_narrow {
                crate::modal::RevealAxis::Horizontal
            } else {
                crate::modal::RevealAxis::Vertical
            },
        )
    }
}
#[derive(Clone)]
struct DragShortcut(TerminalShortcut);
impl Desktop {
    /// Opens settings on `category` with fresh focus; the font list stays cached.
    pub(super) fn open_settings(&mut self, category: SettingsCategory) {
        self.cancel_import_preview();
        self.clear_shortcut_drag();
        self.settings.category = category;
        self.settings.main_offset = Point::default();
        self.settings.restore = false;
        self.settings.entry = SharedString::default();
        self.settings.nav.set_offset(Point::default());
        self.settings.nav_reveal.reset();
        self.dialog = Some(Dialog::Settings);
    }
    /// Shows another category, keeping focus on its navigation button.
    pub(super) fn select_settings_category(&mut self, category: SettingsCategory) {
        match self.dialog {
            Some(Dialog::Settings) if self.settings.category == category => return,
            Some(Dialog::TerminalFont) if category == SettingsCategory::Terminal => {
                self.settings_back();
                return;
            }
            _ => {}
        }
        if self.settings.category == SettingsCategory::Sync {
            self.cancel_import_preview();
        }
        self.clear_shortcut_drag();
        self.settings.category = category;
        self.settings.main_offset = Point::default();
        self.settings.entry = category.nav_id().into();
        self.settings.restore = true;
        self.dialog = Some(Dialog::Settings);
    }
    pub(super) fn clear_shortcut_drag(&mut self) {
        self.settings.dragging = None;
        self.settings.destination = None;
        self.settings.pointer = None;
    }
    /// Remembers the detail scroll and the control that opens a second-level page.
    pub(super) fn remember_settings_entry(&mut self, entry: impl Into<SharedString>) {
        self.settings.main_offset = self.modal.scroll.offset();
        self.settings.entry = entry.into();
    }
    /// Returns from the font list to the terminal category.
    pub(super) fn settings_back(&mut self) {
        self.clear_shortcut_drag();
        self.settings.category = SettingsCategory::Terminal;
        self.settings.entry = "terminal-font".into();
        self.settings.restore = true;
        self.dialog = Some(Dialog::Settings);
    }
    /// Returns from the launch editor to the quick launch category and its entry.
    pub(super) fn return_from_launch_editor(&mut self) {
        self.settings.category = SettingsCategory::Launch;
        self.settings.restore = true;
        self.dialog = Some(Dialog::Settings);
    }
    /// The settings dialog body: category navigation beside (or, in narrow windows,
    /// above) the scrolling detail. Navigation is built first so the focus ring runs
    /// close → categories → detail.
    pub(super) fn settings_shell(
        &mut self,
        font_page: bool,
        max_h: f32,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette;
        let current = if font_page {
            SettingsCategory::Terminal
        } else {
            self.settings.category
        };
        if self.settings.nav_narrow != narrow {
            self.settings.nav.set_offset(Point::default());
            self.settings.nav_reveal.reset();
        }
        self.settings.nav_narrow = narrow;
        let mut nav = div()
            .id("settings-nav")
            .debug_selector(|| "settings-nav".into())
            .track_scroll(&self.settings.nav)
            .flex()
            .gap_1()
            .flex_shrink_0();
        nav = if narrow {
            nav.flex_row()
                .items_center()
                .h(px(NAV_STRIP_H))
                .overflow_x_scroll()
        } else {
            nav.flex_col().w(px(NAV_WIDTH)).h_full().overflow_y_scroll()
        };
        for category in SettingsCategory::ALL {
            if category.cluster_start() {
                nav = nav.child(if narrow {
                    div().w(px(8.)).flex_shrink_0()
                } else {
                    div().h(px(10.)).flex_shrink_0()
                });
            }
            nav = nav.child(self.settings_nav_item(category, category == current, narrow, cx));
        }
        let header = if font_page {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    self.button_base("terminal-font-back", cx, |s, _, _| s.settings_back())
                        .size(px(settings_ui::CONTROL_H))
                        .p_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .border_color(rgb(p.control_border))
                        .native_tooltip(t("common.back"))
                        .child(UiIcon::ArrowLeft.render(p.fg)),
                )
                .child(self.settings_page(
                    format!(
                        "{} › {}",
                        t(SettingsCategory::Terminal.title()),
                        t("font.terminal_font")
                    ),
                    None,
                ))
        } else {
            self.settings_page(
                t(current.title()),
                current.description().map(|key| t(key).into()),
            )
        };
        let page = if font_page {
            self.terminal_font_settings(cx)
        } else {
            match current {
                SettingsCategory::General => self.settings_general(cx),
                SettingsCategory::Terminal => self.settings_terminal(cx),
                SettingsCategory::Shortcuts => self.shortcut_settings(cx),
                SettingsCategory::Power => self.settings_power(cx),
                SettingsCategory::Agents => div()
                    .flex()
                    .flex_col()
                    .gap(px(PAGE_GAP))
                    .child(self.agent_settings(cx))
                    .child(self.agent_status_settings(cx)),
                SettingsCategory::Launch => self.launch_settings(cx),
                SettingsCategory::Sync => self.export_sync_settings(cx),
                SettingsCategory::Security => self.settings_security(cx),
                SettingsCategory::About => self.about_app(cx),
            }
        };
        let detail = div()
            .id("dialog-body")
            .debug_selector(|| "dialog-body".into())
            .track_scroll(&self.modal.scroll)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .debug_selector(move || format!("settings-page-{}", current.id()))
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .gap(px(PAGE_GAP))
                    .pr_2()
                    .pb_2()
                    .child(header)
                    .child(page),
            );
        if narrow {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .h(px(max_h))
                .child(nav)
                .child(detail)
        } else {
            div()
                .flex()
                .h(px(max_h))
                .child(nav)
                .child(
                    div()
                        .w(px(1.))
                        .h_full()
                        .flex_shrink_0()
                        .mx_4()
                        .bg(rgb(p.line)),
                )
                .child(detail.h_full())
        }
    }
    fn settings_general(&mut self, cx: &mut Context<Self>) -> Div {
        let choice =
            Language::from_code(&self.local.preferences.language).map_or("system", Language::code);
        let languages = std::iter::once(("system", t("common.system")))
            .chain(Language::all().map(|language| (language.code(), language.native_name())))
            .map(|(value, label)| {
                self.settings_choice(
                    format!("language-{value}"),
                    label,
                    choice == value,
                    cx,
                    move |s, _, cx| s.language(value, cx),
                )
                .into_any_element()
            })
            .collect();
        let themes = [
            ("system", t("common.system")),
            ("light", t("settings.theme_light")),
            ("dark", t("settings.theme_dark")),
        ]
        .into_iter()
        .map(|(value, label)| {
            self.settings_choice(
                format!("theme-{value}"),
                label,
                self.local.preferences.theme == value,
                cx,
                move |s, _, cx| s.theme(value, cx),
            )
            .into_any_element()
        })
        .collect();
        let appearance = self.settings_group(
            "appearance",
            None,
            None,
            vec![
                self.settings_stack(Some(t("settings.language").into()), None, languages)
                    .into_any_element(),
                self.settings_stack_marked(
                    Some(t("settings.theme").into()),
                    None,
                    themes,
                    Some("theme"),
                )
                .into_any_element(),
            ],
        );
        let accent = self.accent_settings(cx);
        let workspace = self.settings_group(
            "workspace",
            Some(t("settings.workspace").into()),
            None,
            vec![
                self.settings_toggle(
                    "show-all-hierarchy",
                    t("settings.show_single_windows_too"),
                    Some(t("settings.show_single_windows_description").into()),
                    self.local.preferences.show_all_hierarchy,
                    true,
                    cx,
                    |s, _, _| {
                        s.local.preferences.show_all_hierarchy =
                            !s.local.preferences.show_all_hierarchy;
                        s.save();
                    },
                )
                .into_any_element(),
            ],
        );
        div()
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(appearance)
            .child(accent)
            .child(workspace)
    }
    fn settings_terminal(&self, cx: &mut Context<Self>) -> Div {
        let mut stepper = div().flex().items_center().gap_2();
        for (delta, label) in [(-1, t("settings.smaller")), (1, t("settings.larger"))] {
            stepper = stepper.child(self.settings_action(
                format!("font-size-{delta}"),
                label,
                ActionKind::Normal,
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
        let text = self.settings_group(
            "text",
            Some(t("settings.group_text").into()),
            None,
            vec![
                self.settings_row_with(
                    self.settings_label_marked(
                        tr!(
                            "settings.terminal_font_size",
                            self.local.preferences.font_size
                        ),
                        None,
                        Some("font-size"),
                    ),
                    stepper,
                )
                .into_any_element(),
                self.settings_link(
                    "terminal-font",
                    t("font.terminal_font"),
                    Some(family.into()),
                    cx,
                    |s, _, cx| s.open_terminal_fonts(cx),
                )
                .into_any_element(),
            ],
        );
        let display = self.settings_group(
            "display",
            Some(t("settings.group_display").into()),
            None,
            vec![
                self.settings_row_with(
                    self.settings_label_marked(
                        tr!(
                            "settings.scrollback_description",
                            self.local.preferences.scrollback
                        ),
                        None,
                        Some("scrollback"),
                    ),
                    div(),
                )
                .id("scrollback-setting")
                .debug_selector(|| "scrollback-setting".into())
                .into_any_element(),
            ],
        );
        let assist_mode = dgl_core::scroll::ScrollAssist::from_preference(
            &self.local.preferences.terminal_scroll_assist,
        );
        let choices = [
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
        ]
        .into_iter()
        .map(|(mode, label)| {
            self.settings_choice(
                format!("scroll-assist-{}", mode.preference()),
                label,
                assist_mode == mode,
                cx,
                move |s, _, _| {
                    s.local.preferences.terminal_scroll_assist = mode.preference().into();
                    s.save();
                },
            )
            .into_any_element()
        })
        .collect();
        let mode_note = t(match assist_mode {
            dgl_core::scroll::ScrollAssist::Auto => "settings.scroll_assist_auto_description",
            dgl_core::scroll::ScrollAssist::On => "settings.scroll_assist_on_description",
            dgl_core::scroll::ScrollAssist::Off => "settings.scroll_assist_off_description",
        });
        let scroll_assist = self.settings_group(
            "scroll-assist",
            Some(t("settings.scroll_assist_title").into()),
            Some(t("settings.scroll_assist_description").into()),
            vec![
                self.settings_stack(None, None, choices)
                    .child(self.settings_note(mode_note))
                    .into_any_element(),
            ],
        );
        let clipboard = self.settings_group(
            "clipboard",
            Some(t("settings.group_clipboard").into()),
            None,
            vec![
                self.settings_toggle(
                    "tmux-buffer-clipboard",
                    t("settings.copy_tmux_buffers"),
                    Some(t("settings.copy_tmux_buffers_description").into()),
                    self.local.preferences.desktop_tmux_buffer_clipboard,
                    true,
                    cx,
                    |s, _, _| {
                        s.local.preferences.desktop_tmux_buffer_clipboard =
                            !s.local.preferences.desktop_tmux_buffer_clipboard;
                        s.save();
                    },
                )
                .into_any_element(),
            ],
        );
        div()
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(text)
            .child(display)
            .child(scroll_assist)
            .child(clipboard)
    }
    fn settings_power(&self, cx: &mut Context<Self>) -> Div {
        let power_mode =
            crate::power::Mode::from_preference(&self.local.preferences.desktop_power_saving);
        let choices = [
            (
                crate::power::Mode::System,
                t("settings.power_saving_follow_os"),
            ),
            (
                crate::power::Mode::Always,
                t("settings.power_saving_always"),
            ),
            (crate::power::Mode::Off, t("settings.power_saving_off")),
        ]
        .into_iter()
        .map(|(mode, label)| {
            self.settings_choice(
                format!("power-saving-{}", mode.preference()),
                label,
                power_mode == mode,
                cx,
                move |s, _, _| {
                    s.local.preferences.desktop_power_saving = mode.preference().into();
                    s.save();
                },
            )
            .into_any_element()
        })
        .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(self.settings_group(
                "power-saving",
                Some(t("settings.power_saving_pacing").into()),
                None,
                vec![self.settings_stack(None, None, choices).into_any_element()],
            ))
    }
    fn settings_security(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let mut servers = vec![];
        for (endpoint, fingerprint) in &self.local.trust {
            let e = endpoint.clone();
            servers.push(
                self.settings_row_with(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(settings_ui::BODY))
                                .child(endpoint.clone()),
                        )
                        .child(
                            self.settings_note(fingerprint.clone())
                                .font_family(platform::TERMINAL_FONT),
                        ),
                    self.settings_action(
                        format!("forget-{endpoint}"),
                        t("settings.remove_trust"),
                        ActionKind::Danger,
                        cx,
                        move |s, _, _| {
                            s.dialog = Some(Dialog::Delete {
                                title: tr!("settings.remove_trust_named", e),
                                operation: DeleteOp::Trust(e.clone()),
                            });
                        },
                    ),
                )
                .into_any_element(),
            );
        }
        if servers.is_empty() {
            servers.push(
                div()
                    .px(px(settings_ui::ROW_PX))
                    .py(px(settings_ui::ROW_PY))
                    .child(self.settings_note(t("settings.no_trusted_servers")))
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(self.settings_group(
                "trusted-servers",
                Some(t("settings.trusted_servers").into()),
                None,
                servers,
            ))
            .child(self.settings_group(
                "storage",
                None,
                None,
                vec![
                    self.settings_row(
                        t("settings.storage_location"),
                        None,
                        div()
                            .text_size(px(settings_ui::BODY))
                            .text_color(rgb(p.muted))
                            .child(platform::STORAGE_LABEL),
                    )
                    .id("settings-storage")
                    .debug_selector(|| "settings-storage".into())
                    .into_any_element(),
                ],
            ))
    }
    /// The app's own keyboard shortcuts, read-only.
    fn settings_app_shortcuts(&self) -> Stateful<Div> {
        let p = self.palette;
        let help = t(platform::SHORTCUT_HELP);
        let parsed: Option<Vec<(&str, &str)>> = help
            .split(" · ")
            .map(|entry| entry.split_once(' '))
            .collect();
        let rows = match parsed {
            Some(pairs) => pairs
                .into_iter()
                .map(|(key, action)| {
                    self.settings_row(
                        action.to_owned(),
                        None,
                        div()
                            .px_2()
                            .py(px(2.))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(p.control_border))
                            .bg(rgb(p.bg))
                            .text_size(px(settings_ui::BODY))
                            .font_family(platform::TERMINAL_FONT)
                            .child(key.to_owned()),
                    )
                    .into_any_element()
                })
                .collect(),
            None => vec![
                div()
                    .px(px(settings_ui::ROW_PX))
                    .py(px(settings_ui::ROW_PY))
                    .child(self.settings_note(help))
                    .into_any_element(),
            ],
        };
        self.settings_group(
            "app-shortcuts",
            Some(t("settings.app_shortcuts").into()),
            None,
            rows,
        )
    }
    pub(super) fn tick_shortcut_drag(&mut self, cx: &mut Context<Self>) {
        if self.settings.dragging.is_none() {
            return;
        }
        if !cx.has_active_drag()
            || !matches!(self.dialog, Some(Dialog::Settings))
            || self.settings.category != SettingsCategory::Shortcuts
        {
            self.clear_shortcut_drag();
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
        let keys = &self.local.preferences.terminal_shortcuts;
        let mut active = vec![];
        if keys.is_empty() {
            active.push(
                div()
                    .px(px(settings_ui::ROW_PX))
                    .py(px(settings_ui::ROW_PY))
                    .child(self.settings_note(t("shortcut.none_active")))
                    .into_any_element(),
            );
        }
        for (index, &key) in keys.iter().enumerate() {
            let view = cx.entity();
            let paint_view = view.clone();
            let insertion =
                self.settings.dragging.is_some() && self.settings.destination == Some(index);
            let dragging = self.settings.dragging == Some(key);
            let count = keys.len();
            active.push(
                div()
                    .id(SharedString::from(format!("shortcut-row-{index}")))
                    .debug_selector(move || format!("shortcut-row-{index}"))
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(8.))
                    .py(px(5.))
                    .min_h(px(40.))
                    .when(dragging, |d| d.bg(rgb(p.selection)))
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
                        s.clear_shortcut_drag();
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
                        self.settings_icon(
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
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(settings_ui::BODY))
                            .child(key.label()),
                    )
                    .child(
                        self.settings_icon(
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
                        self.settings_icon(
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
                    .child(self.settings_action(
                        format!("shortcut-setting-{}", key.label()),
                        t("shortcut.exclude"),
                        ActionKind::Normal,
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
                    })
                    .into_any_element(),
            );
        }
        let excluded: Vec<AnyElement> = TerminalShortcut::all()
            .into_iter()
            .filter(|k| !keys.contains(k))
            .map(|key| {
                self.settings_row(
                    key.label(),
                    None,
                    self.settings_action(
                        format!("shortcut-setting-{}", key.label()),
                        t("common.add"),
                        ActionKind::Normal,
                        cx,
                        move |s, _, _| {
                            if s.settings.dragging.is_some() {
                                return;
                            }
                            s.local.preferences.terminal_shortcuts.push(key);
                            s.save();
                        },
                    ),
                )
                .into_any_element()
            })
            .collect();
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(self.settings_app_shortcuts())
            .child(
                div()
                    .id("shortcut-settings-title")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_1()
                    .child(
                        div()
                            .text_size(px(settings_ui::GROUP_TITLE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(self.palette.fg))
                            .child(t("settings.bottom_shortcuts")),
                    )
                    .child(self.settings_note(t("shortcut.drag_hint")))
                    .child(self.settings_note(t("shortcut.modifiers_once"))),
            )
            .child(self.settings_group(
                "shortcut-active",
                Some(t("shortcut.active").into()),
                None,
                active,
            ));
        if !excluded.is_empty() {
            page = page.child(self.settings_group(
                "shortcut-excluded",
                Some(t("shortcut.excluded").into()),
                None,
                excluded,
            ));
        }
        page.child(div().flex().child(self.settings_action(
            "reset-shortcuts",
            t("shortcut.reset"),
            ActionKind::Normal,
            cx,
            |s, _, _| {
                if s.settings.dragging.is_some() {
                    return;
                }
                s.local.preferences.terminal_shortcuts = TerminalShortcut::defaults();
                s.save();
            },
        )))
    }
}
impl Desktop {
    /// Opens the family list; the system font collection is read off the UI thread.
    pub(super) fn open_terminal_fonts(&mut self, cx: &mut Context<Self>) {
        self.remember_settings_entry("terminal-font");
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
        let preview = div()
            .px(px(settings_ui::ROW_PX))
            .py(px(settings_ui::ROW_PY))
            .child(
                div()
                    .id("terminal-font-preview")
                    .debug_selector(|| "terminal-font-preview".into())
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
        let page = div().flex().flex_col().gap(px(PAGE_GAP));
        let Some(fonts) = &self.settings.fonts else {
            return page.child(self.settings_group(
                "terminal-font",
                None,
                Some(t("font.description").into()),
                vec![
                    preview.into_any_element(),
                    div()
                        .px(px(settings_ui::ROW_PX))
                        .py(px(settings_ui::ROW_PY))
                        .child(self.settings_note(t("font.loading_fonts")))
                        .into_any_element(),
                ],
            ));
        };
        let option =
            |id: String, label: String, selected: bool, family: String, cx: &mut Context<Self>| {
                self.settings_choice(id, label, selected, cx, move |s, _, cx| {
                    s.set_terminal_font(&family, cx)
                })
                .w_full()
            };
        let mut list = div()
            .flex()
            .flex_col()
            .gap_1()
            .px(px(settings_ui::ROW_PX))
            .py(px(settings_ui::ROW_PY))
            .child(option(
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
            list = list.child(self.settings_note(t("font.none_found")));
        }
        for family in installed {
            list = list.child(option(
                format!("terminal-font-{family}"),
                family.clone(),
                face.selected.as_ref() == family.as_str(),
                family.clone(),
                cx,
            ));
        }
        page.child(self.settings_group(
            "terminal-font",
            None,
            Some(t("font.description").into()),
            vec![preview.into_any_element(), list.into_any_element()],
        ))
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's `test` attribute would replace the standard one.
    use super::SettingsCategory;
    use dgl_core::i18n::t;
    use std::collections::HashSet;

    #[test]
    fn categories_have_unique_ids_kinds_and_known_texts() {
        let mut seen = HashSet::new();
        for category in SettingsCategory::ALL {
            for value in [
                category.id(),
                category.nav_id(),
                category.kind(),
                category.title(),
            ] {
                assert!(seen.insert(value), "{value}");
            }
            assert_eq!(category.nav_id(), format!("settings-nav-{}", category.id()));
            assert_eq!(category.kind(), format!("settings-{}", category.id()));
            for key in std::iter::once(category.title()).chain(category.description()) {
                assert_ne!(t(key), key, "{key}");
            }
        }
        assert_eq!(SettingsCategory::default(), SettingsCategory::General);
        assert_eq!(SettingsCategory::ALL.len(), seen.len() / 4);
        let clusters: Vec<_> = SettingsCategory::ALL
            .into_iter()
            .filter(|c| c.cluster_start())
            .collect();
        assert_eq!(clusters, [SettingsCategory::Agents, SettingsCategory::Sync]);
    }
}
