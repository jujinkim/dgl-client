use super::{
    AuthKind, Connection, Credential, Desktop, Dialog, HostProfile, PromptOp, Tab, Terminal,
    TextInput, text_input,
};
use gpui::{
    AppContext, Entity, EntityInputHandler, Focusable, Modifiers, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase, VisualTestContext, point, px,
};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn setup(cx: &mut TestAppContext) -> (Entity<Desktop>, &mut VisualTestContext) {
    cx.update(text_input::bind_keys);
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let path = std::env::temp_dir()
        .join(format!(
            "dgl-ui-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("settings.json");
    cx.add_window_view(|_, cx| Desktop::new(runtime, path, cx))
}
pub(super) fn redraw(cx: &mut VisualTestContext) {
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
}

struct HierarchyPreview {
    desktop: Entity<Desktop>,
    state: dgl_core::connection::State,
    width: f32,
}
impl gpui::Render for HierarchyPreview {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{ParentElement, Styled};
        let state = self.state.clone();
        let tree = self
            .desktop
            .update(cx, |s, cx| s.render_tree_state(state, cx));
        gpui::div().w(px(self.width)).child(tree)
    }
}

#[gpui::test]
fn hierarchy_wraps_and_preserves_focus_expansion_and_menus(cx: &mut TestAppContext) {
    let (desktop, ui) = setup(cx);
    let terminal = add_terminal(&desktop, ui);
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v1.json")).unwrap();
    let mut state = desktop.read_with(ui, |s, _| s.tabs[0].connection.state());
    state.remote = serde_json::from_value(fixture["remote"].clone()).unwrap();
    state.phase = dgl_core::connection::Phase::Ready;
    state.session = Some("$1".into());
    state.pane = Some("%2".into());
    desktop.update(ui, |s, _| {
        s.local.preferences.show_all_hierarchy = true;
        s.expanded
            .extend(["1/$1".into(), "1/$1/@1".into(), "1/$1/@2".into()]);
    });
    let owner = desktop.clone();
    let (preview, ui) = cx.add_window_view(|_, _| HierarchyPreview {
        desktop: owner,
        state,
        width: 280.,
    });
    for dark in [false, true] {
        desktop.update(ui, |s, _| s.palette = super::theme::Palette::new(dark));
        redraw(ui);
        for width in [240., 272., 360.] {
            preview.update(ui, |p, cx| {
                p.width = width;
                cx.notify();
            });
            redraw(ui);
            for (id, max_height) in [("window-$1-@2-title", 44.), ("window-$1-@2-detail", 17.)] {
                let bounds = ui.debug_bounds(id).unwrap();
                assert!(
                    bounds.size.height > px(0.) && bounds.size.height <= px(max_height + 1.),
                    "{id}: {bounds:?}"
                );
                assert!(bounds.right() <= px(width), "{id} overflows: {bounds:?}");
            }
            let session_x = ui.debug_bounds("session-$1").unwrap().left();
            let window_x = ui.debug_bounds("window-$1-@1").unwrap().left();
            assert_eq!(window_x - session_x, px(20.));
            assert!(ui.debug_bounds("pane-$1-@1-%1").is_none());
            let pane = ui.debug_bounds("window-$1-@2").unwrap();
            let pane_menu = ui.debug_bounds("1/$1/@2/menu").unwrap();
            let badge_id = preview.read_with(ui, |p, _| format!("agent-{}-@2", p.state.handle));
            let badge = ui
                .debug_bounds(Box::leak(badge_id.into_boxed_str()))
                .unwrap();
            assert!(pane.right() <= pane_menu.left());
            assert!(pane_menu.right() <= badge.left());
            assert!(badge.right() <= px(width));
            let title = ui.debug_bounds("window-$1-@2").unwrap();
            let menu = ui.debug_bounds("1/$1/@2/menu").unwrap();
            assert!(title.right() <= menu.left(), "title overlaps menu");
        }
    }
    let focus = ui.update(|_, cx| cx.focus_handle());
    ui.update(|w, _| w.focus(&focus));
    let expanded = desktop.read_with(ui, |s, _| s.expanded.clone());
    preview.update(ui, |p, cx| {
        p.state.remote.panes[1].title = "메타데이터 갱신 🚀".into();
        cx.notify();
    });
    redraw(ui);
    assert!(ui.update(|w, _| focus.is_focused(w)));
    assert_eq!(desktop.read_with(ui, |s, _| s.expanded.clone()), expanded);
    let title = ui.debug_bounds("window-$1-@1").unwrap();
    ui.simulate_click(title.center(), Modifiers::default());
    assert_eq!(
        desktop.read_with(ui, |s, _| s.expanded.clone()),
        expanded,
        "title selects without toggling"
    );
    assert!(ui.debug_bounds("expand-1/$1/@1").is_none());
    redraw(ui);
    let menu = ui.debug_bounds("1/$1/@1/menu").unwrap();
    ui.simulate_click(menu.center(), Modifiers::default());
    redraw(ui);
    assert_eq!(
        desktop.read_with(ui, |s, _| s.menu.clone()),
        Some("1/$1/@1/menu".into())
    );
    assert!(ui.debug_bounds("rename-@1").is_some());
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn compact_hierarchy_keeps_the_target_menus_and_panel_details(cx: &mut TestAppContext) {
    let (desktop, ui) = setup(cx);
    let terminal = add_terminal(&desktop, ui);
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v2.json")).unwrap();
    let mut state = desktop.read_with(ui, |s, _| s.tabs[0].connection.state());
    state.remote = serde_json::from_value(fixture[0]["remote"].clone()).unwrap();
    state.phase = dgl_core::connection::Phase::Ready;
    state.session = Some("$1".into());
    state.pane = Some("%1".into());
    let handle = state.handle;
    let owner = desktop.clone();
    let (preview, ui) = cx.add_window_view(|_, _| HierarchyPreview {
        desktop: owner,
        state,
        width: 240.,
    });
    let focus = ui.update(|_, cx| cx.focus_handle());
    ui.update(|w, _| w.focus(&focus));
    redraw(ui);
    let row = ui.debug_bounds("session-$1").unwrap();
    let menu = ui.debug_bounds("1/$1/menu").unwrap();
    let badge = ui
        .debug_bounds(Box::leak(format!("agent-{handle}-@1").into_boxed_str()))
        .unwrap();
    assert!(
        row.right() <= menu.left() && menu.right() <= badge.left() && badge.right() <= px(240.)
    );
    ui.simulate_click(menu.center(), Modifiers::default());
    redraw(ui);
    for id in [
        "new-window-$1",
        "rename-session-$1",
        "kill-session-$1",
        "rename-@1",
        "kill-@1",
    ] {
        assert!(ui.debug_bounds(id).is_some(), "missing merged action {id}");
    }
    assert!(ui.update(|w, _| focus.is_focused(w)));
    ui.simulate_click(badge.center(), Modifiers::default());
    assert!(desktop.read_with(
        ui,
        |s, _| matches!(&s.dialog, Some(super::Dialog::Agents(_, _, Some(pane))) if pane == "@1")
    ));
    let before = preview.read_with(ui, |p, _| (p.state.session.clone(), p.state.pane.clone()));
    desktop.update(ui, |s, _| {
        s.dialog = None;
        s.local.preferences.show_all_hierarchy = true;
        s.expanded.extend(["1/$1".into(), "1/$1/@1".into()]);
    });
    redraw(ui);
    assert!(ui.debug_bounds("window-$1-@1").is_some());
    assert!(ui.debug_bounds("pane-$1-@1-%1").is_none());
    desktop.update(ui, |s, _| s.local.preferences.show_all_hierarchy = false);
    redraw(ui);
    assert_eq!(
        before,
        preview.read_with(ui, |p, _| (p.state.session.clone(), p.state.pane.clone()))
    );
    assert!(desktop.read_with(ui, |s, _| s.expanded.contains("1/$1/@1")));
    assert!(ui.update(|w, _| focus.is_focused(w)));
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

struct AgentPanelPreview {
    desktop: Entity<Desktop>,
    connection: Arc<Connection>,
    state: dgl_core::connection::State,
}
impl gpui::Render for AgentPanelPreview {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let state = self.state.clone();
        let connection = self.connection.clone();
        self.desktop.update(cx, |s, cx| {
            s.agent_details_state(connection, "검증".into(), Some("%1"), state, cx)
        })
    }
}

#[gpui::test]
fn panel_details_keep_all_runs_and_exclude_other_panes(cx: &mut TestAppContext) {
    use dgl_core::agent::{Agent, AgentEvent, Kind, Observation, Snapshot};
    let (desktop, ui) = setup(cx);
    let terminal = add_terminal(&desktop, ui);
    let connection = desktop.read_with(ui, |s, _| s.tabs[0].connection.clone());
    let mut state = connection.state();
    let event = |run: &str, pane: &str, kind| AgentEvent {
        id: run.into(),
        seq: 1,
        server: "server".into(),
        run: run.into(),
        actor: "parent".into(),
        session: "$1".into(),
        window: "@1".into(),
        pane: pane.into(),
        agent: Agent::Claude,
        kind,
        event: "Test".into(),
        at: dgl_core::agent::now_ms(),
    };
    state.agents = Snapshot {
        valid: true,
        observed_at: dgl_core::agent::now_ms(),
        observations: [
            event("one", "%1", Kind::Question),
            event("two", "%1", Kind::Response),
            event("other", "%2", Kind::Error),
        ]
        .into_iter()
        .map(|e| Observation {
            session: e.session.clone(),
            window: e.window.clone(),
            pane: e.pane.clone(),
            run: e.run.clone(),
            actor: e.actor.clone(),
            agent: e.agent,
            last: Some(e),
            supported: vec![],
        })
        .collect(),
        ..Default::default()
    };
    assert!(
        state
            .agents
            .label(Some("%1"), dgl_core::agent::now_ms())
            .contains("질문")
    );
    assert!(
        state
            .agents
            .label(Some("%2"), dgl_core::agent::now_ms())
            .contains("오류")
    );
    assert!(
        state
            .agents
            .label(Some("%3"), dgl_core::agent::now_ms())
            .contains("미확인")
    );
    let owner = desktop.clone();
    let (_, ui) = cx.add_window_view(|_, _| AgentPanelPreview {
        desktop: owner,
        connection,
        state,
    });
    redraw(ui);
    assert!(ui.debug_bounds("agent-go-one-parent").is_some());
    assert!(ui.debug_bounds("agent-go-two-parent").is_some());
    assert!(ui.debug_bounds("agent-go-other-parent").is_none());
    assert!(ui.debug_bounds("agent-inspect").is_none());
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn host_disconnect_only_closes_the_selected_connection(cx: &mut TestAppContext) {
    let (desktop, ui) = setup(cx);
    let first_terminal = add_terminal(&desktop, ui);
    let second_terminal = add_terminal(&desktop, ui);
    let (first_id, first_handle, second_id) = desktop.update(ui, |s, _| {
        s.local.hosts.push(s.tabs[0].host.clone());
        (
            s.tabs[0].id,
            s.tabs[0].connection.state().handle,
            s.tabs[1].id,
        )
    });
    assert!(!desktop.read_with(ui, |s, _| s.matches_selected_connection(
        "fixture",
        first_id,
        first_handle
    )));
    redraw(ui);
    let menu = ui.debug_bounds("host-menu-fixture").unwrap();
    ui.simulate_click(menu.center(), Modifiers::default());
    redraw(ui);
    let disconnect = ui.debug_bounds("host-disconnect").unwrap();
    ui.simulate_click(disconnect.center(), Modifiers::default());
    redraw(ui);
    assert!(desktop.read_with(ui, |s, _| s.tabs.len() == 1
        && s.tabs[0].id == first_id
        && s.workspace.selected() != Some(second_id)));
    assert!(first_terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
    assert!(second_terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn retry_buttons_keep_the_tab_terminal_and_connection(cx: &mut TestAppContext) {
    use dgl_core::connection::State;
    use std::time::Duration;
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    let (runtime, connection, group, tab) = view.read_with(cx, |s, _| {
        (
            s.runtime.clone(),
            s.tabs[0].connection.clone(),
            s.workspace.active,
            s.tabs[0].id,
        )
    });
    let wait = |predicate: fn(&State) -> bool| {
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), async {
                while !predicate(&connection.state()) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
        });
    };
    wait(|s| s.retry.wait_seconds.is_some());
    let handle = connection.state().handle;
    redraw(cx);
    assert_eq!(group, 1);
    let stop = cx.debug_bounds("stop-retry-1").unwrap();
    cx.simulate_click(stop.center(), Modifiers::default());
    wait(|s| s.retry.stopped);
    view.update(cx, |_, cx| cx.notify());
    redraw(cx);
    // GPUI keeps old debug bounds after removing an element. Its old location
    // must no longer invoke StopRetrying (which would now report NotReady).
    cx.simulate_click(stop.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.error.is_none()));
    assert!(!connection.state().can_stop_retrying());
    let retry = cx.debug_bounds("retry-now-1").unwrap();
    cx.simulate_click(retry.center(), Modifiers::default());
    wait(|s| !s.retry.stopped && s.retry.wait_seconds.is_some());
    view.update(cx, |_, cx| cx.notify());
    redraw(cx);
    assert!(cx.debug_bounds("stop-retry-1").is_some());
    assert_eq!(connection.state().handle, handle);
    assert_eq!(connection.state().retry.attempt, 0);
    assert!(view.read_with(cx, |s, _| {
        s.tabs.len() == 1
            && s.tabs[0].id == tab
            && s.tabs[0].terminal.entity_id() == terminal.entity_id()
    }));
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    connection.close();
    runtime.block_on(connection.join());
}

#[gpui::test]
fn agent_notice_missing_target_is_read_without_reconnect_or_input(cx: &mut TestAppContext) {
    use dgl_core::agent::{Agent, AgentEvent, Kind, Notice};
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.inbox.recent.push_back(Notice {
            endpoint: "fixture@127.0.0.1:1".into(),
            read: false,
            event: AgentEvent {
                id: "test".into(),
                seq: 1,
                server: "old-server".into(),
                run: "old-run".into(),
                actor: "parent".into(),
                session: "$0".into(),
                window: "@0".into(),
                pane: "%0".into(),
                agent: Agent::Codex,
                kind: Kind::Approval,
                event: "PermissionRequest".into(),
                at: dgl_core::agent::now_ms(),
            },
        });
        s.dialog = Some(Dialog::Notifications);
        cx.notify();
    });
    redraw(cx);
    let button = cx.debug_bounds("notice-fixture@127.0.0.1:1:test").unwrap();
    cx.simulate_click(button.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.inbox.recent[0].read));
    assert!(view.read_with(cx, |s, _| s.error.as_ref().unwrap().contains("종료")));
    assert_eq!(view.read_with(cx, |s, _| s.tabs.len()), 1);
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}
fn input(view: &Entity<Desktop>, cx: &mut VisualTestContext) -> Entity<TextInput> {
    view.read_with(cx, |s, _| match s.dialog.as_ref().unwrap() {
        Dialog::Host { fields, .. } => fields[0].clone(),
        Dialog::Prompt { input, .. } => input.clone(),
        _ => panic!("input dialog"),
    })
}
#[gpui::test]
fn modal_focus_cycle_buttons_restore_and_occlusion(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let previous = cx.update(|w, cx| {
        let focus = view.read(cx).focus.clone();
        w.focus(&focus);
        focus
    });
    view.update(cx, |s, cx| {
        s.edit(None, cx);
        cx.notify();
    });
    redraw(cx);
    let first = input(&view, cx);
    assert!(cx.update(|w, cx| first.focus_handle(cx).is_focused(w)));
    cx.simulate_input("alias");
    assert_eq!(first.read_with(cx, |i, _| i.content.to_string()), "alias");
    cx.simulate_keystrokes("tab");
    let second = view.read_with(cx, |s, _| {
        if let Some(Dialog::Host { fields, .. }) = &s.dialog {
            fields[1].clone()
        } else {
            panic!()
        }
    });
    assert!(cx.update(|w, cx| second.focus_handle(cx).is_focused(w)));
    cx.simulate_keystrokes("shift-tab");
    assert!(cx.update(|w, cx| first.focus_handle(cx).is_focused(w)));
    cx.simulate_keystrokes("shift-tab");
    assert!(!cx.update(|w, cx| first.focus_handle(cx).is_focused(w)));
    cx.simulate_keystrokes("tab");
    assert!(cx.update(|w, cx| first.focus_handle(cx).is_focused(w)));
    // The host-add control is still rendered behind the scrim, but cannot replace the dialog.
    let add = cx.debug_bounds("add").unwrap();
    cx.simulate_click(add.center(), Modifiers::default());
    assert_eq!(input(&view, cx).entity_id(), first.entity_id());
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
    assert!(cx.update(|w, _| previous.is_focused(w)));
    // A dialog without inputs starts on a button and is operable from the keyboard.
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    cx.simulate_keystrokes("enter");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
}

