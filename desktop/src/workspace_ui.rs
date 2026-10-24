use super::*;
use workspace::DIVIDER;

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
    fn group_focus(&mut self, group: GroupId, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.active = group;
        if let Some(t) = self.active_tab() {
            window.focus(&t.terminal.focus_handle(cx));
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }
    pub fn render_workspace(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
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
        let workspace = self.render_layout(layout, vec![], cx);
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
            .capture_any_mouse_up(cx.listener(|s, _, _, cx| {
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match layout {
            Layout::Group { id } => self.render_group(id, cx).into_any_element(),
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
                let first = self.render_layout(*first, left, cx);
                let second = self.render_layout(*second, right, cx);
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
    fn render_group(&mut self, id: GroupId, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette;
        let group = &self.workspace.groups[&id];
        let selected = group.selected;
        let ids = group.tabs.clone();
        let entity = cx.entity();
        let mut header = div()
            .id(SharedString::from(format!("group-tabs-{id}")))
            .flex_1()
            .min_w_0()
            .overflow_x_scroll()
            .flex()
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
                        .on_drag(DragTab { id: tab_id, label }, |tab, _, _, cx| {
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
        let mut tools = div().flex().flex_shrink_0().text_xs();
        for (axis, label) in [
            (Axis::Horizontal, "화면 분할 ↔"),
            (Axis::Vertical, "화면 분할 ↕"),
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
                .when(!enabled, |d| d.opacity(0.35).cursor_default()),
            );
        }
        if self.workspace.groups.len() > 1 {
            tools = tools.child(self.button(
                format!("merge-{id}"),
                "그룹 합치기",
                cx,
                move |s, w, cx| {
                    s.workspace.merge(id);
                    s.save_layout();
                    s.group_focus(s.workspace.active, w, cx);
                },
            ));
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
            .child(div().flex().bg(rgb(p.panel)).child(header))
            .child(
                div()
                    .id(SharedString::from(format!("group-tools-{id}")))
                    .overflow_x_scroll()
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
        let mut controls = div()
            .flex()
            .flex_wrap()
            .text_xs()
            .p_2()
            .border_t_1()
            .border_color(rgb(p.line));
        for (horizontal, label) in [(true, "tmux 패널 분할 ↔"), (false, "tmux 패널 분할 ↕")]
        {
            let c = connection.clone();
            let pane = state.pane.clone();
            let ready = state.phase == Phase::Ready;
            controls = controls.child(
                self.button(label, label, cx, move |s, _, _| {
                    if ready && let Some(target) = pane.clone() {
                        s.submit(
                            &c,
                            Action::Manage {
                                operation: Operation::Split { target, horizontal },
                            },
                        );
                    }
                })
                .when(!ready, |d| d.opacity(0.35)),
            );
        }
        for (direction, arrow) in [('L', "←"), ('R', "→"), ('U', "↑"), ('D', "↓")] {
            for focus in [true, false] {
                let c = connection.clone();
                let pane = state.pane.clone();
                let ready = state.phase == Phase::Ready;
                let label = format!("{}{arrow}", if focus { "포커스 " } else { "경계" });
                controls = controls.child(
                    self.button(label.clone(), label, cx, move |s, _, _| {
                        if ready && let Some(target) = pane.clone() {
                            s.submit(
                                &c,
                                Action::Manage {
                                    operation: if focus {
                                        Operation::FocusPane { target, direction }
                                    } else {
                                        Operation::ResizePane { target, direction }
                                    },
                                },
                            );
                        }
                    })
                    .when(!ready, |d| d.opacity(0.35)),
                );
            }
        }
        tree.child(controls)
            .child(
                div()
                    .flex()
                    .text_xs()
                    .child(self.button("reconnect", "재접속", cx, |s, _, cx| s.reconnect(cx)))
                    .child(self.button("disconnect", "연결 종료", cx, |s, _, _| {
                        if let Some(id) = s.workspace.selected() {
                            s.close_tab(id);
                        }
                    })),
            )
            .into_any_element()
    }
}
