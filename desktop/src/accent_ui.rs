use super::*;
use dgl_core::accent::{PRESETS, normalize_hex, parse_hex};

impl Desktop {
    pub(super) fn init_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.appearance = Some(appearance::Appearance::new());
        self.refresh_appearance(window, cx);
        cx.spawn_in(window, async move |view, cx| {
            loop {
                Timer::after(Duration::from_millis(250)).await;
                if view
                    .update_in(cx, |s, w, cx| s.refresh_appearance(w, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    fn refresh_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .appearance
            .as_mut()
            .is_some_and(|a| a.refresh(window, &self.local.preferences.theme))
        {
            cx.notify();
        }
    }
    pub(super) fn apply_accent(&mut self, cx: &mut Context<Self>) {
        let Some(color) = normalize_hex(&self.accent_input.read(cx).content) else {
            self.accent_error = true;
            cx.notify();
            return;
        };
        self.local.preferences.accent.mode = "custom".into();
        self.local.preferences.accent.custom_color = Some(color.clone());
        self.accent_input
            .update(cx, |i, cx| i.set_content(&color, cx));
        self.accent_error = false;
        self.save();
        cx.notify();
    }
    /// The accent colour group of the general settings page.
    pub(super) fn accent_settings(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette;
        self.accent_input.update(cx, |i, _| i.palette = p);
        // Keep the editor entity and draft across rendering and mode switches.
        let mode = self.local.preferences.accent.mode.clone();
        let modes = [
            ("system", t("common.system")),
            ("default", t("accent.dagali_default")),
            ("custom", t("accent.custom")),
        ]
        .into_iter()
        .map(|(value, label)| {
            self.settings_choice(
                format!("accent-{value}"),
                label,
                mode == value,
                cx,
                move |s, _, _| {
                    s.local.preferences.accent.mode = value.into();
                    s.save();
                },
            )
            .into_any_element()
        })
        .collect();
        let custom = self
            .local
            .preferences
            .accent
            .custom_color
            .as_deref()
            .filter(|_| mode == "custom")
            .and_then(parse_hex);
        let presets = PRESETS
            .iter()
            .map(|&(label, color)| {
                let raw = parse_hex(color).unwrap();
                self.settings_choice(
                    format!("accent-preset-{color}"),
                    t(label),
                    custom == Some(raw),
                    cx,
                    move |s, _, cx| {
                        s.accent_input.update(cx, |i, cx| i.set_content(color, cx));
                        s.apply_accent(cx);
                    },
                )
                .child(
                    div()
                        .size(px(18.))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(p.line))
                        .bg(rgb(raw)),
                )
                .into_any_element()
            })
            .collect();
        let raw = parse_hex(&self.accent_input.read(cx).content);
        let focus = self.accent_input.focus_handle(cx);
        // The input stays in the modal focus ring in every mode.
        self.modal.input(focus.clone());
        let invalid = raw.is_none() || self.accent_error;
        let hex = div()
            .flex()
            .flex_col()
            .gap_2()
            .px(px(settings_ui::ROW_PX))
            .py(px(settings_ui::ROW_PY))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(if invalid { p.danger } else { p.control_border }))
                            .on_children_prepainted(self.modal.reveal_on_focus(focus))
                            .child(self.accent_input.clone()),
                    )
                    .child(
                        div()
                            .size(px(24.))
                            .flex_shrink_0()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(p.line))
                            .bg(rgb(raw.unwrap_or(p.bg))),
                    )
                    .child(self.settings_action(
                        "apply-accent",
                        t("accent.apply"),
                        settings_ui::ActionKind::Primary,
                        cx,
                        |s, _, cx| s.apply_accent(cx),
                    )),
            )
            .when(invalid, |d| {
                d.child(
                    self.settings_note(t("accent.invalid_hex"))
                        .text_color(rgb(p.danger)),
                )
            });
        self.settings_group(
            "accent",
            Some(t("accent.accent_color").into()),
            None,
            vec![
                self.settings_stack(None, None, modes).into_any_element(),
                self.settings_stack(None, None, presets).into_any_element(),
                hex.into_any_element(),
            ],
        )
    }
}