fn add_terminal(view: &Entity<Desktop>, cx: &mut VisualTestContext) -> Entity<Terminal> {
    view.update(cx, |s, cx| {
        let host = HostProfile {
            id: "fixture".into(),
            alias: "fixture".into(),
            hostname: "127.0.0.1".into(),
            port: 1,
            username: "fixture".into(),
            auth: AuthKind::Password,
            credential_ref: "fixture".into(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        };
        let _enter = s.runtime.enter();
        let connection = Connection::start(host.clone(), Credential::default(), None);
        let terminal = cx.new(|cx| Terminal::new(connection.clone(), 14, 5000, cx));
        let id = s.workspace.add_tab();
        s.tabs.push(Tab {
            id,
            refresh: None,
            host,
            connection,
            terminal: terminal.clone(),
            last_sequence: 0,
        });
        cx.notify();
        terminal
    })
}
#[gpui::test]
fn prompt_ime_enter_and_background_terminal_input(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    redraw(cx);
    cx.update(|w, cx| w.focus(&terminal.focus_handle(cx)));
    cx.simulate_keystrokes("tab");
    assert_eq!(
        terminal.read_with(cx, |t, cx| t.captured_inputs(cx).clone()),
        vec![vec![b'\t']]
    );
    terminal.update(cx, |t, cx| t.clear_captured_inputs(cx));
    view.update(cx, |s, cx| {
        s.prompt("세션 만들기", PromptOp::Session, cx);
        cx.notify();
    });
    redraw(cx);
    let name = input(&view, cx);
    cx.update(|w, cx| {
        name.update(cx, |i, cx| {
            i.replace_and_mark_text_in_range(None, "한", Some(1..1), w, cx)
        })
    });
    cx.simulate_keystrokes("enter");
    assert!(view.read_with(cx, |s, _| s.dialog.is_some()));
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    // Commit IME, then Enter submits through the same handler as the confirm button.
    cx.update(|w, cx| name.update(cx, |i, cx| i.replace_text_in_range(None, "한글", w, cx)));
    // IME selection offsets are relative to the composition, including after a multibyte prefix.
    cx.update(|w, cx| {
        name.update(cx, |i, cx| {
            i.replace_and_mark_text_in_range(None, "ab", Some(1..1), w, cx);
            assert_eq!(i.selected_text_range(false, w, cx).unwrap().range, 3..3);
            i.unmark_text(w, cx);
        })
    });
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
    assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    view.update(cx, |s, cx| {
        s.edit(None, cx);
        cx.notify();
    });
    redraw(cx);
    let bounds = cx.debug_bounds("terminal").unwrap();
    // Click the visible edge of the terminal behind the modal, then type and scroll.
    let point = point(bounds.right() - px(5.), bounds.top() + px(5.));
    cx.simulate_click(point, Modifiers::default());
    cx.simulate_input("secret");
    cx.simulate_event(ScrollWheelEvent {
        position: point,
        delta: ScrollDelta::Lines(gpui::point(0., 5.)),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
    });
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    assert_eq!(
        input(&view, cx).read_with(cx, |i, _| i.content.to_string()),
        "secret"
    );
    cx.simulate_keystrokes("escape");
    redraw(cx);
    cx.simulate_keystrokes("tab");
    assert_eq!(
        terminal.read_with(cx, |t, cx| t.captured_inputs(cx).clone()),
        vec![vec![b'\t']]
    );
}

#[gpui::test]
fn group_split_drag_move_merge_and_terminal_identity(cx: &mut TestAppContext) {
    use super::workspace::Axis;
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    redraw(cx);
    let tab = view.read_with(cx, |s, _| s.workspace.selected().unwrap());
    let split = cx.debug_bounds("split-1-Horizontal").unwrap();
    cx.simulate_click(split.center(), Modifiers::default());
    redraw(cx);
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 2);
    let right = view.read_with(cx, |s, _| s.workspace.active);
    assert!(view.read_with(cx, |s, _| s.workspace.selected().is_none()));
    let divider = view.read_with(cx, |s, _| {
        let b = s.group_bounds[&1];
        gpui::point(b.right() + px(2.), b.center().y)
    });
    cx.simulate_mouse_down(divider, gpui::MouseButton::Left, Modifiers::default());
    let moved = gpui::point(divider.x + px(30.), divider.y);
    cx.simulate_mouse_move(moved, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(moved, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    view.read_with(cx, |s, _| {
        assert!(matches!(s.workspace.layout, super::workspace::Layout::Split { ratio, .. } if ratio > 0.5));
        assert_eq!(super::workspace::Layout::load(&s.path.with_file_name("workspace-layout.json")), s.workspace.layout);
    });
    view.update(cx, |s, cx| {
        let b = s.group_bounds[&right];
        assert!(s.workspace.split(
            right,
            Axis::Vertical,
            f32::from(b.size.width),
            f32::from(b.size.height)
        ));
        cx.notify();
    });
    redraw(cx);
    let bottom = view.read_with(cx, |s, _| s.workspace.active);
    // Use GPUI's actual drag/drop dispatch to move the tab into the empty group.
    let from = cx.debug_bounds("tab-1").unwrap().center();
    let to = view.read_with(cx, |s, _| s.group_bounds[&bottom].center());
    cx.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        gpui::point(from.x + px(20.), from.y),
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(
        view.read_with(cx, |s, _| s.workspace.groups[&bottom].selected),
        Some(tab)
    );
    assert_eq!(
        view.read_with(cx, |s, _| s.active_tab().unwrap().terminal.entity_id()),
        terminal.entity_id()
    );
    // Closing one tab leaves its group in place; merging explicitly removes it.
    view.update(cx, |s, cx| {
        s.close_tab(tab);
        assert!(s.workspace.groups[&bottom].tabs.is_empty());
        assert!(s.workspace.merge(bottom));
        cx.notify();
    });
    redraw(cx);
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 2);
}

#[gpui::test]
fn pane_toolbars_stay_above_each_terminal_and_disable_unavailable_actions(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let first = add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        assert!(
            s.workspace
                .split(1, super::workspace::Axis::Horizontal, 1200., 800.)
        );
        cx.notify();
    });
    let second = add_terminal(&view, cx);
    cx.simulate_resize(gpui::size(px(850.), px(600.)));
    redraw(cx);
    let toolbar1 = cx.debug_bounds("tmux-toolbar-1").unwrap();
    let toolbar2 = cx.debug_bounds("tmux-toolbar-2").unwrap();
    let header1 = cx.debug_bounds("group-header-1").unwrap();
    let header2 = cx.debug_bounds("group-header-2").unwrap();
    assert!(toolbar1.top() >= header1.bottom());
    assert!(toolbar2.top() >= header2.bottom());
    assert!(toolbar1.right() <= header1.right());
    assert!(toolbar2.right() <= header2.right());
    assert!(cx.debug_bounds("terminal").unwrap().top() >= toolbar2.bottom());
    for (toolbar, category) in [
        (toolbar1, "tmux-category-1-split"),
        (toolbar1, "tmux-category-1-focus"),
        (toolbar1, "tmux-category-1-resize"),
        (toolbar1, "tmux-category-1-close"),
        (toolbar2, "tmux-category-2-split"),
        (toolbar2, "tmux-category-2-focus"),
        (toolbar2, "tmux-category-2-resize"),
        (toolbar2, "tmux-category-2-close"),
    ] {
        let bounds = cx.debug_bounds(category).unwrap();
        assert!(toolbar.contains(&bounds.center()));
        assert!(bounds.left() >= toolbar.left() && bounds.right() <= toolbar.right());
    }
    assert!(cx.debug_bounds("tmux 패널 분할 ↔").is_none());
    for button in [
        "tmux-1-split-0",
        "tmux-1-focus-0",
        "tmux-1-resize-0",
        "tmux-1-close-0",
        "tmux-2-split-1",
        "tmux-2-focus-3",
        "tmux-2-resize-3",
        "tmux-2-close-0",
    ] {
        let bounds = cx.debug_bounds(button).unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.dialog.is_none() && s.error.is_none()));
    }
    assert_eq!(view.read_with(cx, |s, _| s.tabs.len()), 2);
    assert!(first.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    assert!(second.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn shortcut_buttons_are_spaced_and_send_modified_keys_to_selected_terminal(
    cx: &mut TestAppContext,
) {
    let (view, cx) = setup(cx);
    let first = add_terminal(&view, cx);
    let second = add_terminal(&view, cx);
    redraw(cx);
    let mut expected = Vec::new();
    for theme in ["light", "dark"] {
        view.update(cx, |s, cx| {
            s.local.preferences.theme = theme.into();
            cx.notify();
        });
        redraw(cx);
        let mut previous_right = None;
        for (selector, bytes) in [
            ("key-Esc", b"\x1b".as_slice()),
            ("key-Tab", b"\t".as_slice()),
            ("key-Enter", b"\r".as_slice()),
            ("key-Shift+Tab", b"\x1b[Z".as_slice()),
            ("key-Shift+←", b"\x1b[1;2D".as_slice()),
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert_eq!(bounds.size.height, px(32.));
            if let Some(right) = previous_right {
                assert!(bounds.left() > right);
            }
            previous_right = Some(bounds.right());
            cx.simulate_click(bounds.center(), Modifiers::default());
            redraw(cx);
            expected.push(bytes.to_vec());
            assert_eq!(
                second.read_with(cx, |t, cx| t.captured_inputs(cx).clone()),
                expected
            );
            assert!(first.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
            assert!(cx.update(|w, cx| second.focus_handle(cx).is_focused(w)));
        }
    }
    // On-screen modifiers encode the same events as the physical keyboard.
    cx.simulate_keystrokes("shift-tab shift-left");
    expected.extend([b"\x1b[Z".to_vec(), b"\x1b[1;2D".to_vec()]);
    assert_eq!(
        second.read_with(cx, |t, cx| t.captured_inputs(cx).clone()),
        expected
    );
    assert!(first.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn shortcut_settings_change_the_bar_and_survive_reload(cx: &mut TestAppContext) {
    use dgl_core::{input::TerminalKey, model::LocalState};
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    for selector in [
        "shortcut-setting-Esc",
        "shortcut-setting-Tab",
        "shortcut-setting-Enter",
        "shortcut-setting-Shift+Tab",
        "shortcut-setting-Shift+←",
    ] {
        let bounds = cx.debug_bounds(selector).unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        redraw(cx);
    }
    view.read_with(cx, |s, _| {
        let saved = LocalState::read(&s.path).unwrap();
        assert_eq!(
            saved.preferences.terminal_shortcuts,
            s.local.preferences.terminal_shortcuts
        );
        assert!(
            !saved
                .preferences
                .terminal_shortcuts
                .contains(&TerminalKey::Enter)
        );
    });
    let close = cx.debug_bounds("cancel-dialog").unwrap();
    cx.simulate_click(close.center(), Modifiers::default());
    redraw(cx);
    // Removed keys no longer occupy the front of the bar.
    let bar = cx.debug_bounds("shortcut-bar").unwrap();
    let first = cx.debug_bounds("key-Ctrl+C").unwrap();
    assert_eq!(first.left(), bar.left());
    cx.simulate_click(first.center(), Modifiers::default());
    assert_eq!(
        terminal.read_with(cx, |t, cx| t.captured_inputs(cx).clone()),
        vec![b"\x03".to_vec()]
    );
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    let reset = cx.debug_bounds("reset-shortcuts").unwrap();
    cx.simulate_click(reset.center(), Modifiers::default());
    redraw(cx);
    view.read_with(cx, |s, _| {
        assert_eq!(
            LocalState::read(&s.path)
                .unwrap()
                .preferences
                .terminal_shortcuts,
            TerminalKey::DEFAULT_SHORTCUTS
        );
    });
    assert_eq!(
        terminal.read_with(cx, |t, cx| t.captured_inputs(cx).len()),
        1
    );
}

fn begin_tab_drag(cx: &mut VisualTestContext) {
    let from = cx.debug_bounds("tab-1").unwrap().center();
    cx.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(from.x + px(20.), from.y),
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    redraw(cx);
    assert!(cx.update(|_, cx| cx.has_active_drag()));
}

#[gpui::test]
fn tab_drag_splits_at_each_edge_and_preview_matches_result(cx: &mut TestAppContext) {
    use super::workspace::{DockEdge, Layout};
    for edge in [
        DockEdge::Left,
        DockEdge::Right,
        DockEdge::Top,
        DockEdge::Bottom,
    ] {
        let (view, cx) = setup(cx);
        let terminal = add_terminal(&view, cx);
        add_terminal(&view, cx);
        redraw(cx);
        let before = view.read_with(cx, |s, _| s.group_bounds[&1]);
        let to = match edge {
            DockEdge::Left => point(before.left() + px(8.), before.center().y),
            DockEdge::Right => point(before.right() - px(8.), before.center().y),
            DockEdge::Top => point(before.center().x, before.top() + px(40.)),
            DockEdge::Bottom => point(before.center().x, before.bottom() - px(8.)),
        };
        begin_tab_drag(cx);
        cx.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        let preview = cx.debug_bounds("tab-drop-preview-1").unwrap();
        assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 1);
        assert!(preview.size.width > px(0.) && preview.size.height > px(0.));
        cx.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert_eq!(s.workspace.groups.len(), 2);
            let active = s.workspace.active;
            assert_eq!(s.workspace.groups[&active].tabs, vec![1]);
            assert_eq!(s.workspace.groups[&1].tabs, vec![2]);
            assert_eq!(
                s.active_tab().unwrap().terminal.entity_id(),
                terminal.entity_id()
            );
            assert_eq!(
                Layout::load(&s.path.with_file_name("workspace-layout.json")),
                s.workspace.layout
            );
            let actual = s.group_bounds[&active];
            assert!((f32::from(actual.origin.x - preview.origin.x)).abs() <= 3.);
            assert!((f32::from(actual.origin.y - preview.origin.y)).abs() <= 3.);
            assert!((f32::from(actual.size.width - preview.size.width)).abs() <= 3.);
            assert!((f32::from(actual.size.height - preview.size.height)).abs() <= 3.);
            assert!(s.dragging_tab.is_none());
        });
        assert!(!cx.update(|_, cx| cx.has_active_drag()));
        assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    }
}

#[gpui::test]
fn tab_drag_moves_into_occupied_group_and_reorders_in_header(cx: &mut TestAppContext) {
    use super::workspace::Axis;
    let (view, cx) = setup(cx);
    let first = add_terminal(&view, cx);
    add_terminal(&view, cx);
    redraw(cx);
    view.update(cx, |s, cx| {
        assert!(s.workspace.split(1, Axis::Horizontal, 1200., 800.));
        cx.notify();
    });
    let right = view.read_with(cx, |s, _| s.workspace.active);
    add_terminal(&view, cx);
    redraw(cx);
    let to = view.read_with(cx, |s, _| s.group_bounds[&right].center());
    begin_tab_drag(cx);
    cx.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    let preview = cx.debug_bounds("tab-drop-preview-2").unwrap();
    assert!(preview.contains(&to));
    cx.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(
        view.read_with(cx, |s, _| s.workspace.groups[&right].tabs.clone()),
        vec![3, 1]
    );
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 2);
    let before = cx.debug_bounds("tab-3").unwrap().center();
    begin_tab_drag(cx);
    cx.simulate_mouse_move(before, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(before, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(
        view.read_with(cx, |s, _| s.workspace.groups[&right].tabs.clone()),
        vec![1, 3]
    );
    assert_eq!(
        view.read_with(cx, |s, _| s.active_tab().unwrap().terminal.entity_id()),
        first.entity_id()
    );
    assert!(first.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn tab_drag_cancel_outside_and_small_groups_do_not_split(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    add_terminal(&view, cx);
    redraw(cx);
    let bounds = view.read_with(cx, |s, _| s.group_bounds[&1]);
    let edge = point(bounds.right() - px(8.), bounds.center().y);
    begin_tab_drag(cx);
    cx.simulate_mouse_move(edge, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert!(cx.debug_bounds("tab-drop-preview-1").is_some());
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(edge, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert!(!cx.update(|_, cx| cx.has_active_drag()));
    assert!(view.read_with(cx, |s, _| s.dragging_tab.is_none()));
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 1);
    let outside = point(px(20.), px(150.));
    begin_tab_drag(cx);
    cx.simulate_mouse_move(outside, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(outside, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(
        view.read_with(cx, |s, _| s.workspace.groups[&1].tabs.clone()),
        vec![1, 2]
    );
    cx.simulate_resize(gpui::size(px(640.), px(360.)));
    redraw(cx);
    let bounds = view.read_with(cx, |s, _| s.group_bounds[&1]);
    let edge = point(bounds.right() - px(8.), bounds.center().y);
    begin_tab_drag(cx);
    cx.simulate_mouse_move(edge, gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(edge, gpui::MouseButton::Left, Modifiers::default());
    redraw(cx);
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 1);
    assert_eq!(
        view.read_with(cx, |s, _| s.workspace.groups[&1].tabs.clone()),
        vec![1, 2]
    );
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn split_emoji_buttons_stay_at_right_of_overflowing_tab_bar(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    for _ in 0..8 {
        add_terminal(&view, cx);
    }
    cx.simulate_resize(gpui::size(px(640.), px(480.)));
    redraw(cx);
    let header = cx.debug_bounds("group-header-1").unwrap();
    let tools = cx.debug_bounds("group-tools-1").unwrap();
    let horizontal = cx.debug_bounds("split-1-Horizontal").unwrap();
    let vertical = cx.debug_bounds("split-1-Vertical").unwrap();
    assert!((f32::from(header.right() - tools.right())).abs() <= 1.);
    assert!(header.contains(&horizontal.center()));
    assert!(header.contains(&vertical.center()));
    assert!(vertical.left() >= horizontal.right());
    assert!(tools.size.width < px(100.));
    // The narrow group cannot be split horizontally even though the button is visible.
    cx.simulate_click(horizontal.center(), Modifiers::default());
    redraw(cx);
    assert_eq!(view.read_with(cx, |s, _| s.workspace.groups.len()), 1);
}

#[gpui::test]
fn launcher_layout_disabled_actions_dismissal_and_focus(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    add_terminal(&view, cx);
    cx.simulate_resize(gpui::size(px(640.), px(480.)));
    redraw(cx);
    for theme in ["light", "dark"] {
        let terminal = view.read_with(cx, |s, _| s.active_tab().unwrap().terminal.clone());
        view.update(cx, |s, cx| {
            s.local.preferences.theme = theme.into();
            cx.notify();
        });
        redraw(cx);
        let trigger = cx.debug_bounds("cli-launcher").unwrap();
        assert!(trigger.right() <= px(640.));
        assert!(trigger.size.width >= px(40.));
        cx.simulate_click(trigger.center(), Modifiers::default());
        redraw(cx);
        let menu = cx.debug_bounds("cli-launcher-menu").unwrap();
        assert!(menu.bottom() <= trigger.top());
        assert_eq!(menu.size.width, px(300.));
        assert!(cx.debug_bounds("launch-Claude Code-test").is_none());
        assert!(cx.debug_bounds("launch-Codex-test").is_none());
        for (badge, normal) in [
            ("launch-AGY-test", "launch-AGY-normal"),
            ("launch-Grok Build-test", "launch-Grok Build-normal"),
        ] {
            let badge = cx.debug_bounds(badge).unwrap();
            let normal = cx.debug_bounds(normal).unwrap();
            assert!(badge.left() > normal.left());
            assert!(badge.right() <= normal.right());
            assert!(badge.size.height < px(20.));
        }
        for (normal, bypass) in [
            ("launch-Claude Code-normal", "launch-Claude Code-bypass"),
            ("launch-Codex-normal", "launch-Codex-bypass"),
            ("launch-AGY-normal", "launch-AGY-bypass"),
            ("launch-Grok Build-normal", "launch-Grok Build-bypass"),
        ] {
            let normal = cx.debug_bounds(normal).unwrap();
            let bypass = cx.debug_bounds(bypass).unwrap();
            assert!(
                (f32::from(normal.size.width) / f32::from(bypass.size.width) - 1.5).abs() < 0.02
            );
            cx.simulate_click(normal.center(), Modifiers::default());
            cx.simulate_click(bypass.center(), Modifiers::default());
            assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
            assert!(view.read_with(cx, |s, _| s.launcher.opened.is_some()));
        }
        cx.simulate_keystrokes("escape");
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.launcher.opened.is_none()));
        assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
        assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
        cx.simulate_click(trigger.center(), Modifiers::default());
        redraw(cx);
        cx.simulate_click(point(px(300.), px(100.)), Modifiers::default());
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.launcher.opened.is_none()));
        cx.simulate_click(trigger.center(), Modifiers::default());
        redraw(cx);
        // Changing active tabs invalidates the open menu without sending anything.
        add_terminal(&view, cx);
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.launcher.opened.is_none()));
    }
}

#[gpui::test]
fn input_tools_stay_under_terminals_with_fixed_ends_in_narrow_and_split_windows(
    cx: &mut TestAppContext,
) {
    let (view, cx) = setup(cx);
    add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.local.preferences.terminal_shortcuts = dgl_core::input::TerminalKey::ALL.to_vec();
        cx.notify();
    });
    for width in [640., 1200.] {
        cx.simulate_resize(gpui::size(px(width), px(800.)));
        for theme in ["light", "dark"] {
            view.update(cx, |s, cx| {
                s.local.preferences.theme = theme.into();
                cx.notify();
            });
            redraw(cx);
            let sidebar = cx.debug_bounds("session-sidebar").unwrap();
            let tools = cx.debug_bounds("terminal-input-tools").unwrap();
            let compose = cx.debug_bounds("open-composer").unwrap();
            let shortcuts = cx.debug_bounds("shortcut-bar").unwrap();
            let cli = cx.debug_bounds("cli-launcher").unwrap();
            assert!(tools.left() >= sidebar.right());
            assert!(tools.bottom() <= sidebar.bottom());
            assert!(compose.right() <= shortcuts.left());
            assert!(shortcuts.right() <= cli.left());
            assert!(cli.right() <= px(width));
            cx.simulate_event(ScrollWheelEvent {
                position: shortcuts.center(),
                delta: ScrollDelta::Pixels(point(px(10000.), px(0.))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            });
            redraw(cx);
            let key_before = cx.debug_bounds("key-Ctrl+A").unwrap();
            cx.simulate_event(ScrollWheelEvent {
                position: shortcuts.center(),
                delta: ScrollDelta::Pixels(point(px(-400.), px(0.))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            });
            redraw(cx);
            assert_eq!(compose, cx.debug_bounds("open-composer").unwrap());
            assert_eq!(cli, cx.debug_bounds("cli-launcher").unwrap());
            let key_after = cx.debug_bounds("key-Ctrl+A").unwrap();
            assert!(key_after.left() < key_before.left());
        }
    }
    view.update(cx, |s, cx| {
        assert!(
            s.workspace
                .split(1, super::workspace::Axis::Horizontal, 1200., 800.)
        );
        cx.notify();
    });
    add_terminal(&view, cx);
    redraw(cx);
    let compose = cx.debug_bounds("open-composer").unwrap();
    cx.simulate_click(compose.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| matches!(s.dialog, Some(Dialog::Compose(2)))));
    cx.simulate_keystrokes("escape");
    view.update(cx, |s, cx| {
        s.workspace.active = 1;
        cx.notify();
    });
    redraw(cx);
    cx.simulate_click(compose.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| matches!(s.dialog, Some(Dialog::Compose(1)))));
}

#[gpui::test]
fn composer_restores_focus_draft_selection_and_enter_option_and_blocks_terminal(
    cx: &mut TestAppContext,
) {
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    redraw(cx);
    cx.update(|w, cx| w.focus(&terminal.focus_handle(cx)));
    view.update(cx, |s, cx| {
        s.open_composer(cx);
        cx.notify();
    });
    redraw(cx);
    let input = view.read_with(cx, |s, _| s.composers[&1].input.clone());
    assert!(!view.read_with(cx, |s, _| s.composers[&1].include_enter));
    assert!(cx.update(|w, cx| input.focus_handle(cx).is_focused(w)));
    cx.simulate_input("첫 줄 😀");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("둘째 줄");
    assert_eq!(
        input.read_with(cx, |i, _| i.content.to_string()),
        "첫 줄 😀\n둘째 줄"
    );
    cx.simulate_keystrokes("shift-left shift-left");
    let selection =
        cx.update(|w, cx| input.update(cx, |i, cx| i.selected_text_range(false, w, cx).unwrap()));
    let option = cx.debug_bounds("composer-enter").unwrap();
    cx.simulate_click(option.center(), Modifiers::default());
    assert!(view.read_with(cx, |s, _| s.composers[&1].include_enter));
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    view.update(cx, |s, cx| {
        s.open_composer(cx);
        cx.notify();
    });
    redraw(cx);
    assert_eq!(
        input.entity_id(),
        view.read_with(cx, |s, _| s.composers[&1].input.entity_id())
    );
    let restored =
        cx.update(|w, cx| input.update(cx, |i, cx| i.selected_text_range(false, w, cx).unwrap()));
    assert_eq!(selection.range, restored.range);
    assert!(view.read_with(cx, |s, _| s.composers[&1].include_enter));
    cx.update(|w, cx| {
        input.update(cx, |i, cx| {
            i.replace_and_mark_text_in_range(None, "ㅎ", Some(1..1), w, cx);
        })
    });
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!input.read_with(cx, |i, _| i.is_composing()));
    view.update(cx, |s, cx| {
        s.open_composer(cx);
        cx.notify();
    });
    redraw(cx);
    let cancel = cx.debug_bounds("cancel-dialog").unwrap();
    // Even a directly focused background terminal cannot forward keys or an IME commit.
    cx.update(|w, cx| w.focus(&terminal.focus_handle(cx)));
    cx.simulate_keystrokes("ctrl-c enter");
    cx.simulate_input("blocked");
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    cx.simulate_click(cancel.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
    view.update(cx, |s, cx| {
        s.close_tab(1);
        cx.notify();
    });
    assert!(view.read_with(cx, |s, _| s.composers.is_empty()));
}

#[gpui::test]
fn composer_completion_preserves_other_drafts_and_restores_focus_on_success(
    cx: &mut TestAppContext,
) {
    use dgl_core::connection::{Completion, CompletionOutcome};
    let (view, cx) = setup(cx);
    add_terminal(&view, cx);
    let second = add_terminal(&view, cx);
    redraw(cx);
    cx.update(|w, cx| w.focus(&second.focus_handle(cx)));
    view.update(cx, |s, cx| {
        s.open_composer(cx);
        cx.notify();
    });
    redraw(cx);
    let input = view.read_with(cx, |s, _| s.composers[&2].input.clone());
    cx.simulate_input("second draft");
    view.update(cx, |s, cx| {
        let composer = s.composers.get_mut(&2).unwrap();
        composer.pending = Some(700);
        composer.input.update(cx, |i, _| i.read_only = true);
        cx.notify();
    });
    redraw(cx);
    cx.simulate_input("blocked");
    let send = cx.debug_bounds("send-composer").unwrap();
    cx.simulate_click(send.center(), Modifiers::default());
    cx.simulate_click(send.center(), Modifiers::default());
    assert_eq!(
        input.read_with(cx, |i, _| i.content.to_string()),
        "second draft"
    );
    assert_eq!(
        view.read_with(cx, |s, _| s.composers[&2].pending),
        Some(700)
    );
    cx.simulate_keystrokes("escape");
    redraw(cx);
    view.update(cx, |s, cx| {
        s.workspace.select(1, 1);
        s.open_composer(cx);
        cx.notify();
    });
    redraw(cx);
    cx.simulate_input("first draft");
    view.update(cx, |s, cx| {
        assert!(super::composer::complete(
            &mut s.composers,
            &mut s.dialog,
            2,
            &Completion {
                request_id: 700,
                outcome: CompletionOutcome::Sent,
                error: None
            },
            cx
        ));
        assert!(matches!(s.dialog, Some(Dialog::Compose(1))));
        assert!(!s.composers.contains_key(&2));
        let composer = s.composers.get_mut(&1).unwrap();
        composer.pending = Some(701);
        assert!(super::composer::complete(
            &mut s.composers,
            &mut s.dialog,
            1,
            &Completion {
                request_id: 701,
                outcome: CompletionOutcome::Failed,
                error: Some(dgl_core::Error::Network)
            },
            cx
        ));
        assert!(s.composers[&1].error.is_some());
        assert_eq!(
            s.composers[&1].input.read(cx).content.as_ref(),
            "first draft"
        );
        let failure = s.composers[&1].error.clone();
        s.dismiss_dialog(cx);
        s.open_composer(cx);
        assert_eq!(s.composers[&1].error, failure);
        s.composers.get_mut(&1).unwrap().pending = Some(702);
        assert!(super::composer::complete(
            &mut s.composers,
            &mut s.dialog,
            1,
            &Completion {
                request_id: 702,
                outcome: CompletionOutcome::Sent,
                error: None
            },
            cx
        ));
        cx.notify();
    });
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none() && s.composers.is_empty()));
    assert!(cx.update(|w, cx| second.focus_handle(cx).is_focused(w)));
}
