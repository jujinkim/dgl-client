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
    pub(super) fn accent_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        self.accent_input.update(cx, |i, _| i.palette = p);
        // Keep the editor entity and draft across rendering and mode switches.
        let mode = self.local.preferences.accent.mode.clone();
        let mut modes = div().flex().gap_2();
        for (value, label) in [
            ("system", t("시스템")),
            ("default", t("DGL 기본")),
            ("custom", t("사용자 지정")),
        ] {
            let selected = mode == value;
            modes = modes.child(
                self.button(format!("accent-{value}"), label, cx, move |s, _, _| {
                    s.local.preferences.accent.mode = value.into();
                    s.save();
                })
                .when(selected, |d| {
                    d.bg(rgb(p.accent_fill))
                        .text_color(rgb(p.on_accent))
                        .border_color(rgb(p.accent))
                }),
            );
        }
        let mut presets = div().flex().flex_wrap().gap_2();
        for (label, color) in PRESETS {
            let raw = parse_hex(color).unwrap();
            presets = presets.child(
                self.button(
                    format!("accent-preset-{color}"),
                    t(label),
                    cx,
                    move |s, _, cx| {
                        s.accent_input.update(cx, |i, cx| i.set_content(color, cx));
                        s.apply_accent(cx);
                    },
                )
                .child(div().w(px(18.)).h(px(18.)).bg(rgb(raw))),
            );
        }
        let raw = parse_hex(&self.accent_input.read(cx).content);
        let focus = self.accent_input.focus_handle(cx);
        self.modal.input(focus.clone());
        let invalid = raw.is_none();
        let mut result = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t("강조 색상"))
            .child(modes);
        // The input stays in the modal focus ring in every mode.
        result = result.child(presets).child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .border_1()
                        .border_color(rgb(if invalid || self.accent_error {
                            0xd63d45
                        } else {
                            p.line
                        }))
                        .on_children_prepainted(self.modal.reveal_on_focus(focus))
                        .child(self.accent_input.clone()),
                )
                .child(
                    div()
                        .w(px(24.))
                        .h(px(24.))
                        .border_1()
                        .border_color(rgb(p.line))
                        .bg(rgb(raw.unwrap_or(p.bg))),
                )
                .child(self.button("apply-accent", t("적용"), cx, |s, _, cx| {
                    s.apply_accent(cx)
                })),
        );
        if invalid || self.accent_error {
            result = result.child(t(
                "#RRGGBB 형식의 색상을 입력하세요. 기존 색상은 유지됩니다.",
            ));
        }
        result
    }
}
