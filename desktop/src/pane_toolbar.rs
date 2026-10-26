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
        let Some(target) = state.pane else { return };
        let operation = match action {
            PaneAction::Split(horizontal) => Operation::Split { target, horizontal },
            PaneAction::Focus(direction) => Operation::FocusPane { target, direction },
            PaneAction::Resize(direction) => Operation::ResizePane { target, direction },
            PaneAction::Close => Operation::KillPane { target },
        };
        self.group_focus(group, window, cx);
        if let Operation::KillPane { target } = &operation {
            self.dialog = Some(Dialog::Delete {
                title: tr!("tmux 패널 {0} 닫기", target),
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
                t("패널 분할"),
                &[
                    ("↔", t("tmux 패널 좌우 분할"), PaneAction::Split(true)),
                    ("↕", t("tmux 패널 상하 분할"), PaneAction::Split(false)),
                ][..],
            ),
            (
                "focus",
                t("이동"),
                &[
                    ("←", t("왼쪽 패널로 이동"), PaneAction::Focus('L')),
                    ("→", t("오른쪽 패널로 이동"), PaneAction::Focus('R')),
                    ("↑", t("위 패널로 이동"), PaneAction::Focus('U')),
                    ("↓", t("아래 패널로 이동"), PaneAction::Focus('D')),
                ][..],
            ),
            (
                "resize",
                t("크기 조절"),
                &[
                    ("←", t("패널 경계를 왼쪽으로 3셀"), PaneAction::Resize('L')),
                    (
                        "→",
                        t("패널 경계를 오른쪽으로 3셀"),
                        PaneAction::Resize('R'),
                    ),
                    ("↑", t("패널 경계를 위로 3셀"), PaneAction::Resize('U')),
                    ("↓", t("패널 경계를 아래로 3셀"), PaneAction::Resize('D')),
                ][..],
            ),
            (
                "close",
                t("닫기"),
                &[("×", t("현재 tmux 패널 닫기"), PaneAction::Close)][..],
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
