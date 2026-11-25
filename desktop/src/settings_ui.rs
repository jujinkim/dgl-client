//! The settings screen's own component kit: roomy rows, groups and controls built on
//! the app's focusable buttons, so focus ring, reveal and hover/focus colours carry
//! over. Other screens keep the compact `button()` defaults. Plain GPUI only, so the
//! same code builds for macOS, Windows MSVC and Linux.
use super::*;

pub(super) const PANEL_WIDTH: f32 = 880.;
pub(super) const NAV_WIDTH: f32 = 196.;
/// Below this viewport width the category list becomes a strip above the detail.
pub(super) const NARROW_BELOW: f32 = 720.;
/// Explicit pixel sizes: the root text is 13px while rem stays 16px, so `text_sm`
/// would be larger than body text here.
const TITLE: f32 = 18.;
pub(super) const GROUP_TITLE: f32 = 14.;
pub(super) const BODY: f32 = 13.;
const NOTE: f32 = 12.;
pub(super) const CONTROL_H: f32 = 30.;
const CONTROL_PX: f32 = 12.;
const ACTION_PX: f32 = 14.;
pub(super) const ROW_PX: f32 = 14.;
pub(super) const ROW_PY: f32 = 10.;
pub(super) const ROW_MIN_H: f32 = 44.;
const CHECK_MIN_H: f32 = 40.;
pub(super) const PAGE_GAP: f32 = 20.;
const GROUP_GAP: f32 = 8.;
const NAV_ITEM_H: f32 = 32.;
/// Height of the horizontal category strip in narrow windows.
pub(super) const NAV_STRIP_H: f32 = 36.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ActionKind {
    Normal,
    Primary,
    Danger,
}

/// Dims a control that cannot be used; its handler still guards itself.
pub(super) fn settings_disabled<E: Styled>(element: E) -> E {
    element.opacity(0.45).cursor_default()
}

