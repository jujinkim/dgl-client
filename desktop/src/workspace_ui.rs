use super::*;
use workspace::{DIVIDER, DockEdge};

/// Pointer drags in progress and the layout bounds they measure against.
#[derive(Default)]
pub(crate) struct PointerState {
    pub dragging_tab: Option<TabId>,
    pub dragging_host: Option<String>,
    pub host_drop: Option<Option<String>>,
    pub resizing: Option<Vec<bool>>,
    pub split_bounds: HashMap<Vec<bool>, Bounds<Pixels>>,
    pub group_bounds: HashMap<GroupId, Bounds<Pixels>>,
    /// Horizontal scroll of each group's tab strip, read by its display-only scrollbar.
    pub tab_scroll: HashMap<GroupId, ScrollHandle>,
}

const TAB_BAR_HEIGHT: f32 = 48.;
const TAB_SCROLLBAR_HEIGHT: f32 = 2.;
const TAB_SCROLLBAR_MIN_THUMB: f32 = 24.;

/// The thumb's `(left, width)` inside a `viewport`-wide track, or `None` when nothing overflows.
/// `offset` is the scroll offset, which runs from 0 down to `-max_offset`.
pub(crate) fn scroll_thumb(viewport: f32, max_offset: f32, offset: f32) -> Option<(f32, f32)> {
    if viewport <= 0. || max_offset <= 0.5 {
        return None;
    }
    let width = (viewport * viewport / (viewport + max_offset))
        .max(TAB_SCROLLBAR_MIN_THUMB)
        .min(viewport);
    let progress = (-offset / max_offset).clamp(0., 1.);
    Some((progress * (viewport - width), width))
}

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
            let Some(bounds) = self.pointer.group_bounds.get(&group) else {
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
            let Some(bounds) = self.pointer.group_bounds.get(&target.group) else {
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
            self.pointer.dragging_tab = None;
            self.pointer.dragging_host = None;
            self.pointer.host_drop = None;
        }
        let preview = self.pointer.dragging_tab.and_then(|tab| {
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
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(p.line))
                    .text_sm()
                    .text_color(rgb(p.muted))
                    .truncate()
                    .child(t("workspace.explorer_title")),
            );
        let hosts_header = self.sidebar_section_header(
            "hosts-header",
            t("common.hosts"),
            vec![
                self.button("add", UiIcon::Plus, cx, |s, _, cx| s.edit(None, cx))
                    .into_any_element(),
            ],
        );
        let mut hosts: Vec<_> = self
            .host_order
            .ids()
            .iter()
            .filter_map(|id| self.local.hosts.iter().find(|h| &h.id == id).cloned())
            .collect();
        let mut explorer = div().id("explorer").flex_1().min_h_0().overflow_y_scroll();
        hosts.insert(0, self.local_host());
        for host in hosts {
            let is_local = local_host::Target::of(&host) == local_host::Target::Local;
            let id = host.id.clone();
            let open = host.clone();
            let edit = host.clone();
            let delete = host.clone();
            let setup_host = host.clone();
            let toggle_host = host.clone();
            let refresh_host = host.clone();
            let expanded = self.host_expanded(&id);
            let source = expanded.then(|| self.tree_source(&id)).flatten();
            let browse = source
                .as_ref()
                .is_some_and(|s| s.connection.is_some() && s.state.browses_sessions());
            let refreshing = source.as_ref().is_some_and(|s| s.refreshing);
            let create_on = source.as_ref().and_then(|s| s.connection.clone());
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
            let before_line =
                !is_local && self.pointer.host_drop.as_ref() == Some(&Some(id.clone()));
            let after_line = !is_local && next.is_none() && self.pointer.host_drop == Some(None);
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
                                if is_local {
                                    return;
                                }
                                s.pointer.host_drop =
                                    Some(if e.event.position.y < e.bounds.center().y {
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
                        if s.pointer.dragging_host.as_ref() == Some(&dragged.id)
                            && let Some(before) = s.pointer.host_drop.take()
                            && s.local.hosts.iter().any(|h| h.id == drop_id)
                            && s.host_order.drop_before(&dragged.id, before.as_deref())
                        {
                            s.save_host_order();
                        }
                        s.pointer.dragging_host = None;
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
                            format!("expand-host-{id}"),
                            if expanded {
                                UiIcon::ChevronDown
                            } else {
                                UiIcon::ChevronRight
                            },
                            cx,
                            move |s, _, cx| {
                                s.menu = None;
                                s.toggle_host(&toggle_host, cx);
                                cx.notify();
                            },
                        )
                        .w(px(28.))
                        .px_0()
                        .flex_shrink_0(),
                    )
                    .child(
                        self.button(
                            format!("host-{id}"),
                            if host.favorite {
                                ButtonLabel::IconText(UiIcon::Star, host.alias.clone().into())
                            } else {
                                ButtonLabel::Text(host.alias.clone().into())
                            },
                            cx,
                            move |s, w, cx| {
                                s.open(open.clone(), cx);
                                s.group_focus(s.workspace.active, w, cx);
                            },
                        )
                        // Host rows are list items; the row itself carries the selection.
                        .rounded_none()
                        .border_color(gpui::transparent_black())
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(!is_local, |d| {
                            d.cursor_move().on_drag(
                                DragHost {
                                    id: id.clone(),
                                    label: host.alias.clone(),
                                },
                                move |host, _, _, cx| {
                                    drag_view.update(cx, |s, cx| {
                                        s.pointer.dragging_host = Some(host.id.clone());
                                        s.pointer.host_drop = None;
                                        s.menu = None;
                                        cx.notify();
                                    });
                                    cx.new(|_| DragPreview(host.label.clone()))
                                },
                            )
                        }),
                    )
                    .when(expanded, |d| {
                        d.child(
                            self.button(
                                format!("new-session-{id}"),
                                UiIcon::Plus,
                                cx,
                                move |s, _, cx| {
                                    if let Some(connection) = create_on.clone().filter(|_| browse) {
                                        s.prompt_on(
                                            connection,
                                            t("workspace.create_session"),
                                            PromptOp::Session,
                                            cx,
                                        );
                                    }
                                },
                            )
                            .w(px(28.))
                            .px_0()
                            .flex_shrink_0()
                            .when(!browse, |d| d.opacity(0.35)),
                        )
                        .child(
                            self.button(
                                format!("refresh-sessions-{id}"),
                                UiIcon::Refresh,
                                cx,
                                move |s, _, cx| {
                                    s.refresh_host(&refresh_host, cx);
                                    cx.notify();
                                },
                            )
                            .native_tooltip(t("workspace.refresh_sessions"))
                            .w(px(28.))
                            .px_0()
                            .flex_shrink_0()
                            .when(refreshing, |d| d.opacity(0.35)),
                        )
                    })
                    .when(!is_local, |d| {
                        d.child(
                            self.button(menu_id.clone(), UiIcon::More, cx, move |s, _, _| {
                                s.toggle_menu(toggle.clone())
                            })
                            .flex_shrink_0(),
                        )
                    }),
            );
            if self.menu.as_ref() == Some(&menu_id) {
                explorer = explorer.child(
                    div()
                        .pl_3()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .text_sm()
                        .child(self.host_connection_controls(&host.id, cx))
                        .child(self.button(
                            format!("edit-{id}"),
                            t("common.edit"),
                            cx,
                            move |s, _, cx| {
                                s.menu = None;
                                s.edit(Some(edit.clone()), cx);
                            },
                        ))
                        .child(self.button(
                            format!("agent-setup-{id}"),
                            t("agent.integration"),
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
                                        t("workspace.connect_host_first"),
                                        SnackbarDuration::Short,
                                    );
                                }
                            },
                        ))
                        .child(self.button(
                            format!("delete-{id}"),
                            t("workspace.delete_ellipsis"),
                            cx,
                            move |s, _, _| {
                                s.menu = None;
                                s.dialog = Some(Dialog::Delete {
                                    title: tr!("workspace.delete_host_named", delete.alias),
                                    operation: DeleteOp::Host(delete.clone()),
                                });
                            },
                        )),
                );
            }
            if expanded {
                explorer = explorer.child(self.render_host_tree(&host, source, cx));
            }
        }
        // One scrolling list: each host row carries its own collapsible session tree,
        // so the sessions of several hosts show at once.
        sidebar = sidebar.child(
            div()
                .debug_selector(|| "host-section".into())
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(hosts_header)
                .child(explorer),
        );
        let unread = self.agents.inbox.recent.iter().filter(|n| !n.read).count();
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
            .unwrap_or_else(|| t("workspace.empty_group").into());
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
            // Any keyboard or pointer press resumes the frame rate before its effects arrive.
            .capture_any_mouse_down(|_, _, _| crate::cadence::wake())
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                crate::cadence::wake();
                if e.keystroke.key == "escape"
                    && (s.pointer.dragging_tab.is_some() || s.pointer.dragging_host.is_some())
                    && cx.has_active_drag()
                {
                    s.pointer.dragging_tab = None;
                    s.pointer.dragging_host = None;
                    s.pointer.host_drop = None;
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
                use platform::AppShortcut::*;
                match platform::app_shortcut_for(&e.keystroke) {
                    Some(NewSession) => {
                        s.prompt(t("workspace.create_session"), PromptOp::Session, cx)
                    }
                    Some(NewHost) => s.edit(None, cx),
                    Some(CloseTab) => {
                        if let Some(id) = s.workspace.selected() {
                            s.close_tab(id);
                        }
                    }
                    Some(Settings) => s.dialog = Some(Dialog::Settings),
                    Some(Refresh) => s.refresh(),
                    Some(SelectTab(n)) => {
                        let Some(id) = s.workspace.groups[&s.workspace.active]
                            .tabs
                            .get(n - 1)
                            .copied()
                        else {
                            return;
                        };
                        s.workspace.select(s.workspace.active, id);
                        s.group_focus(s.workspace.active, w, cx);
                    }
                    // Copy and paste belong to the focused view; quit is a key action.
                    Some(Copy | Paste | Quit) | None => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_drag_move::<DragTab>(cx.listener(|_, _, _, cx| {
                cx.notify();
                cx.stop_propagation();
            }))
            .on_drag_move::<DragHost>(cx.listener(|s, _, _, cx| {
                s.pointer.host_drop = None;
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if s.dialog.is_some() {
                    return;
                }
                if let Some(path) = s.pointer.resizing.clone()
                    && let Some(bounds) = s.pointer.split_bounds.get(&path).copied()
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
                if let Some(tab) = s.pointer.dragging_tab.take() {
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
                if s.pointer.resizing.take().is_some() {
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
                                                t("input.compose_text"),
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
                                                    let anchor = self.agents.anchor.clone();
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
                                        .native_tooltip(t("common.notifications"))
                                        .child(
                                            div()
                                                .debug_selector(|| "notification-icon".into())
                                                .child(UiIcon::Bell.render(p.fg)),
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
                                        self.button("settings", UiIcon::Settings, cx, |s, _, _| {
                                            s.dialog = Some(Dialog::Settings);
                                        })
                                        .w(px(28.))
                                        .h(px(24.))
                                        .p_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .native_tooltip(t("common.settings")),
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
                                    s.pointer.split_bounds.insert(measure.clone(), bounds);
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
                                    s.pointer.resizing = Some(drag.clone());
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
        let groups = &self.workspace.groups;
        self.pointer
            .tab_scroll
            .retain(|g, _| groups.contains_key(g));
        let scroll = self.pointer.tab_scroll.entry(id).or_default().clone();
        let mut header = div()
            .id(SharedString::from(format!("group-tabs-{id}")))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_x_scroll()
            .track_scroll(&scroll)
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
            // The selected tab already wears `selection`, so its hovered control gets an outline.
            let is_selected = selected == Some(tab_id);
            header = header.child(
                div()
                    .id(SharedString::from(format!("tab-item-{tab_id}")))
                    .debug_selector(move || format!("tab-item-{tab_id}"))
                    .flex()
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(if is_selected {
                        rgb(p.accent)
                    } else {
                        rgb(p.line)
                    })
                    .when(is_selected, |d| d.bg(rgb(p.selection)))
                    // A hovered tab is tinted weakly; the control under the cursor adds `selection`.
                    .when(!is_selected, |d| d.hover(move |s| s.bg(rgb(p.hover))))
                    .child(
                        self.outline_hover_button(
                            format!("tab-{tab_id}"),
                            is_selected,
                            cx,
                            move |s, w, cx| {
                                s.workspace.select(id, tab_id);
                                s.group_focus(id, w, cx);
                            },
                        )
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
                                .child(hierarchy::tab_title(&tab.heading())),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    self.info_badge(hierarchy::display_text(&tab.host.alias))
                                        .debug_selector(move || format!("tab-host-{tab_id}"))
                                        .flex_shrink_0()
                                        .max_w(px(120.)),
                                )
                                // Without a session title the first line already shows the name.
                                .when(tab.session_heading().is_some(), |d| {
                                    d.child(
                                        div()
                                            .debug_selector(move || format!("tab-session-{tab_id}"))
                                            .min_w_0()
                                            .truncate()
                                            .text_color(rgb(p.muted))
                                            .text_size(px(11.))
                                            .line_height(px(14.))
                                            .child(hierarchy::tab_title(&tab.name())),
                                    )
                                }),
                        )
                        .native_tooltip(label.clone())
                        .cursor_move()
                        .on_drag(DragTab { id: tab_id, label }, move |tab, _, _, cx| {
                            drag_view.update(cx, |s, cx| {
                                s.pointer.dragging_tab = Some(tab.id);
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
                    .child(self.labelled(
                        self.outline_hover_button(menu.clone(), is_selected, cx, move |s, _, _| {
                            s.toggle_menu(toggle.clone())
                        }),
                        UiIcon::More,
                    ))
                    .child(self.labelled(
                        self.outline_hover_button(
                            format!("close-{tab_id}"),
                            is_selected,
                            cx,
                            move |s, _, _| s.close_tab(tab_id),
                        ),
                        UiIcon::Close,
                    )),
            );
        }
        // Display-only: the track has no handlers, so pressing it does nothing.
        let thumb_color = rgb(p.control_border);
        let header = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .debug_selector(move || format!("tab-scrollbar-{id}"))
                    .w_full()
                    .h(px(TAB_SCROLLBAR_HEIGHT))
                    .flex_shrink_0()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                // Read at paint time so the thumb matches this frame's layout.
                                let viewport = f32::from(scroll.bounds().size.width);
                                let max = f32::from(scroll.max_offset().width);
                                let offset = f32::from(scroll.offset().x);
                                if let Some((left, width)) = scroll_thumb(viewport, max, offset) {
                                    let thumb = Bounds::new(
                                        point(bounds.left() + px(left), bounds.top()),
                                        size(px(width), bounds.size.height),
                                    );
                                    window.paint_quad(
                                        fill(thumb, thumb_color)
                                            .corner_radii(px(TAB_SCROLLBAR_HEIGHT / 2.)),
                                    );
                                }
                            },
                        )
                        .size_full(),
                    ),
            );
        let bounds = self
            .pointer
            .group_bounds
            .get(&id)
            .copied()
            .unwrap_or_default();
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
                self.button(format!("merge-{id}"), UiIcon::Merge, cx, move |s, w, cx| {
                    s.workspace.merge(id);
                    s.save_layout();
                    s.group_focus(s.workspace.active, w, cx);
                })
                .native_tooltip(t("workspace.merge_groups")),
            );
        }
        for (axis, label, description) in [
            (
                Axis::Horizontal,
                UiIcon::SplitHorizontal,
                t("workspace.split_screen_horizontally"),
            ),
            (
                Axis::Vertical,
                UiIcon::SplitVertical,
                t("workspace.split_screen_vertically"),
            ),
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
                            if s.pointer.group_bounds.insert(id, bounds) != Some(bounds) {
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
                let mut menu = div().flex().flex_wrap().gap_1().text_xs();
                for (delta, label) in [
                    (-1i32, t("workspace.move_left")),
                    (1, t("workspace.move_right")),
                ] {
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
                        tr!("workspace.move_to_group", dest),
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
            if tab.host.id == local_host::ID
                && matches!(
                    state.error,
                    Some(
                        dgl_core::Error::LocalToolMissing
                            | dgl_core::Error::LocalToolUnsupported
                            | dgl_core::Error::LocalToolPermission
                    )
                )
            {
                let (install, url) = local_host::installation();
                let message = match state.error {
                    Some(dgl_core::Error::LocalToolPermission) => {
                        t("workspace.local_permission_hint")
                    }
                    Some(dgl_core::Error::LocalToolUnsupported) => {
                        t("workspace.local_requirements")
                    }
                    _ => t("workspace.local_install_hint"),
                };
                let mut panel = div()
                    .id("local-installation")
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(message);
                if let Some(command) = install {
                    panel = panel.child(command).child(self.button(
                        "copy-local-install",
                        t("workspace.local_copy_command"),
                        cx,
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(command.into()))
                        },
                    ));
                }
                if cfg!(target_os = "windows") {
                    panel = panel.child(t("workspace.local_psmux_zip_hint"));
                }
                panel = panel
                    .child(self.button(
                        "open-local-guide",
                        t("workspace.local_open_guide"),
                        cx,
                        move |_, _, cx| cx.open_url(url),
                    ))
                    .child(self.button(
                        "recheck-local-tool",
                        t("workspace.local_check_again"),
                        cx,
                        move |s, _, cx| {
                            s.workspace.active = id;
                            s.reconnect(cx);
                        },
                    ));
                body = body.child(panel);
            }

            if state.offers_retry() && !state.retry_exhausted() {
                let retry_connection = tab.connection.clone();
                let stop_connection = tab.connection.clone();
                let mut retry_controls =
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .text_xs()
                        .child(self.button(
                            format!("retry-now-{id}"),
                            t("workspace.retry_refresh"),
                            cx,
                            move |s, _, _| {
                                s.submit(&retry_connection, Action::RetryNow);
                            },
                        ));
                if state.reconnecting() {
                    retry_controls = retry_controls.child(self.button(
                        format!("stop-retry-{id}"),
                        t("workspace.stop_retrying"),
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
                        .child(state.retry_message().unwrap_or_else(|| {
                            if tab.host.id == local_host::ID {
                                t("workspace.local_connecting").into()
                            } else {
                                t("workspace.connecting_ssh").into()
                            }
                        }))
                        .child(retry_controls),
                );
            }
            if state.phase == Phase::TrustRequired {
                let host = tab.host.clone();
                let fingerprint = state.fingerprint.clone().unwrap_or_default();
                body = body
                    .child(div().p_3().child(tr!(
                        "workspace.first_connection_prompt",
                        host.endpoint(),
                        fingerprint
                    )))
                    .child(self.button(
                        format!("trust-{id}"),
                        t("workspace.trust_after_verify"),
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
                                    tr!("workspace.host_key_save_failed", e),
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
            let mut area = div()
                .debug_selector(move || format!("terminal-area-{id}"))
                .relative()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(terminal);
            if waits_for_session(&state.phase) {
                area = area.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p_3()
                        .child(
                            div()
                                .debug_selector(move || format!("select-session-hint-{id}"))
                                .text_center()
                                .text_color(rgb(p.muted))
                                .child(t("workspace.select_or_create_session")),
                        ),
                );
            }
            if state.retry_exhausted() {
                let connection = tab.connection.clone();
                area = area.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w_full()
                                .max_w(px(240.))
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(p.panel))
                                .text_color(rgb(p.fg))
                                .child(t("workspace.reconnection_failed"))
                                .child(
                                    self.button(
                                        format!("retry-center-{id}"),
                                        t("workspace.try_reconnecting_again"),
                                        cx,
                                        move |s, _, _| {
                                            s.submit(&connection, Action::RetryNow);
                                        },
                                    )
                                    .w_full()
                                    .max_w_full()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis(),
                                ),
                        ),
                );
            }
            body = body.child(area);
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
                    .child(tr!("workspace.group", id))
                    .child(t("workspace.empty_group_hint"))
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
                Some(DockEdge::Left) => t("workspace.split_left"),
                Some(DockEdge::Right) => t("workspace.split_right"),
                Some(DockEdge::Top) => t("workspace.split_above"),
                Some(DockEdge::Bottom) => t("workspace.split_below"),
                None => t("workspace.move_to_this_group"),
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
        let mut controls = div().flex().gap_1().text_xs();
        if let Some(tab) = self.active_tab().filter(|t| t.host.id == host) {
            let tid = tab.id;
            let handle = tab.connection.state().handle;
            let host_id = host.to_owned();
            let retry_host = host_id.clone();
            if !tab.connection.state().can_retry_now() {
                controls = controls.child(self.button(
                    "host-reconnect",
                    t("workspace.reconnect"),
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
                t("common.disconnect"),
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
    /// A sidebar section title with its actions on the right; hosts and sessions share it.
    fn sidebar_section_header(
        &self,
        selector: &'static str,
        title: &'static str,
        actions: Vec<AnyElement>,
    ) -> Div {
        let p = self.palette;
        div()
            .debug_selector(move || selector.into())
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .pl_3()
            .pr_1()
            .py_1()
            .border_b_1()
            .border_color(rgb(p.line))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(p.fg))
                    .child(title),
            )
            .children(actions)
    }
    /// The session tree under an expanded host row, indented on a lighter surface.
    fn render_host_tree(
        &mut self,
        host: &HostProfile,
        source: Option<host_tree::TreeSource>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette;
        let id = host.id.clone();
        let body = match source {
            Some(source) => self.render_tree_state(host, &source, cx),
            None => div()
                .debug_selector(|| "session-empty".into())
                .px_3()
                .py_2()
                .text_xs()
                .text_color(rgb(p.muted))
                .child(t("workspace.connect_host_first"))
                .into_any_element(),
        };
        div()
            .debug_selector(move || format!("host-tree-{id}"))
            .pl(px(12.))
            .bg(rgb(p.bg))
            .border_b_1()
            .border_color(rgb(p.line))
            .child(body)
    }
    pub(super) fn render_tree_state(
        &mut self,
        host: &HostProfile,
        source: &host_tree::TreeSource,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut tree = div().debug_selector(|| "session-tree".into());
        let state = &source.state;
        let connection = source.connection.as_ref();
        let browse = connection.is_some() && state.browses_sessions();
        if connection.is_none() {
            // A closed probe's rows still open tabs; refreshing lists them again.
            tree = tree.child(
                div()
                    .debug_selector(|| "session-list-paused".into())
                    .px_3()
                    .text_xs()
                    .text_color(rgb(self.palette.muted))
                    .child(t("workspace.list_paused")),
            );
        } else if !browse && state.remote.sessions.is_empty() {
            // Keep cached rows at the same coordinates while the connection recovers.
            // The status bar already displays the phase; only an empty tree needs a hint.
            tree = tree.child(
                div()
                    .debug_selector(|| "session-list-waiting".into())
                    .px_3()
                    .text_xs()
                    .child(tr!(
                        "workspace.name_waiting_list",
                        hierarchy::phase_label(&state.phase)
                    )),
            );
        }
        for row in hierarchy::rows(&state.remote, self.local.preferences.show_all_hierarchy) {
            tree = tree.child(self.render_hierarchy_row(&row, 0, host, connection, state, cx));
        }
        tree.into_any_element()
    }
    fn render_hierarchy_row(
        &self,
        row: &hierarchy::Row,
        depth: usize,
        host: &HostProfile,
        connection: Option<&Arc<Connection>>,
        state: &Arc<dgl_core::connection::State>,
        cx: &mut Context<Self>,
    ) -> Div {
        let browse = connection.is_some() && state.browses_sessions();
        let p = self.palette;
        let expand = format!("{}/{}", host.id, row.key);
        let toggle = expand.clone();
        let menu_id = format!("{expand}/menu");
        let menu = menu_id.clone();
        let c = connection.cloned();
        let open_host = host.clone();
        let listed = state.clone();
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
            .id(SharedString::from(format!("row-{expand}")))
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
                        UiIcon::ChevronDown
                    } else {
                        UiIcon::ChevronRight
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
                    let window = (!session_row).then(|| target.window.clone()).flatten();
                    let pane = (!session_row).then(|| target.pane.clone()).flatten();
                    let opened = match &c {
                        Some(c) => {
                            s.select_session_tab(c, generation, &target.session, window, pane, cx)
                        }
                        None => s.route_session(
                            open_host.clone(),
                            &target.session,
                            window,
                            pane,
                            &listed,
                            cx,
                        ),
                    };
                    if opened.is_some() {
                        s.group_focus(s.workspace.active, w, cx);
                    }
                },
            ))
            .when_some(expand_button, |d, button| d.child(button))
            .when(connection.is_some(), |d| {
                d.child(
                    self.button(menu_id.clone(), UiIcon::More, cx, move |s, _, _| {
                        s.toggle_menu(menu.clone())
                    })
                    .w(px(28.))
                    .px_0(),
                )
            });
        if let Some(pid) = &row.window
            && let Some(connection) = connection
        {
            line = line.child(self.agent_badge(connection.clone(), host, pid, state, cx));
        }
        let mut body = div().flex().flex_col().child(line);
        if browse
            && self.menu.as_ref() == Some(&menu_id)
            && let Some(connection) = connection
        {
            body = body.child(self.hierarchy_menu(row, depth, connection, cx));
        }
        if self.expanded.contains(&expand) {
            for child in &row.children {
                body = body.child(self.render_hierarchy_row(
                    child,
                    depth + 1,
                    host,
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
            .gap_1()
            .text_xs();
        if row.kind == hierarchy::Kind::Session {
            let add = row.session.clone();
            menu = menu.child(self.button(
                format!("new-window-{add}"),
                t("workspace.create_window_ellipsis"),
                cx,
                {
                    let c = connection.clone();
                    move |s, _, cx| {
                        s.menu = None;
                        s.prompt_on(
                            c.clone(),
                            t("workspace.create_window"),
                            PromptOp::Window(add.clone()),
                            cx,
                        );
                    }
                },
            ));
            let target = row.session.clone();
            menu = menu.child(self.button(
                format!("rename-session-{target}"),
                t("workspace.rename_session_ellipsis"),
                cx,
                {
                    let c = connection.clone();
                    move |s, _, cx| {
                        s.menu = None;
                        s.prompt_on(
                            c.clone(),
                            t("workspace.rename_session"),
                            PromptOp::RenameSession(target.clone()),
                            cx,
                        );
                    }
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-session-{}", row.session),
                t("workspace.delete_session"),
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
                t("workspace.rename_window_ellipsis"),
                cx,
                {
                    let c = connection.clone();
                    move |s, _, cx| {
                        s.menu = None;
                        s.prompt_on(
                            c.clone(),
                            t("workspace.rename_window"),
                            PromptOp::RenameWindow(target.clone()),
                            cx,
                        );
                    }
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-{window}"),
                t("workspace.delete_window"),
                t("common.window").into(),
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
                (true, t("workspace.split_pane_horizontally")),
                (false, t("workspace.split_pane_vertically")),
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
                            } else if tab.connection.state().browses_sessions() {
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
                t("workspace.rename_pane_ellipsis"),
                cx,
                {
                    let c = connection.clone();
                    move |s, _, cx| {
                        s.menu = None;
                        s.prompt_on(
                            c.clone(),
                            t("workspace.rename_pane"),
                            PromptOp::RenamePane(target.clone()),
                            cx,
                        );
                    }
                },
            ));
            menu = menu.child(self.hierarchy_delete(
                format!("kill-{pane}"),
                t("workspace.delete_pane"),
                t("common.pane").into(),
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
                title: tr!("common.delete_named", title),
                operation: DeleteOp::Remote(c.clone(), operation.clone()),
            });
        })
    }
}

/// A connected tab without a tmux target asks for a session instead of showing a blank grid.
fn waits_for_session(phase: &Phase) -> bool {
    *phase == Phase::Selecting
}

#[cfg(test)]
mod tests {
    use super::waits_for_session;
    use dgl_core::connection::Phase::*;

    #[test]
    fn only_session_selection_shows_the_session_hint() {
        assert!(waits_for_session(&Selecting));
        for phase in [
            Closed,
            Connecting,
            TrustRequired,
            Authenticating,
            Restoring,
            Ready,
            Reconnecting,
            MissingSession,
            Failed,
            Shell,
        ] {
            assert!(!waits_for_session(&phase), "{phase:?}");
        }
    }
}
