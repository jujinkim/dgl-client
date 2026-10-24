use super::*;
use workspace::{DIVIDER, DockEdge};

const TAB_BAR_HEIGHT: f32 = 32.;

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
pub(super) struct Tip(pub(super) String);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
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
        if let Some(t) = self.active_tab() {
            window.focus(&t.terminal.focus_handle(cx));
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }
    fn tab_drop_target(&self, tab: TabId, position: Point<Pixels>) -> Option<TabDropTarget> {
        if self.dialog.is_some() || self.error.is_some() {
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
                    .child(div().flex_1().child("dgl · 탐색기"))
                    .child(self.button("add", "+", cx, |s, _, cx| s.edit(None, cx))),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("호스트"),
            );
        let mut hosts = self.local.hosts.clone();
        hosts.sort_by_key(|h| (!h.favorite, std::cmp::Reverse(h.last_connected)));
        let mut explorer = div().id("explorer").flex_1().min_h_0().overflow_y_scroll();
        if hosts.is_empty() {
            explorer = explorer.child(
                div()
                    .p_3()
                    .text_sm()
                    .child("+ 버튼으로 SSH 호스트를 추가하세요."),
            );
        }
        for host in hosts {
            let id = host.id.clone();
            let open = host.clone();
            let edit = host.clone();
            let delete = host.clone();
            let menu_id = format!("host-menu-{id}");
            let toggle = menu_id.clone();
            explorer = explorer.child(
                div()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(p.line))
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
                        .truncate(),
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
                        .text_sm()
                        .child(
                            self.button(format!("edit-{id}"), "편집", cx, move |s, _, cx| {
                                s.menu = None;
                                s.edit(Some(edit.clone()), cx);
                            }),
                        )
                        .child(self.button(
                            format!("delete-{id}"),
                            "삭제…",
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                s.dialog = Some(Dialog::Delete {
                                    title: format!("호스트 {} 삭제", delete.alias),
                                    operation: DeleteOp::Host(delete.clone()),
                                });
                            },
                        )),
                );
            }
        }
        explorer = explorer.child(self.render_tree(cx));
        sidebar =
            sidebar
                .child(explorer)
                .child(self.button("settings", "설정", cx, |s, _, _| {
                    s.dialog = Some(Dialog::Settings)
                }));
        let layout = self.workspace.layout.clone();
        let workspace = self.render_layout(layout, vec![], preview, cx);
        let status = self
            .active_tab()
            .map(|tab| {
                let state = tab.connection.state();
                format!(
                    "{} · {:?} · {} · {}",
                    tab.host.alias,
                    state.phase,
                    state.session.as_deref().unwrap_or("세션 선택"),
                    state.pane.as_deref().unwrap_or("—")
                ) + &state.error.map(|e| format!(" · {e}")).unwrap_or_default()
            })
            .unwrap_or_else(|| "빈 그룹 · 왼쪽 호스트를 선택하여 연결".into());
        let mut shortcuts = div()
            .id("shortcut-bar")
            .flex_1()
            .min_w_0()
            .flex()
            .overflow_x_scroll()
            .text_xs()
            .bg(rgb(p.panel));
        if let Some(tab) = self.active_tab() {
            let terminal = tab.terminal.clone();
            for (key, label) in [
                (dgl_core::input::TerminalKey::Escape, "Esc"),
                (dgl_core::input::TerminalKey::Tab, "Tab"),
                (dgl_core::input::TerminalKey::CtrlC, "Ctrl+C"),
                (dgl_core::input::TerminalKey::CtrlD, "Ctrl+D"),
                (dgl_core::input::TerminalKey::CtrlL, "Ctrl+L"),
                (dgl_core::input::TerminalKey::Up, "↑"),
                (dgl_core::input::TerminalKey::Down, "↓"),
                (dgl_core::input::TerminalKey::Left, "←"),
                (dgl_core::input::TerminalKey::Right, "→"),
                (dgl_core::input::TerminalKey::Home, "Home"),
                (dgl_core::input::TerminalKey::End, "End"),
            ] {
                let t = terminal.clone();
                shortcuts = shortcuts.child(self.button(
                    format!("key-{label}"),
                    label,
                    cx,
                    move |_, w, cx| {
                        t.update(cx, |t, _| t.send(key.bytes().to_vec()));
                        w.focus(&t.focus_handle(cx));
                    },
                ));
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
                if e.keystroke.key == "escape" && s.dragging_tab.is_some() && cx.has_active_drag() {
                    s.dragging_tab = None;
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
                            s.dialog = None;
                            cx.stop_propagation();
                            w.prevent_default();
                        }
                        "enter" => {
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
                        s.prompt("세션 만들기", PromptOp::Session, cx)
                    }
                    "s" if cfg!(target_os = "windows") => {
                        s.prompt("세션 만들기", PromptOp::Session, cx)
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
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(sidebar)
                            .child(div().flex_1().min_w_0().child(workspace)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(40.))
                            .flex_shrink_0()
                            .bg(rgb(p.panel))
                            .child(shortcuts)
                            .child(self.launcher_button(cx)),
                    )
                    .child(
                        div()
                            .h(px(26.))
                            .px_3()
                            .flex()
                            .items_center()
                            .bg(rgb(p.panel))
                            .border_t_1()
                            .border_color(rgb(p.line))
                            .text_xs()
                            .child(status),
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
            let label = tab.host.alias.clone();
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
                        self.button(
                            format!("tab-{tab_id}"),
                            label.clone(),
                            cx,
                            move |s, w, cx| {
                                s.workspace.select(id, tab_id);
                                s.group_focus(id, w, cx);
                            },
                        )
                        .cursor_move()
                        .on_drag(DragTab { id: tab_id, label }, move |tab, _, _, cx| {
                            drag_view.update(cx, |s, cx| {
                                s.dragging_tab = Some(tab.id);
                                s.menu = None;
                                cx.notify();
                            });
                            cx.new(|_| Tip(tab.label.clone()))
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
                .tooltip(|_, cx| cx.new(|_| Tip("그룹 합치기".into())).into()),
            );
        }
        for (axis, label, description) in [
            (Axis::Horizontal, "↔️", "화면 좌우 분할"),
            (Axis::Vertical, "↕️", "화면 상하 분할"),
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
                .tooltip(move |_, cx| cx.new(|_| Tip(description.into())).into())
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
                for (delta, label) in [(-1i32, "왼쪽으로"), (1, "오른쪽으로")] {
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
                        format!("그룹 {dest}로 이동"),
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
                    "새로고침 재시도",
                    cx,
                    move |s, _, _| {
                        s.submit(&retry_connection, Action::RetryNow);
                    },
                ));
                if state.can_stop_retrying() {
                    retry_controls = retry_controls.child(self.button(
                        format!("stop-retry-{id}"),
                        "재시도 정지",
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
                                .unwrap_or_else(|| "SSH 연결 중…".into()),
                        )
                        .child(retry_controls),
                );
            }
            if state.phase == Phase::TrustRequired {
                let host = tab.host.clone();
                let fingerprint = state.fingerprint.unwrap_or_default();
                body = body
                    .child(div().p_3().child(format!(
                        "처음 연결하는 서버 · {}\n{}\n관리자에게 받은 SHA-256 지문과 비교하세요.",
                        host.endpoint(),
                        fingerprint
                    )))
                    .child(self.button(
                        format!("trust-{id}"),
                        "지문 확인 후 신뢰",
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
                                s.error = Some(format!("호스트키 저장 실패: {e}"));
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
                    .child(format!("그룹 {id}"))
                    .child("호스트를 연결하거나 탭을 끌어오세요.")
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
                Some(DockEdge::Left) => "왼쪽으로 분할",
                Some(DockEdge::Right) => "오른쪽으로 분할",
                Some(DockEdge::Top) => "위로 분할",
                Some(DockEdge::Bottom) => "아래로 분할",
                None => "이 그룹으로 이동",
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
    fn render_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette;
        let mut tree = div().mt_3().border_t_1().border_color(rgb(p.line));
        let Some(tab) = self.active_tab() else {
            return tree.into_any_element();
        };
        let connection = tab.connection.clone();
        let state = connection.state();
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
                .child(div().flex_1().child("세션"))
                .child(
                    self.button("new-session", "+", cx, move |s, _, cx| {
                        if browse {
                            s.prompt("세션 만들기", PromptOp::Session, cx);
                        }
                    })
                    .when(!browse, |d| d.opacity(0.35)),
                )
                .child(
                    self.button("세션 새로고침", "↻", cx, move |s, _, _| {
                        if browse {
                            s.refresh();
                        }
                    })
                    .tooltip(|_, cx| cx.new(|_| Tip("세션 새로고침".into())).into())
                    .when(refreshing || !browse, |d| d.opacity(0.35)),
                ),
        );
        if !browse {
            tree = tree.child(
                div()
                    .px_3()
                    .text_xs()
                    .child(format!("{:?} · 목록 대기", state.phase)),
            );
        }
        for session in &state.remote.sessions {
            let sid = session.id.clone();
            let expand = format!("{tid}/{sid}");
            let toggle = expand.clone();
            let menu_id = format!("{expand}/menu");
            let menu = menu_id.clone();
            let c = connection.clone();
            let select_sid = sid.clone();
            tree = tree.child(
                div()
                    .flex()
                    .items_center()
                    .bg(rgb(p.session_header))
                    .when(state.session.as_ref() == Some(&sid), |d| {
                        d.bg(rgb(p.selection))
                    })
                    .child(self.button(
                        format!("expand-{expand}"),
                        if self.expanded.contains(&expand) {
                            "▾"
                        } else {
                            "▸"
                        },
                        cx,
                        move |s, _, _| s.toggle_expanded(toggle.clone()),
                    ))
                    .child(
                        self.button(
                            format!("session-{sid}"),
                            session.name.clone(),
                            cx,
                            move |s, _, _| {
                                if browse {
                                    s.submit(
                                        &c,
                                        Action::SelectTarget {
                                            session: select_sid.clone(),
                                            window: None,
                                            pane: None,
                                        },
                                    );
                                }
                            },
                        )
                        .font_weight(FontWeight::BOLD)
                        .flex_1()
                        .min_w_0()
                        .truncate(),
                    )
                    .child(self.button(menu_id.clone(), "…", cx, move |s, _, _| {
                        s.toggle_menu(menu.clone())
                    })),
            );
            if self.menu.as_ref() == Some(&menu_id) {
                let add = sid.clone();
                let rename = sid.clone();
                let kill = sid.clone();
                let c = connection.clone();
                tree = tree.child(
                    div()
                        .pl_4()
                        .text_xs()
                        .flex()
                        .flex_wrap()
                        .child(self.button(
                            format!("new-window-{sid}"),
                            "+ 창",
                            cx,
                            move |s, _, cx| {
                                s.menu = None;
                                s.prompt("창 만들기", PromptOp::Window(add.clone()), cx);
                            },
                        ))
                        .child(self.button(
                            format!("rename-session-{sid}"),
                            "이름 변경",
                            cx,
                            move |s, _, cx| {
                                s.menu = None;
                                s.prompt(
                                    "세션 이름 변경",
                                    PromptOp::RenameSession(rename.clone()),
                                    cx,
                                );
                            },
                        ))
                        .child(self.button(
                            format!("kill-session-{sid}"),
                            "삭제…",
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                s.dialog = Some(Dialog::Delete {
                                    title: format!("세션 {kill} 삭제"),
                                    operation: DeleteOp::Remote(
                                        c.clone(),
                                        Operation::KillSession {
                                            target: kill.clone(),
                                        },
                                    ),
                                });
                            },
                        )),
                );
            }
            if !self.expanded.contains(&expand) {
                continue;
            }
            for win in state.remote.windows.iter().filter(|w| w.session == sid) {
                let wid = win.id.clone();
                let expand = format!("{tid}/{sid}/{wid}");
                let toggle = expand.clone();
                let menu_id = format!("{expand}/menu");
                let menu = menu_id.clone();
                let c = connection.clone();
                let select_sid = sid.clone();
                let select_wid = wid.clone();
                let selected = state.session.as_ref() == Some(&sid)
                    && state
                        .remote
                        .panes
                        .iter()
                        .any(|p| Some(&p.id) == state.pane.as_ref() && p.window == wid);
                tree = tree.child(
                    div()
                        .pl_4()
                        .flex()
                        .items_center()
                        .when(selected, |d| d.bg(rgb(p.selection)))
                        .child(self.button(
                            format!("expand-{expand}"),
                            if self.expanded.contains(&expand) {
                                "▾"
                            } else {
                                "▸"
                            },
                            cx,
                            move |s, _, _| s.toggle_expanded(toggle.clone()),
                        ))
                        .child(
                            self.button(
                                format!("window-{sid}-{wid}"),
                                win.name.clone(),
                                cx,
                                move |s, _, _| {
                                    if browse {
                                        s.submit(
                                            &c,
                                            Action::SelectTarget {
                                                session: select_sid.clone(),
                                                window: Some(select_wid.clone()),
                                                pane: None,
                                            },
                                        );
                                    }
                                },
                            )
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                        )
                        .child(self.button(menu_id.clone(), "…", cx, move |s, _, _| {
                            s.toggle_menu(menu.clone())
                        })),
                );
                if self.menu.as_ref() == Some(&menu_id) {
                    let rename = wid.clone();
                    let kill = wid.clone();
                    let c = connection.clone();
                    tree = tree.child(
                        div()
                            .pl_8()
                            .flex()
                            .text_xs()
                            .child(self.button(
                                format!("rename-{wid}"),
                                "이름 변경",
                                cx,
                                move |s, _, cx| {
                                    s.menu = None;
                                    s.prompt(
                                        "창 이름 변경",
                                        PromptOp::RenameWindow(rename.clone()),
                                        cx,
                                    );
                                },
                            ))
                            .child(self.button(
                                format!("kill-{wid}"),
                                "삭제…",
                                cx,
                                move |s, _, _| {
                                    s.menu = None;
                                    s.dialog = Some(Dialog::Delete {
                                        title: format!("창 {kill} 삭제"),
                                        operation: DeleteOp::Remote(
                                            c.clone(),
                                            Operation::KillWindow {
                                                target: kill.clone(),
                                            },
                                        ),
                                    });
                                },
                            )),
                    );
                }
                if !self.expanded.contains(&expand) {
                    continue;
                }
                for pane in state.remote.panes.iter().filter(|p| p.window == wid) {
                    let pid = pane.id.clone();
                    let c = connection.clone();
                    let select_sid = sid.clone();
                    let select_wid = wid.clone();
                    let select_pid = pid.clone();
                    let menu_id = format!("{expand}/{pid}/menu");
                    let menu = menu_id.clone();
                    tree = tree.child(
                        div()
                            .pl_8()
                            .flex()
                            .items_center()
                            .when(
                                state.session.as_ref() == Some(&sid)
                                    && state.pane.as_ref() == Some(&pid),
                                |d| d.bg(rgb(p.selection)),
                            )
                            .child(
                                self.button(
                                    format!("pane-{sid}-{wid}-{pid}"),
                                    format!(
                                        "{} {} · {}×{}",
                                        pid, pane.command, pane.width, pane.height
                                    ),
                                    cx,
                                    move |s, _, _| {
                                        if browse {
                                            s.submit(
                                                &c,
                                                Action::SelectTarget {
                                                    session: select_sid.clone(),
                                                    window: Some(select_wid.clone()),
                                                    pane: Some(select_pid.clone()),
                                                },
                                            );
                                        }
                                    },
                                )
                                .flex_1()
                                .min_w_0()
                                .truncate(),
                            )
                            .child(self.button(menu_id.clone(), "…", cx, move |s, _, _| {
                                s.toggle_menu(menu.clone())
                            })),
                    );
                    if self.menu.as_ref() == Some(&menu_id) {
                        let c = connection.clone();
                        let kill = pid.clone();
                        tree = tree.child(self.button(
                            format!("kill-{pid}"),
                            "패널 삭제…",
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                s.dialog = Some(Dialog::Delete {
                                    title: format!("패널 {kill} 삭제"),
                                    operation: DeleteOp::Remote(
                                        c.clone(),
                                        Operation::KillPane {
                                            target: kill.clone(),
                                        },
                                    ),
                                });
                            },
                        ));
                    }
                }
            }
        }
        tree.child(
            div()
                .flex()
                .text_xs()
                .when(!state.can_retry_now(), |d| {
                    d.child(self.button("reconnect", "재접속", cx, |s, _, cx| s.reconnect(cx)))
                })
                .child(self.button("disconnect", "연결 종료", cx, |s, _, _| {
                    if let Some(id) = s.workspace.selected() {
                        s.close_tab(id);
                    }
                })),
        )
        .into_any_element()
    }
}