impl Desktop {
    /// The page header: title and an inline description.
    pub(super) fn settings_page(
        &self,
        title: impl Into<SharedString>,
        description: Option<SharedString>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(self.palette.fg))
                    .child(title.into()),
            )
            .when_some(description, |d, text| d.child(self.settings_note(text)))
    }
    /// Muted secondary text shown on screen instead of in a tooltip.
    pub(super) fn settings_note(&self, text: impl Into<SharedString>) -> Div {
        div()
            .text_size(px(NOTE))
            .line_height(relative(1.45))
            .text_color(rgb(self.palette.muted))
            .child(text.into())
    }
    /// Rows separated by 1px lines.
    pub(super) fn settings_rows(&self, rows: Vec<AnyElement>) -> Div {
        let mut list = div().flex().flex_col();
        for (index, row) in rows.into_iter().enumerate() {
            if index > 0 {
                list = list.child(div().h(px(1.)).flex_shrink_0().bg(rgb(self.palette.line)));
            }
            list = list.child(row);
        }
        list
    }
    /// A titled card of rows; the title and description sit above the card.
    pub(super) fn settings_group(
        &self,
        id: &'static str,
        title: Option<SharedString>,
        description: Option<SharedString>,
        rows: Vec<AnyElement>,
    ) -> Stateful<Div> {
        let p = self.palette;
        let heading = (title.is_some() || description.is_some()).then(|| {
            div()
                .id(SharedString::from(format!("{id}-title")))
                .flex()
                .flex_col()
                .gap(px(2.))
                .px_1()
                .when_some(title, |d, title| {
                    d.child(
                        div()
                            .text_size(px(GROUP_TITLE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.fg))
                            .child(title),
                    )
                })
                .when_some(description, |d, text| d.child(self.settings_note(text)))
        });
        div()
            .id(SharedString::from(format!("settings-group-{id}")))
            .debug_selector(move || format!("settings-group-{id}"))
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            .when_some(heading, |d, heading| d.child(heading))
            .child(
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(p.line))
                    .bg(rgb(p.panel))
                    .child(self.settings_rows(rows)),
            )
    }
    /// The label column of a row: text and an optional description below it.
    pub(super) fn settings_label(
        &self,
        label: impl Into<SharedString>,
        description: Option<SharedString>,
    ) -> Div {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(BODY)).child(label.into()))
            .when_some(description, |d, text| d.child(self.settings_note(text)))
    }
    /// A row with a custom left side and controls on the right.
    pub(super) fn settings_row_with(
        &self,
        left: impl IntoElement,
        control: impl IntoElement,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap_3()
            .px(px(ROW_PX))
            .py(px(ROW_PY))
            .min_h(px(ROW_MIN_H))
            .child(left)
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(control),
            )
    }
    /// Label and description on the left, a control on the right.
    pub(super) fn settings_row(
        &self,
        label: impl Into<SharedString>,
        description: Option<SharedString>,
        control: impl IntoElement,
    ) -> Div {
        self.settings_row_with(self.settings_label(label, description), control)
    }
    /// A label on top with wrapped controls below, for longer choice lists.
    pub(super) fn settings_stack(
        &self,
        label: Option<SharedString>,
        description: Option<SharedString>,
        content: Vec<AnyElement>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px(px(ROW_PX))
            .py(px(ROW_PY))
            .when_some(label, |d, label| {
                d.child(div().text_size(px(BODY)).child(label))
            })
            .when_some(description, |d, text| d.child(self.settings_note(text)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(content),
            )
    }
    /// A radio choice: selected ones are filled with the selection colour and outlined
    /// in the accent.
    pub(super) fn settings_choice(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette;
        self.button_base(id, cx, action)
            .h(px(CONTROL_H))
            .px(px(CONTROL_PX))
            .py_0()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .text_size(px(BODY))
            .border_color(rgb(if selected { p.accent } else { p.control_border }))
            .when(selected, |d| d.bg(rgb(p.selection)))
            .child(
                if selected {
                    UiIcon::RadioSelected
                } else {
                    UiIcon::Radio
                }
                .render(if selected { p.accent } else { p.fg }),
            )
            .child(label.into())
    }
    /// A whole-row switch with an inline description.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn settings_toggle(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        description: Option<SharedString>,
        on: bool,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette;
        let track = div()
            .flex_shrink_0()
            .w(px(34.))
            .h(px(20.))
            .p(px(2.))
            .rounded_full()
            .flex()
            .items_center()
            .when(on, |d| d.justify_end().bg(rgb(p.accent_fill)))
            .when(!on, |d| d.bg(rgb(p.control_border)))
            .child(
                div()
                    .size(px(16.))
                    .rounded_full()
                    .bg(rgb(if on { p.on_accent } else { p.bg })),
            );
        let row = self
            .button_base(id, cx, move |s, w, cx| {
                if enabled {
                    action(s, w, cx)
                }
            })
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px(px(ROW_PX))
            .py(px(ROW_PY))
            .min_h(px(ROW_MIN_H))
            .rounded_md()
            .child(self.settings_label(label, description))
            .child(track);
        if enabled { row } else { settings_disabled(row) }
    }
    /// A whole-row checkbox.
    pub(super) fn settings_check(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette;
        self.button_base(id, cx, action)
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px(px(ROW_PX))
            .py(px(8.))
            .min_h(px(CHECK_MIN_H))
            .rounded_md()
            .text_size(px(BODY))
            .child(
                if on {
                    UiIcon::CheckboxChecked
                } else {
                    UiIcon::Checkbox
                }
                .render(if on { p.accent } else { p.fg }),
            )
            .child(div().flex_1().min_w_0().child(label.into()))
    }
    /// A whole-row link to a second-level page or an external page.
    pub(super) fn settings_link(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: Option<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette;
        self.button_base(id, cx, action)
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px(px(ROW_PX))
            .py(px(ROW_PY))
            .min_h(px(ROW_MIN_H))
            .rounded_md()
            .text_size(px(BODY))
            .child(div().flex_1().min_w_0().child(label.into()))
            .when_some(value, |d, value| {
                d.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(p.muted))
                        .child(value),
                )
            })
            .child(UiIcon::ChevronRight.render(p.muted))
    }
    /// A text button sized for the settings screen.
    pub(super) fn settings_action(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        kind: ActionKind,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette;
        let button = self
            .button_base(id, cx, action)
            .h(px(CONTROL_H))
            .py_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_size(px(BODY))
            .whitespace_nowrap();
        let label = label.into();
        // The shared button hover and focus fill the control with `selection` and `fg`;
        // Primary and Danger keep their colours on an inner label instead, and show
        // focus with the border alone.
        match kind {
            ActionKind::Normal => button
                .px(px(ACTION_PX))
                .border_color(rgb(p.control_border))
                .child(label),
            ActionKind::Primary => button
                .px_0()
                .border_color(rgb(p.accent_fill))
                .focus(move |s| s.border_color(rgb(p.fg)))
                .child(
                    div()
                        .h_full()
                        .px(px(ACTION_PX))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .bg(rgb(p.accent_fill))
                        .text_color(rgb(p.on_accent))
                        .hover(|s| s.opacity(0.88))
                        .child(label),
                ),
            ActionKind::Danger => button
                .px(px(ACTION_PX))
                .border_color(rgb(p.danger))
                .focus(move |s| s.bg(rgb(p.selection)).border_color(rgb(p.danger)))
                .child(div().text_color(rgb(p.danger)).child(label)),
        }
    }
    /// An icon-only control at the settings control height.
    pub(super) fn settings_icon(
        &self,
        id: impl Into<SharedString>,
        icon: UiIcon,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        self.button(id, icon, cx, action)
            .size(px(CONTROL_H))
            .p_0()
            .rounded_md()
    }
    /// One category of the settings navigation.
    pub(super) fn settings_nav_item(
        &self,
        category: settings::SettingsCategory,
        selected: bool,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette;
        let bar = div().absolute().rounded_full().bg(rgb(p.accent));
        let bar = if narrow {
            bar.bottom_0().left(px(8.)).right(px(8.)).h(px(3.))
        } else {
            bar.left_0().top(px(6.)).bottom(px(6.)).w(px(3.))
        };
        self.button_base(category.nav_id(), cx, move |s, _, _| {
            s.select_settings_category(category)
        })
        .relative()
        .min_h(px(NAV_ITEM_H))
        .px(px(12.))
        .py(px(6.))
        .flex()
        .items_center()
        .rounded_md()
        .text_size(px(BODY))
        .when(narrow, |d| d.whitespace_nowrap())
        .when(!narrow, |d| d.w_full())
        .when(selected, |d| {
            d.bg(rgb(p.selection))
                .text_color(rgb(p.accent))
                .font_weight(FontWeight::SEMIBOLD)
                .child(bar)
        })
        .child(t(category.title()))
    }
}
