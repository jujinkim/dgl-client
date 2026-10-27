use super::*;
use workspace::{DIVIDER, DockEdge};

const TAB_BAR_HEIGHT: f32 = 48.;

#[derive(Clone, Copy)]
struct TabDropTarget {
    group: GroupId,
    edge: Option<DockEdge>,
}

#[derive(Clone)]
struct DragTab {
    id: TabId,
    label: String,
}
#[derive(Clone)]
struct DragHost {
    id: String,
    label: String,
}
pub(super) struct DragPreview(pub(super) String);
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .max_w(px(640.))
            .bg(rgb(0x30343b))
            .text_color(rgb(0xffffff))
            .child(self.0.clone())
    }
}
impl Desktop {
    fn toggle_menu(&mut self, id: String) {
        self.menu = if self.menu.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
    }
    fn toggle_expanded(&mut self, id: String) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
    }
    pub(super) fn group_focus(
        &mut self,
        group: GroupId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace.active = group;
        if let Some(id) = self.workspace.selected() {
            self.activate_tab(id);
        }
        if let Some(t) = self.active_tab() {
            window.focus(&t.terminal.focus_handle(cx));
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }
    fn tab_drop_target(&self, tab: TabId, position: Point<Pixels>) -> Option<TabDropTarget> {
        if self.dialog.is_some() || self.snackbar.is_some() {
            return None;
        }
        for group in self.workspace.layout.groups() {
            let Some(bounds) = self.group_bounds.get(&group) else {
                continue;
            };
            let content_top = bounds.top() + px(TAB_BAR_HEIGHT + 1.);
            if !bounds.contains(&position) || position.y < content_top {
                continue;
            }
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let x = f32::from(position.x - bounds.left()) / width;
            let y = f32::from(position.y - content_top) / f32::from(bounds.bottom() - content_top);
            let edge = [
                (DockEdge::Left, x),
                (DockEdge::Right, 1. - x),
                (DockEdge::Top, y),
                (DockEdge::Bottom, 1. - y),
            ]
            .into_iter()
            .filter(|(edge, distance)| {
                *distance <= 0.22
                    && self
                        .workspace
                        .can_split_tab(tab, group, *edge, width, height)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(edge, _)| edge);
            return Some(TabDropTarget { group, edge });
        }
        None
    }
    fn drop_tab(
        &mut self,
        tab: TabId,
        target: TabDropTarget,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edge) = target.edge {
            let Some(bounds) = self.group_bounds.get(&target.group) else {
                return;
            };
            if !self.workspace.split_tab(
                tab,
                target.group,
                edge,
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ) {
                return;
            }
            self.save_layout();
        } else if self.workspace.groups[&target.group].tabs.contains(&tab) {
            self.workspace.select(target.group, tab);
        } else {
            self.workspace.move_tab(tab, target.group, None);
        }
        self.group_focus(self.workspace.active, w, cx);
    }
    pub fn render_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        if !cx.has_active_drag() {
            self.dragging_tab = None;
            self.dragging_host = None;
            self.host_drop = None;
        }
        let preview = self.dragging_tab.and_then(|tab| {
            self.tab_drop_target(tab, window.mouse_position())
                .filter(|target| {
                    target.edge.is_some()
                        || !self.workspace.groups[&target.group].tabs.contains(&tab)
                })
        });
        let p = self.palette;
        let mut sidebar = div()
            .debug_selector(|| "session-sidebar".into())
            .w(px(272.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.panel))
            .border_r_1()
            .border_color(rgb(p.line))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .child(div().flex_1().child(t("DaGaLi · 탐색기")))
                    .child(self.button("add", "+", cx, |s, _, cx| s.edit(None, cx))),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child(t("호스트")),
            );
        let hosts: Vec<_> = self
            .host_order
            .ids()
            .iter()
            .filter_map(|id| self.local.hosts.iter().find(|h| &h.id == id).cloned())
            .collect();
        let mut explorer = div().id("explorer").flex_1().min_h_0().overflow_y_scroll();
        if hosts.is_empty() {
            explorer = explorer.child(
                div()
                    .p_3()
                    .text_sm()
                    .child(t("+ 버튼으로 SSH 호스트를 추가하세요.")),
            );
        }
        for host in hosts {
            let id = host.id.clone();
            let open = host.clone();
            let edit = host.clone();
            let delete = host.clone();
            let setup_host = host.clone();
            let menu_id = format!("host-menu-{id}");
            let toggle = menu_id.clone();
            let before = id.clone();
            let drop_id = id.clone();
            let next = self
                .host_order
                .ids()
                .iter()
                .position(|i| i == &id)
                .and_then(|i| self.host_order.ids().get(i + 1))
                .cloned();
            let drag_view = cx.entity();
            let before_line = self.host_drop.as_ref() == Some(&Some(id.clone()));
            let after_line = next.is_none() && self.host_drop == Some(None);
            explorer = explorer.child(
                div()
                    .id(SharedString::from(format!("host-row-{id}")))
                    .debug_selector({
                        let id = id.clone();
                        move || format!("host-row-{id}")
                    })
                    .relative()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(p.line))
                    .on_drag_move::<DragHost>(cx.listener(
                        move |s, e: &DragMoveEvent<DragHost>, _, cx| {
                            if e.bounds.contains(&e.event.position) {
                                s.host_drop = Some(if e.event.position.y < e.bounds.center().y {
                                    Some(before.clone())
                                } else {
                                    next.clone()
                                });
                                cx.notify();
                                cx.stop_propagation();
                            }
                        },
                    ))
                    .on_drop(cx.listener(move |s, dragged: &DragHost, _, cx| {
                        if s.dragging_host.as_ref() == Some(&dragged.id)
                            && let Some(before) = s.host_drop.take()
                            && s.local.hosts.iter().any(|h| h.id == drop_id)
                            && s.host_order.drop_before(&dragged.id, before.as_deref())
                        {
                            s.save_host_order();
                        }
                        s.dragging_host = None;
                        cx.notify();
                        cx.stop_propagation();
                    }))
                    .when(before_line || after_line, |d| {
                        d.child(
                            div()
                                .debug_selector(|| "host-drop-indicator".into())
                                .absolute()
                                .left_0()
                                .right_0()
                                .h(px(2.))
                                .bg(rgb(p.accent))
                                .when(before_line, |d| d.top_0())
                                .when(after_line, |d| d.bottom_0()),
                        )
                    })
                    .when(self.active_tab().is_some_and(|t| t.host.id == id), |d| {
                        d.bg(rgb(p.selection))
                    })
                    .child(
                        self.button(
                            format!("host-{id}"),
                            format!("{}{}", if host.favorite { "★ " } else { "" }, host.alias),
                            cx,
                            move |s, w, cx| {
                                s.open(open.clone(), cx);
                                s.group_focus(s.workspace.active, w, cx);
                            },
                        )
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .cursor_move()
                        .on_drag(
                            DragHost {
                                id: id.clone(),
                                label: host.alias.clone(),
                            },
                            move |host, _, _, cx| {
                                drag_view.update(cx, |s, cx| {
                                    s.dragging_host = Some(host.id.clone());
                                    s.host_drop = None;
                                    s.menu = None;
                                    cx.notify();
                                });
                                cx.new(|_| DragPreview(host.label.clone()))
                            },
                        ),
                    )
                    .child(self.button(menu_id.clone(), "…", cx, move |s, _, _| {
                        s.toggle_menu(toggle.clone())
                    })),
            );
            if self.menu.as_ref() == Some(&menu_id) {
                explorer = explorer.child(
                    div()
                        .pl_3()
                        .flex()
                        .flex_wrap()
                        .text_sm()
                        .child(self.host_connection_controls(&host.id, cx))
                        .child(self.button(
                            format!("edit-{id}"),
                            t("편집"),
                            cx,
                            move |s, _, cx| {
                                s.menu = None;
                                s.edit(Some(edit.clone()), cx);
                            },
                        ))
                        .child(self.button(
                            format!("agent-setup-{id}"),
                            t("에이전트 상태 연동"),
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                if let Some(tab) = s
                                    .active_tab()
                                    .filter(|t| t.host.id == setup_host.id)
                                    .or_else(|| {
                                        s.tabs.iter().rev().find(|t| t.host.id == setup_host.id)
                                    })
                                {
                                    s.dialog = Some(Dialog::Agents(
                                        tab.connection.clone(),
                                        setup_host.alias.clone(),
                                        None,
                                    ));
                                } else {
                                    s.snackbar.show_snackbar_with_duration(
                                        t("호스트에 먼저 연결하세요."),
                                        SnackbarDuration::Short,
                                    );
                                }
                            },
                        ))
                        .child(self.button(
                            format!("delete-{id}"),
                            t("삭제…"),
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                s.dialog = Some(Dialog::Delete {
                                    title: tr!("호스트 {0} 삭제", delete.alias),
                                    operation: DeleteOp::Host(delete.clone()),
                                });
                            },
                        )),
                );
            }
        }
        explorer = explorer.child(self.render_tree(cx));
        sidebar = sidebar.child(explorer);
        let unread = self.inbox.recent.iter().filter(|n| !n.read).count();
        let layout = self.workspace.layout.clone();
        let workspace = self.render_layout(layout, vec![], preview, cx);
        let status = self
            .active_tab()
            .map(|tab| {
                let state = tab.connection.state();
                [
                    hierarchy::display_text(&tab.host.alias),
                    hierarchy::phase_label(&state.phase).into(),
                    hierarchy::target_label(
                        &state.remote,
                        state.session.as_deref(),
                        state.pane.as_deref(),
                    ),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
                    + &state.error.map(|e| format!(" · {e}")).unwrap_or_default()
            })
            .unwrap_or_else(|| t("빈 그룹 · 왼쪽 호스트를 선택하여 연결").into());
        let status_host = self
            .active_tab()
            .map(|tab| hierarchy::display_text(&tab.host.alias));
        let status_detail = status_host
            .as_ref()
            .and_then(|host| status.strip_prefix(&format!("{host} · ")))
            .unwrap_or(&status)
            .to_string();
        let mut shortcuts = div()
            .id("shortcut-bar")
            .debug_selector(|| "shortcut-bar".into())
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            .h_full()
            .overflow_x_scroll()
            .text_xs()
            .bg(rgb(p.panel));
        if let Some(tab) = self.active_tab() {
            let terminal = tab.terminal.clone();
            let mut shown = Vec::new();
            for &key in &self.local.preferences.terminal_shortcuts {
                if shown.contains(&key) {
                    continue;
                }
                shown.push(key);
                let label = key.label();
                let t = terminal.clone();
                shortcuts = shortcuts.child(
                    self.input_button(format!("key-{label}"), label, cx, move |_, w, cx| {
                        t.update(cx, |t, cx| t.shortcut(key, cx));
                        w.focus(&t.focus_handle(cx));
                    })
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(rgb(if matches!(key, dgl_core::input::TerminalShortcut::Modifier(m) if terminal.read(cx).modifiers(cx).contains(m)) { p.selection } else { p.panel }))
                    .border_color(rgb(if matches!(key, dgl_core::input::TerminalShortcut::Modifier(m) if terminal.read(cx).modifiers(cx).contains(m)) { p.accent } else { p.line })),
                );
            }
        }
        div()
            .id("desktop")
            .size_full()
            .relative()
            .font_family(platform::UI_FONT)
            .text_size(px(13.))
            .bg(rgb(p.bg))
            .text_color(rgb(p.fg))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape"
                    && (s.dragging_tab.is_some() || s.dragging_host.is_some())
                    && cx.has_active_drag()
                {
                    s.dragging_tab = None;
                    s.dragging_host = None;
                    s.host_drop = None;
                    cx.stop_active_drag(w);
                    cx.stop_propagation();
                    w.prevent_default();
                    cx.notify();
                    return;
                }
                if s.settings.dragging.is_some() && e.keystroke.key == "escape" {
                    s.settings.dragging = None;
                    cx.stop_active_drag(w);
                    cx.stop_propagation();
                    w.prevent_default();
                    cx.notify();
                    return;
                }
                if s.launcher.opened.is_some() && e.keystroke.key == "escape" {
                    s.launcher_escape(w, cx);
                    return;
                }
                if s.dialog.is_some() {
                    match e.keystroke.key.as_str() {
                        "tab" => {
                            s.modal.cycle(e.keystroke.modifiers.shift, w);
                            cx.stop_propagation();
                            w.prevent_default();
                        }
                        "escape" => {
                            s.dismiss_dialog(cx);
                            cx.stop_propagation();
                            w.prevent_default();
                        }
                        "enter" => {
                            if matches!(s.dialog, Some(Dialog::Settings))
                                && s.accent_input.focus_handle(cx).is_focused(w)
                                && !s.accent_input.read(cx).is_composing()
                            {
                                s.apply_accent(cx);
                                cx.stop_propagation();
                                w.prevent_default();
                            }
                            if let Some(Dialog::Prompt { input, .. }) = &s.dialog
                                && input.focus_handle(cx).is_focused(w)
                                && !input.read(cx).is_composing()
                            {
                                s.accept_name(cx);
                                cx.stop_propagation();
                                w.prevent_default();
                            }
                        }
                        _ => {}
                    }
                    cx.notify();
                    return;
                }
                if !platform::app_shortcut(&e.keystroke.modifiers) {
                    return;
                }
                match e.keystroke.key.as_str() {
                    "n" if cfg!(target_os = "macos") && e.keystroke.modifiers.shift => {
                        s.prompt(t("세션 만들기"), PromptOp::Session, cx)
                    }
                    "s" if cfg!(target_os = "windows") => {
                        s.prompt(t("세션 만들기"), PromptOp::Session, cx)
                    }
                    "n" => s.edit(None, cx),
                    "w" => {
                        if let Some(id) = s.workspace.selected() {
                            s.close_tab(id);
                        }
                    }
                    "," => s.dialog = Some(Dialog::Settings),
                    "p" if cfg!(target_os = "windows") => s.dialog = Some(Dialog::Settings),
                    "r" => s.refresh(),
                    key => {
                        let key = if cfg!(target_os = "windows") {
                            key.strip_prefix('f').unwrap_or("")
                        } else {
                            key
                        };
                        let Some(id) = key
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .and_then(|i| {
                                s.workspace.groups[&s.workspace.active].tabs.get(i).copied()
                            })
                        else {
                            return;
                        };
                        s.workspace.select(s.workspace.active, id);
                        s.group_focus(s.workspace.active, w, cx);
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_drag_move::<DragTab>(cx.listener(|_, _, _, cx| {
                cx.notify();
                cx.stop_propagation();
            }))
            .on_drag_move::<DragHost>(cx.listener(|s, _, _, cx| {
                s.host_drop = None;
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if s.dialog.is_some() {
                    return;
                }
                if let Some(path) = s.resizing.clone()
                    && let Some(bounds) = s.split_bounds.get(&path).copied()
                    && let Some(Layout::Split {
                        axis,
                        ratio,
                        first,
                        second,
                    }) = s.workspace.layout.at_mut(&path)
                {
                    let (a, b, length, position) = match axis {
                        Axis::Horizontal => (
                            first.minimum().0,
                            second.minimum().0,
                            f32::from(bounds.size.width) - DIVIDER,
                            f32::from(e.position.x - bounds.left()),
                        ),
                        Axis::Vertical => (
                            first.minimum().1,
                            second.minimum().1,
                            f32::from(bounds.size.height) - DIVIDER,
                            f32::from(e.position.y - bounds.top()),
                        ),
                    };
                    if length >= a + b {
                        *ratio = (position / length)
                            .clamp((a / length).max(0.05), (1. - b / length).min(0.95));
                        cx.notify();
                    }
                }
            }))
            .capture_any_mouse_up(cx.listener(|s, e: &MouseUpEvent, w, cx| {
                if let Some(tab) = s.dragging_tab.take() {
                    if cx.has_active_drag()
                        && e.button == MouseButton::Left
                        && let Some(target) = s.tab_drop_target(tab, e.position)
                    {
                        cx.stop_active_drag(w);
                        s.drop_tab(tab, target, w, cx);
                        cx.stop_propagation();
                    }
                    cx.notify();
                }
                if s.resizing.take().is_some() {
                    s.save_layout();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        div().flex_1().min_h_0().flex().child(sidebar).child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().flex_1().min_h_0().child(workspace))
                                .child(
                                    div()
                                        .debug_selector(|| "terminal-input-tools".into())
                                        .flex()
                                        .items_center()
                                        .h(px(40.))
                                        .px_1()
                                        .gap_1()
                                        .flex_shrink_0()
                                        .bg(rgb(p.panel))
                                        .child(
                                            self.input_button(
                                                "open-composer",
                                                t("텍스트 작성"),
                                                cx,
                                                |s, _, cx| s.open_composer(cx),
                                            )
                                            .h(px(32.))
                                            .flex()
                                            .items_center()
                                            .rounded_md()
                                            .when(self.active_tab().is_none(), |d| {
                                                d.opacity(0.35).cursor_default()
                                            }),
                                        )
                                        .child(shortcuts)
                                        .child(self.launcher_button(cx)),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "status-bar".into())
                            .h(px(26.))
                            .flex_shrink_0()
                            .px_3()
                            .flex()
                            .items_center()
                            .bg(rgb(p.panel))
                            .border_t_1()
                            .border_color(rgb(p.line))
                            .text_xs()
                            .child(
                                div()
                                    .id("connection-status")
                                    .debug_selector(|| "connection-status".into())
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .native_tooltip(status.clone())
                                    .when_some(status_host, |d, host| {
                                        d.child(self.info_badge(host).max_w(px(120.)))
                                    })
                                    .child(
                                        div().flex_1().min_w_0().truncate().child(status_detail),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "status-actions".into())
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    // Preserve the prior focus until the modal records it.
                                    .on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())
                                    .child(
                                        self.button_base("agent-notifications", cx, |s, _, _| {
                                            s.dialog = if matches!(
                                                s.dialog,
                                                Some(Dialog::Notifications)
                                            ) {
                                                None
                                            } else {
                                                Some(Dialog::Notifications)
                                            };
                                        })
                                        .child(
                                            canvas(
                                                {
                                                    let anchor = self.notification_anchor.clone();
                                                    move |bounds, _, _| {
                                                        // The canvas is inset by the button's 1px border.
                                                        *anchor.borrow_mut() = bounds.dilate(px(1.))
                                                    }
                                                },
                                                |_, _, _, _| {},
                                            )
                                            .absolute()
                                            .inset_0(),
                                        )
                                        .w(px(56.))
                                        .h(px(24.))
                                        .p_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap_1()
                                        .native_tooltip(t("알림"))
                                        .child(
                                            div()
                                                .debug_selector(|| "notification-icon".into())
                                                .child("🔔"),
                                        )
                                        .when(
                                            unread > 0,
                                            |d| {
                                                d.child(
                                                    div()
                                                        .debug_selector(|| {
                                                            "unread-notifications".into()
                                                        })
                                                        .w(px(24.))
                                                        .flex_shrink_0()
                                                        .rounded_full()
                                                        .bg(rgb(p.accent_fill))
                                                        .text_color(rgb(p.on_accent))
                                                        .text_size(px(10.))
                                                        .line_height(px(14.))
                                                        .text_center()
                                                        .child(if unread > 99 {
                                                            "99+".into()
                                                        } else {
                                                            unread.to_string()
                                                        }),
                                                )
                                            },
                                        ),
                                    )
                                    .child(
                                        self.button("settings", "⚙️", cx, |s, _, _| {
                                            s.dialog = Some(Dialog::Settings);
                                        })
                                        .w(px(28.))
                                        .h(px(24.))
                                        .p_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .native_tooltip(t("설정")),
                                    ),
                            ),
                    ),
            )
            .children(self.render_launcher(cx))
    }
    fn render_layout(
        &mut self,
        layout: Layout,
        path: Vec<bool>,
        preview: Option<TabDropTarget>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match layout {
            Layout::Group { id } => self
                .render_group(id, preview.filter(|p| p.group == id), cx)
                .into_any_element(),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let p = self.palette;
                let horizontal = axis == Axis::Horizontal;
                let mut left = path.clone();
                left.push(false);
                let mut right = path.clone();
                right.push(true);
                let first = self.render_layout(*first, left, preview, cx);
                let second = self.render_layout(*second, right, preview, cx);
                let entity = cx.entity();
                let measure = path.clone();
                let drag = path.clone();
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .flex()
                    .when(!horizontal, |d| d.flex_col())
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                entity.update(cx, |s, _| {
                                    s.split_bounds.insert(measure.clone(), bounds);
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .flex_basis(px(0.))
                            .map(|mut d| {
                                d.style().flex_grow = Some(ratio);
                                d
                            })
                            .when(horizontal, |d| d.h_full())
                            .when(!horizontal, |d| d.w_full())
                            .child(first),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("divider-{path:?}")))
                            .flex_shrink_0()
                            .bg(rgb(p.line))
                            .when(horizontal, |d| {
                                d.w(px(DIVIDER)).h_full().cursor_col_resize()
                            })
                            .when(!horizontal, |d| {
                                d.h(px(DIVIDER)).w_full().cursor_row_resize()
                            })
                            .hover(move |d| d.bg(rgb(p.accent)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |s, _, _, cx| {
                                    s.resizing = Some(drag.clone());
                                    cx.stop_propagation();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .flex_basis(px(0.))
                            .map(|mut d| {
                                d.style().flex_grow = Some(1. - ratio);
                                d
                            })
                            .when(horizontal, |d| d.h_full())
                            .when(!horizontal, |d| d.w_full())
                            .child(second),
                    )
                    .into_any_element()
            }
        }
    }
    fn render_group(
        &mut self,
        id: GroupId,
        preview: Option<TabDropTarget>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette;
        let group = &self.workspace.groups[&id];
        let selected = group.selected;
        let ids = group.tabs.clone();
        let entity = cx.entity();
        let mut header = div()
            .id(SharedString::from(format!("group-tabs-{id}")))
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .flex()
            .items_center()
            .on_drop(cx.listener(move |s, tab: &DragTab, w, cx| {
                s.workspace.move_tab(tab.id, id, None);
                s.group_focus(id, w, cx);
            }));
        for tab_id in ids.clone() {
            let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
                continue;
            };
            let label = tab.label();
            let menu = format!("tab-menu-{tab_id}");
            let toggle = menu.clone();
            let drag_view = cx.entity();
            header = header.child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(if selected == Some(tab_id) {
                        rgb(p.accent)
                    } else {
                        rgb(p.line)
                    })
                    .when(selected == Some(tab_id), |d| d.bg(rgb(p.selection)))
                    .child(
                        self.button_base(format!("tab-{tab_id}"), cx, move |s, w, cx| {
                            s.workspace.select(id, tab_id);
                            s.group_focus(id, w, cx);
                        })
                        .h_full()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .items_start()
                        .min_w_0()
                        .max_w(px(320.))
                        .child(
                            div()
                                .debug_selector(move || format!("tab-title-{tab_id}"))
                                .w_full()
                                .truncate()
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .child(hierarchy::tab_title(&tab.title())),
                        )
                        .child(
                            self.info_badge(hierarchy::display_text(&tab.host.alias))
                                .debug_selector(move || format!("tab-host-{tab_id}"))
                                .max_w(px(120.)),
                        )
                        .native_tooltip(label.clone())
                        .cursor_move()
                        .on_drag(DragTab { id: tab_id, label }, move |tab, _, _, cx| {
                            drag_view.update(cx, |s, cx| {
                                s.dragging_tab = Some(tab.id);
                                s.menu = None;
                                cx.notify();
                            });
                            cx.new(|_| DragPreview(tab.label.clone()))
                        })
                        .on_drop(cx.listener(
                            move |s, tab: &DragTab, w, cx| {
                                s.workspace.move_tab(tab.id, id, Some(tab_id));
                                s.group_focus(id, w, cx);
                                cx.stop_propagation();
                            },
                        )),
                    )
                    .child(self.button(menu.clone(), "…", cx, move |s, _, _| {
                        s.toggle_menu(toggle.clone())
                    }))
                    .child(
                        self.button(format!("close-{tab_id}"), "×", cx, move |s, _, _| {
                            s.close_tab(tab_id)
                        }),
                    ),
            );
        }
        let bounds = self.group_bounds.get(&id).copied().unwrap_or_default();
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let mut tools = div()
            .id(SharedString::from(format!("group-tools-{id}")))
            .debug_selector(move || format!("group-tools-{id}"))
            .flex()
            .items_center()
            .flex_shrink_0()
            .h_full();
        if self.workspace.groups.len() > 1 {
            tools = tools.child(
                self.button(format!("merge-{id}"), "🔗", cx, move |s, w, cx| {
                    s.workspace.merge(id);
                    s.save_layout();
                    s.group_focus(s.workspace.active, w, cx);
                })
                .native_tooltip(t("그룹 합치기")),
            );
        }
        for (axis, label, description) in [
            (Axis::Horizontal, "↔️", t("화면 좌우 분할")),
            (Axis::Vertical, "↕️", t("화면 상하 분할")),
        ] {
            let enabled = self.workspace.can_split(id, axis, width, height);
            tools = tools.child(
                self.button(
                    format!("split-{id}-{axis:?}"),
                    label,
                    cx,
                    move |s, w, cx| {
                        if s.workspace.split(id, axis, width, height) {
                            s.save_layout();
                            s.group_focus(s.workspace.active, w, cx);
                        }
                    },
                )
                .native_tooltip(description)
                .when(!enabled, |d| d.opacity(0.35).cursor_default()),
            );
        }
        let mut body = div()
            .id(SharedString::from(format!("group-{id}")))
            .size_full()
            .min_h_0()
            .min_w_0()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(if self.workspace.active == id {
                p.accent
            } else {
                p.line
            }))
            .capture_any_mouse_down(cx.listener(move |s, _, _, cx| {
                if s.dialog.is_none() {
                    s.workspace.active = id;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |s, cx| {
                            if s.group_bounds.insert(id, bounds) != Some(bounds) {
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .debug_selector(move || format!("group-header-{id}"))
                    .flex()
                    .items_center()
                    .h(px(TAB_BAR_HEIGHT))
                    .flex_shrink_0()
                    .bg(rgb(p.panel))
                    .child(header)
                    .child(tools),
            );
        for tab_id in ids {
            if self.menu.as_deref() == Some(&format!("tab-menu-{tab_id}")) {
                let mut menu = div().flex().flex_wrap().text_xs();
                for (delta, label) in [(-1i32, t("왼쪽으로")), (1, t("오른쪽으로"))] {
                    menu = menu.child(self.button(
                        format!("reorder-{tab_id}-{delta}"),
                        label,
                        cx,
                        move |s, _, _| {
                            let tabs = &s.workspace.groups[&id].tabs;
                            let at = tabs.iter().position(|t| *t == tab_id).unwrap();
                            let before = if delta < 0 {
                                tabs.get(at.saturating_sub(1)).copied()
                            } else {
                                tabs.get(at + 2).copied()
                            };
                            s.workspace.move_tab(tab_id, id, before);
                            s.menu = None;
                        },
                    ));
                }
                for dest in self
                    .workspace
                    .layout
                    .groups()
                    .into_iter()
                    .filter(|g| *g != id)
                {
                    menu = menu.child(self.button(
                        format!("move-{tab_id}-{dest}"),
                        tr!("그룹 {0}로 이동", dest),
                        cx,
                        move |s, w, cx| {
                            s.workspace.move_tab(tab_id, dest, None);
                            s.menu = None;
                            s.group_focus(dest, w, cx);
                        },
                    ));
                }
                body = body.child(menu);
            }
        }
        if let Some(tab) = selected.and_then(|tid| self.tabs.iter().find(|t| t.id == tid)) {
            let terminal = tab.terminal.clone();
            let state = tab.connection.state();
            body = body.child(self.render_pane_toolbar(tab, cx));
            if state.can_retry_now() {
                let retry_connection = tab.connection.clone();
                let stop_connection = tab.connection.clone();
                let mut retry_controls = div().flex().flex_wrap().text_xs().child(self.button(
                    format!("retry-now-{id}"),
                    t("새로고침 재시도"),
                    cx,
                    move |s, _, _| {
                        s.submit(&retry_connection, Action::RetryNow);
                    },
                ));
                if state.can_stop_retrying() {
                    retry_controls = retry_controls.child(self.button(
                        format!("stop-retry-{id}"),
                        t("재시도 정지"),
                        cx,
                        move |s, _, _| {
                            s.submit(&stop_connection, Action::StopRetrying);
                        },
                    ));
                }
                body = body.child(
                    div()
                        .p_2()
                        .text_xs()
                        .child(
                            state
                                .retry_message()
                                .unwrap_or_else(|| t("SSH 연결 중…").into()),
                        )
                        .child(retry_controls),
                );
            }
            if state.phase == Phase::TrustRequired {
                let host = tab.host.clone();
                let fingerprint = state.fingerprint.unwrap_or_default();
                body = body
                    .child(div().p_3().child(tr!(
                        "처음 연결하는 서버 · {0}\n{1}\n관리자에게 받은 SHA-256 지문과 비교하세요.",
                        host.endpoint(),
                        fingerprint
                    )))
                    .child(self.button(
                        format!("trust-{id}"),
                        t("지문 확인 후 신뢰"),
                        cx,
                        move |s, _, cx| {
                            let endpoint = host.endpoint();
                            if let Err(e) = storage::write(
                                &storage::trust_ref(&endpoint),
                                &Credential {
                                    password: fingerprint.clone(),
                                    private_key: String::new(),
                                    passphrase: String::new(),
                                },
                            ) {
                                s.snackbar.show_snackbar_with_duration(
                                    tr!("호스트키 저장 실패: {0}", e),
                                    SnackbarDuration::Long,
                                );
                                return;
                            }
                            s.local.trust.insert(endpoint, fingerprint.clone());
                            if s.save() {
                                s.workspace.active = id;
                                s.reconnect(cx);
                            }
                        },
                    ));
            }
            body = body.child(div().flex_1().min_h_0().child(terminal));
        } else {
            body = body.child(
                div()
                    .id(SharedString::from(format!("empty-{id}")))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .text_color(rgb(p.muted))
                    .child(tr!("그룹 {0}", id))
                    .child(t("호스트를 연결하거나 탭을 끌어오세요."))
                    .on_click(cx.listener(move |s, _, w, cx| s.group_focus(id, w, cx)))
                    .on_drop(cx.listener(move |s, tab: &DragTab, w, cx| {
                        s.workspace.move_tab(tab.id, id, None);
                        s.group_focus(id, w, cx);
                    })),
            );
        }
        if let Some(target) = preview {
            let mut left = 0.;
            let mut top = 0.;
            let mut preview_width = width - 2.;
            let mut preview_height = height - 2.;
            let label = match target.edge {
                Some(DockEdge::Left) => t("왼쪽으로 분할"),
                Some(DockEdge::Right) => t("오른쪽으로 분할"),
                Some(DockEdge::Top) => t("위로 분할"),
                Some(DockEdge::Bottom) => t("아래로 분할"),
                None => t("이 그룹으로 이동"),
            };
            if let Some(edge) = target.edge {
                match edge.axis() {
                    Axis::Horizontal => {
                        preview_width = (preview_width - DIVIDER) / 2.;
                        if !edge.leading() {
                            left = preview_width + DIVIDER;
                        }
                    }
                    Axis::Vertical => {
                        preview_height = (preview_height - DIVIDER) / 2.;
                        if !edge.leading() {
                            top = preview_height + DIVIDER;
                        }
                    }
                }
            }
            body = body.child(
                div()
                    .id(SharedString::from(format!("tab-drop-preview-{id}")))
                    .debug_selector(move || format!("tab-drop-preview-{id}"))
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(preview_width))
                    .h(px(preview_height))
                    .bg(Hsla::from(rgb(p.accent)).opacity(0.22))
                    .border_2()
                    .border_color(rgb(p.accent))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().px_3().py_2().bg(rgb(p.panel)).child(label)),
            );
        }
        body
    }
    fn host_connection_controls(&self, host: &str, cx: &mut Context<Self>) -> Div {
        let mut controls = div().flex().text_xs();
        if let Some(tab) = self.active_tab().filter(|t| t.host.id == host) {
            let tid = tab.id;
            let handle = tab.connection.state().handle;
            let host_id = host.to_owned();
            let retry_host = host_id.clone();
            if !tab.connection.state().can_retry_now() {
                controls = controls.child(self.button(
                    "host-reconnect",
                    t("재접속"),
                    cx,
                    move |s, _, cx| {
                        if s.matches_selected_connection(&retry_host, tid, handle)
                            && s.active_tab()
                                .is_some_and(|t| !t.connection.state().can_retry_now())
                        {
                            s.menu = None;
                            s.reconnect(cx);
                        }
                    },
                ));
            }
            controls = controls.child(self.button(
                "host-disconnect",
                t("연결 종료"),
                cx,
                move |s, _, _| {
                    if s.matches_selected_connection(&host_id, tid, handle) {
                        s.menu = None;
                        s.close_tab(tid);
                    }
                },
            ));
        }
        controls
    }
    pub(super) fn matches_selected_connection(&self, host: &str, tid: TabId, handle: u64) -> bool {
        self.active_tab().is_some_and(|t| {
            t.id == tid && t.host.id == host && t.connection.state().handle == handle
        })
    }
    fn tree_button(
        &self,
        id: impl Into<SharedString>,
        label: hierarchy::Label,
        pane: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let full = label.full_text();
        let id = id.into();
        let title_id = format!("{id}-title");
        let detail_id = format!("{id}-detail");
        self.button_base(id, cx, action)
            .flex_1()
            .flex_shrink()
            .min_w_0()
            .flex()
            .flex_col()
            .items_start()
            .py_2()
            .native_tooltip(full)
            .child(
                div()
                    .id(ElementId::Name(title_id.clone().into()))
                    .debug_selector(move || title_id)
                    .w_full()
                    .text_size(px(if pane { 14. } else { 16. }))
                    .line_height(px(if pane { 19. } else { 22. }))
                    .font_weight(if pane {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::BOLD
                    })
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(label.title),
            )
            .child(
                div()
                    .id(ElementId::Name(detail_id.clone().into()))
                    .debug_selector(move || detail_id)
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(self.palette.muted))
                    .when(!label.subtitle.is_empty(), |d| {
                        d.child(div().flex_1().min_w_0().truncate().child(label.subtitle))
                    })
                    .child(
                        self.info_badge(label.detail)
                            .flex_shrink()
                            .min_w_0()
                            .max_w(px(124.)),
                    ),
            )
    }
    fn info_badge(&self, text: impl Into<SharedString>) -> Div {
        div()
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(rgb(self.palette.line))
            .text_color(rgb(self.palette.muted))
            .text_size(px(10.))
            .line_height(px(14.))
            .truncate()
            .child(text.into())
    }
    fn render_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.active_tab() else {
            return div().into_any_element();
        };
        self.render_tree_state(tab.connection.state(), cx)
    }
    pub(super) fn render_tree_state(
        &mut self,
        state: dgl_core::connection::State,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette;
        let mut tree = div()
            .debug_selector(|| "session-tree".into())
            .mt_3()
            .border_t_1()
            .border_color(rgb(p.line));
        let Some(tab) = self.active_tab() else {
            return tree.into_any_element();
        };
        let connection = tab.connection.clone();
        let tid = tab.id;
        let refreshing = tab.refresh.is_some();
        let browse = matches!(
            state.phase,
            Phase::Selecting | Phase::Ready | Phase::MissingSession
        );
        tree = tree.child(
            div()
                .flex()
                .items_center()
                .px_2()
                .child(div().flex_1().child(t("세션")))
                .child(
                    self.button("new-session", "+", cx, move |s, _, cx| {
                        if browse {
                            s.prompt(t("세션 만들기"), PromptOp::Session, cx);
                        }
                    })
                    .when(!browse, |d| d.opacity(0.35)),
                )
                .child(
                    self.button("refresh-sessions", "↻", cx, move |s, _, _| {
                        if browse {
                            s.refresh();
                        }
                    })
                    .native_tooltip(t("세션 새로고침"))
                    .when(refreshing || !browse, |d| d.opacity(0.35)),
                ),
        );
        // Keep cached rows at the same coordinates while the connection recovers.
        // The status bar already displays the phase; only an empty tree needs a hint.
        if !browse && state.remote.sessions.is_empty() {
            tree = tree.child(
                div()
                    .debug_selector(|| "session-list-waiting".into())
                    .px_3()
                    .text_xs()
                    .child(tr!("{0} · 목록 대기", hierarchy::phase_label(&state.phase))),
            );
        }
        for row in hierarchy::rows(&state.remote, self.local.preferences.show_all_hierarchy) {
            tree = tree.child(self.render_hierarchy_row(&row, 0, tid, &connection, &state, cx));
        }
        tree.into_any_element()
    }
    fn render_hierarchy_row(
        &self,
        row: &hierarchy::Row,
        depth: usize,
        tid: TabId,
        connection: &Arc<Connection>,
        state: &dgl_core::connection::State,
        cx: &mut Context<Self>,
    ) -> Div {
        let browse = matches!(
            state.phase,
            Phase::Selecting | Phase::Ready | Phase::MissingSession
        );
        let p = self.palette;
        let expand = format!("{tid}/{}", row.key);
        let toggle = expand.clone();
        let menu_id = format!("{expand}/menu");
        let menu = menu_id.clone();
        let c = connection.clone();
        let generation = state.generation.connection;
        let target = row.clone();
        let session_row = row.kind == hierarchy::Kind::Session;
        let id = match row.kind {
            hierarchy::Kind::Session => format!("session-{}", row.session),
            hierarchy::Kind::Window => {
                format!("window-{}-{}", row.session, row.window.as_deref().unwrap())
            }
            hierarchy::Kind::Pane => format!(
                "pane-{}-{}-{}",
                row.session,
                row.window.as_deref().unwrap(),
                row.pane.as_deref().unwrap()
            ),
        };
        let mut line = div()
            .id(SharedString::from(format!("row-{}", row.key)))
            .relative()
            .flex()
            .items_center()
            .pl(px(depth as f32 * 20.))
            .when(row.kind == hierarchy::Kind::Session, |d| {
                d.bg(rgb(p.session_header))
            })
            .when(row.selected(state), |d| d.bg(rgb(p.selection)));
        for guide in 0..depth {
            line = line.child(
                div()
                    .absolute()
                    .left(px(guide as f32 * 20. + 4.))
                    .top_0()
                    .bottom_0()
                    .w(px(1.))
                    .bg(rgb(p.line)),
            );
        }
        let expand_button = if row.children.is_empty() {
            None
        } else {
            Some(
                self.button(
                    format!("expand-{expand}"),
                    if self.expanded.contains(&expand) {
                        "▾"
                    } else {
                        "▸"
                    },
                    cx,
                    move |s, _, _| s.toggle_expanded(toggle.clone()),
                )
                .w(px(28.))
                .px_0(),
            )
        };
        line = line
            .child(self.tree_button(
                id,
                row.label.clone(),
                row.kind == hierarchy::Kind::Pane,
                cx,
                move |s, w, cx| {
                    if s.select_session_tab(
                        &c,
                        generation,
                        &target.session,
                        if session_row {
                            None
                        } else {
                            target.window.clone()
                        },
                        if session_row {
                            None
                        } else {
                            target.pane.clone()
                        },
                        cx,
                    )
                    .is_some()
                    {
                        s.group_focus(s.workspace.active, w, cx);
                    }
                },
            ))
            .when_some(expand_button, |d, button| d.child(button))
            .child(
                self.button(menu_id.clone(), "…", cx, move |s, _, _| {
                    s.toggle_menu(menu.clone())
                })
                .w(px(28.))
                .px_0(),
            );
        if let Some(pid) = &row.window
            && let Some(tab) = self.active_tab()
        {
            line = line.child(self.agent_badge(connection.clone(), &tab.host, pid, state, cx));
        }
        let mut body = div().flex().flex_col().child(line);
        if browse && self.menu.as_ref() == Some(&menu_id) {
            body = body.child(self.hierarchy_menu(row, depth, connection, cx));
        }
        if self.expanded.contains(&expand) {
            for child in &row.children {
                body = body.child(self.render_hierarchy_row(
                    child,
                    depth + 1,
                    tid,
                    connection,
                    state,
                    cx,
                ));
            }
        }
        body
    }
    fn hierarchy_menu(
        &self,
        row: &hierarchy::Row,
        depth: usize,
        connection: &Arc<Connection>,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut menu = div()
            .pl(px(depth as f32 * 20.))
            .flex()
            .flex_wrap()
            .text_xs();
        if row.kind == hierarchy::Kind::Session {
            let add = row.session.clone();
            menu = menu.child(self.button(
                format!("new-window-{add}"),
                t("창 만들기"),
                cx,
                move |s, _, cx| {
                    s.menu = None;
                    s.prompt(t("창 만들기"), PromptOp::Window(add.clone()), cx);
                },
            ));
            let target = row.session.clone();
            menu = menu.child(self.button(
                format!("rename-session-{target}"),
                t("세션 이름 변경"),
                cx,
                move |s, _, cx| {
                    s.menu = None;
                    s.prompt(
                        t("세션 이름 변경"),
                        PromptOp::RenameSession(target.clone()),
                        cx,
                    );
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-session-{}", row.session),
                t("세션 삭제…"),
                row.label.title.clone(),
                Operation::KillSession {
                    target: row.session.clone(),
                },
                connection,
                cx,
            ));
        }
        if row.kind != hierarchy::Kind::Pane
            && let Some(window) = &row.window
        {
            let target = window.clone();
            menu = menu.child(self.button(
                format!("rename-{target}"),
                t("창 이름 변경"),
                cx,
                move |s, _, cx| {
                    s.menu = None;
                    s.prompt(
                        t("창 이름 변경"),
                        PromptOp::RenameWindow(target.clone()),
                        cx,
                    );
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-{window}"),
                t("창 삭제…"),
                t("창").into(),
                Operation::KillWindow {
                    target: window.clone(),
                },
                connection,
                cx,
            ));
        }
        let active_pane = row.window.as_ref().and_then(|id| {
            connection
                .state()
                .remote
                .panes
                .iter()
                .find(|p| &p.window == id && p.active)
                .map(|p| p.id.clone())
        });
        if let Some(pane) = &active_pane {
            let generation = connection.state().generation.connection;
            for (horizontal, label) in [
                (true, t("패널 좌우 나누기")),
                (false, t("패널 상하 나누기")),
            ] {
                let c = connection.clone();
                let mut target = row.clone();
                target.pane = Some(pane.clone());
                menu = menu.child(self.button(
                    format!("split-{pane}-{horizontal}"),
                    label,
                    cx,
                    move |s, w, cx| {
                        s.menu = None;
                        if let Some(id) = s.select_session_tab(
                            &c,
                            generation,
                            &target.session,
                            target.window.clone(),
                            target.pane.clone(),
                            cx,
                        ) {
                            let operation = Operation::Split {
                                target: target.pane.clone().unwrap(),
                                horizontal,
                            };
                            let tab = s.tabs.iter_mut().find(|t| t.id == id).unwrap();
                            if let Some(pending) = &mut tab.pending_target {
                                pending.operation = Some(operation);
                            } else if matches!(
                                tab.connection.state().phase,
                                Phase::Ready | Phase::Selecting | Phase::MissingSession
                            ) {
                                let connection = tab.connection.clone();
                                s.submit(&connection, Action::Manage { operation });
                            }
                            s.group_focus(s.workspace.active, w, cx);
                        }
                    },
                ));
            }
            let target = pane.clone();
            menu = menu.child(self.button(
                format!("rename-pane-{target}"),
                t("패널 제목 변경"),
                cx,
                move |s, _, cx| {
                    s.menu = None;
                    s.prompt(
                        t("패널 제목 변경"),
                        PromptOp::RenamePane(target.clone()),
                        cx,
                    );
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-{pane}"),
                t("패널 삭제…"),
                t("패널").into(),
                Operation::KillPane {
                    target: pane.clone(),
                },
                connection,
                cx,
            ));
        }
        menu
    }
    fn hierarchy_delete(
        &self,
        id: String,
        label: &'static str,
        title: String,
        operation: Operation,
        connection: &Arc<Connection>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let c = connection.clone();
        self.button(id, label, cx, move |s, _, _| {
            s.menu = None;
            s.dialog = Some(Dialog::Delete {
                title: tr!("{0} 삭제", title),
                operation: DeleteOp::Remote(c.clone(), operation.clone()),
            });
        })
    }
}
