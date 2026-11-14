use super::*;

#[derive(Clone, Copy)]
enum PaneAction {
    Split(bool),
    Focus(char),
    Resize(char),
    Close,
}

impl Desktop {
    fn pane_action(
        &mut self,
        tab_id: TabId,
        action: PaneAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(group) = self
            .workspace
            .groups
            .iter()
            .find_map(|(id, g)| (g.selected == Some(tab_id)).then_some(*id))
        else {
            return;
        };
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let connection = tab.connection.clone();
        let state = connection.state();
        if state.phase != Phase::Ready {
            return;
        }
        let Some(target) = state.pane.clone() else {
            return;
        };
        let operation = match action {
            PaneAction::Split(horizontal) => Operation::Split { target, horizontal },
            PaneAction::Focus(direction) => Operation::FocusPane { target, direction },
            PaneAction::Resize(direction) => Operation::ResizePane { target, direction },
            PaneAction::Close => Operation::KillPane { target },
        };
        self.group_focus(group, window, cx);
        if let Operation::KillPane { target } = &operation {
            self.dialog = Some(Dialog::Delete {
                title: tr!("pane.close_named", target),
                operation: DeleteOp::Remote(connection, operation),
            });
        } else {
            self.submit(&connection, Action::Manage { operation });
        }
    }

    pub(super) fn render_pane_toolbar(&self, tab: &Tab, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let tab_id = tab.id;
        let state = tab.connection.state();
        let enabled = state.phase == Phase::Ready && state.pane.is_some();
        let categories = [
            (
                "split",
                t("pane.split"),
                &[
                    (
                        UiIcon::SplitHorizontal,
                        t("pane.split_horizontally"),
                        PaneAction::Split(true),
                    ),
                    (
                        UiIcon::SplitVertical,
                        t("pane.split_vertically"),
                        PaneAction::Split(false),
                    ),
                ][..],
            ),
            (
                "focus",
                t("pane.navigate"),
                &[
                    (
                        UiIcon::ArrowLeft,
                        t("pane.focus_pane_left"),
                        PaneAction::Focus('L'),
                    ),
                    (
                        UiIcon::ArrowRight,
                        t("pane.focus_pane_right"),
                        PaneAction::Focus('R'),
                    ),
                    (
                        UiIcon::ArrowUp,
                        t("pane.focus_pane_above"),
                        PaneAction::Focus('U'),
                    ),
                    (
                        UiIcon::ArrowDown,
                        t("pane.focus_pane_below"),
                        PaneAction::Focus('D'),
                    ),
                ][..],
            ),
            (
                "resize",
                t("pane.resize"),
                &[
                    (
                        UiIcon::ArrowLeft,
                        t("pane.resize_left"),
                        PaneAction::Resize('L'),
                    ),
                    (
                        UiIcon::ArrowRight,
                        t("pane.resize_right"),
                        PaneAction::Resize('R'),
                    ),
                    (
                        UiIcon::ArrowUp,
                        t("pane.resize_up"),
                        PaneAction::Resize('U'),
                    ),
                    (
                        UiIcon::ArrowDown,
                        t("pane.resize_down"),
                        PaneAction::Resize('D'),
                    ),
                ][..],
            ),
            (
                "close",
                t("common.close"),
                &[(UiIcon::Close, t("pane.close_current"), PaneAction::Close)][..],
            ),
        ];
        let mut toolbar = div()
            .debug_selector(move || format!("tmux-toolbar-{tab_id}"))
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .p_1()
            .bg(rgb(p.panel))
            .border_b_1()
            .border_color(rgb(p.line))
            .text_xs();
        for (key, title, actions) in categories {
            let mut category = div()
                .debug_selector(move || format!("tmux-category-{tab_id}-{key}"))
                .flex()
                .items_center()
                .flex_shrink_0()
                .border_1()
                .border_color(rgb(p.line))
                .child(div().pl_2().pr_1().text_color(rgb(p.muted)).child(title));
            for (index, &(label, description, action)) in actions.iter().enumerate() {
                category = category.child(
                    self.button(
                        format!("tmux-{tab_id}-{key}-{index}"),
                        label,
                        cx,
                        move |s, w, cx| s.pane_action(tab_id, action, w, cx),
                    )
                    .native_tooltip(description)
                    .when(!enabled, |d| d.opacity(0.35).cursor_default()),
                );
            }
            toolbar = toolbar.child(category);
        }
        toolbar
    }
}
