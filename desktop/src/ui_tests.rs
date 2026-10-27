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

#[gpui::test]
fn snackbar_gauge_tracks_remaining_time_and_close_preserves_focus(cx: &mut TestAppContext) {
    use crate::snackbar::SnackbarDuration;
    use std::time::{Duration, Instant};
    let (view, ui) = setup(cx);
    redraw(ui);
    let focus = ui.update(|w, cx| w.focused(cx));
    view.update(ui, |s, cx| {
        s.snackbar.show_snackbar("timed");
        cx.notify();
    });
    redraw(ui);
    let track = ui.debug_bounds("snackbar-gauge-track").unwrap();
    let gauge = ui.debug_bounds("snackbar-gauge").unwrap();
    let bar = ui.debug_bounds("snackbar").unwrap();
    assert_eq!(track.size.height, px(3.));
    assert_eq!(track.bottom(), bar.bottom());
    assert!(gauge.size.width > track.size.width * 0.95);
    view.update(ui, |s, cx| {
        s.snackbar.current.as_mut().unwrap().started_at =
            Instant::now() - Duration::from_millis(2_500);
        cx.notify();
    });
    redraw(ui);
    let half = ui.debug_bounds("snackbar-gauge").unwrap().size.width;
    assert!(half > track.size.width * 0.45 && half < track.size.width * 0.55);
    let close = ui.debug_bounds("dismiss-error").unwrap();
    ui.simulate_click(close.center(), Modifiers::default());
    redraw(ui);
    assert!(view.read_with(ui, |s, _| s.snackbar.is_none()));
    assert_eq!(ui.update(|w, cx| w.focused(cx)), focus);
    view.update(ui, |s, cx| {
        s.snackbar
            .show_snackbar_with_duration("expires", SnackbarDuration::Short);
        s.snackbar.current.as_mut().unwrap().started_at = Instant::now() - Duration::from_secs(3);
        s.tick(cx);
    });
    redraw(ui);
    assert!(view.read_with(ui, |s, _| s.snackbar.is_none()));
}

#[gpui::test]
fn indefinite_snackbar_renders_without_gauge_and_can_be_closed(cx: &mut TestAppContext) {
    let (view, ui) = setup(cx);
    view.update(ui, |s, cx| {
        s.snackbar.show_snackbar_with_duration(
            "permanent",
            crate::snackbar::SnackbarDuration::Indefinite,
        );
        cx.notify();
    });
    redraw(ui);
    assert!(ui.debug_bounds("snackbar").is_some());
    assert!(ui.debug_bounds("snackbar-gauge-track").is_none());
    assert!(ui.debug_bounds("snackbar-gauge").is_none());
    let close = ui.debug_bounds("dismiss-error").unwrap();
    ui.simulate_click(close.center(), Modifiers::default());
    redraw(ui);
    assert!(view.read_with(ui, |s, _| s.snackbar.is_none()));
}

fn ordered_host(id: &str, session: Option<&str>) -> HostProfile {
    serde_json::from_value(serde_json::json!({
        "id":id, "alias":id, "hostname":"127.0.0.1", "port":1,
        "username":"fixture", "credentialRef":"fixture", "lastSession":session
    }))
    .unwrap()
}

#[gpui::test]
fn host_drag_commits_order_and_cancel_preserves_it_across_reload(cx: &mut TestAppContext) {
    let (view, ui) = setup(cx);
    view.update(ui, |s, cx| {
        s.local.hosts = vec![
            ordered_host("a", None),
            ordered_host("b", None),
            ordered_host("c", None),
        ];
        s.reconcile_hosts();
        cx.notify();
    });
    redraw(ui);
    let from = ui.debug_bounds("host-c").unwrap().center();
    let row = ui.debug_bounds("host-row-a").unwrap();
    let to = point(row.center().x, row.top() + px(3.));
    ui.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
    ui.simulate_mouse_move(
        point(from.x + px(20.), from.y),
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    ui.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(ui);
    assert!(ui.debug_bounds("host-drop-indicator").is_some());
    // Preview has not changed or persisted the order.
    view.read_with(ui, |s, _| assert_eq!(s.host_order.ids(), ["a", "b", "c"]));
    ui.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(ui);
    view.update(ui, |s, _| {
        assert_eq!(s.host_order.ids(), ["c", "a", "b"]);
        s.local.hosts.reverse();
        s.local.hosts[0].favorite = true;
        s.local.hosts[0].last_connected = i64::MAX;
        s.reconcile_hosts();
        assert!(!s.host_order.connection_started("a"));
        assert_eq!(
            super::host_order::HostOrder::load(
                &s.path.with_file_name("host-order.json"),
                &s.local.hosts
            ),
            s.host_order
        );
    });
    redraw(ui);
    assert!(
        ui.debug_bounds("host-row-c").unwrap().top() < ui.debug_bounds("host-row-a").unwrap().top()
    );
    let focus = view.read_with(ui, |s, _| s.focus.clone());
    ui.update(|w, _| w.focus(&focus));
    let from = ui.debug_bounds("host-b").unwrap().center();
    let row = ui.debug_bounds("host-row-c").unwrap();
    let to = point(row.left() + px(40.), row.top() + px(3.));
    ui.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
    ui.simulate_mouse_move(
        point(from.x + px(20.), from.y),
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    ui.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(ui);
    ui.simulate_keystrokes("escape");
    ui.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
    redraw(ui);
    view.read_with(ui, |s, _| assert_eq!(s.host_order.ids(), ["c", "a", "b"]));
    // An outside drop also leaves both memory and disk unchanged.
    let from = ui.debug_bounds("host-b").unwrap().center();
    ui.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
    ui.simulate_mouse_move(
        point(from.x + px(20.), from.y),
        gpui::MouseButton::Left,
        Modifiers::default(),
    );
    let outside = point(px(600.), px(200.));
    ui.simulate_mouse_move(outside, gpui::MouseButton::Left, Modifiers::default());
    ui.simulate_mouse_up(outside, gpui::MouseButton::Left, Modifiers::default());
    redraw(ui);
    view.read_with(ui, |s, _| {
        assert_eq!(s.host_order.ids(), ["c", "a", "b"]);
        assert_eq!(
            super::host_order::HostOrder::load(
                &s.path.with_file_name("host-order.json"),
                &s.local.hosts
            ),
            s.host_order
        );
    });
}

#[gpui::test]
fn session_clicks_reuse_tabs_across_groups_keep_entities_and_host_history(cx: &mut TestAppContext) {
    let (view, ui) = setup(cx);
    let (a1, a2, b1, terminal, connection) = view.update(ui, |s, cx| {
        // Hold the real actors in Connecting; identity must exist before any async work.
        s.runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        );
        let a = ordered_host("a", Some("$1"));
        let a_other = ordered_host("a", Some("$2"));
        let b = ordered_host("b", Some("$1"));
        s.local.hosts = vec![a.clone(), b.clone()];
        s.reconcile_hosts();
        let a1 = s.create_tab(a, Credential::default(), None, cx);
        let terminal = s.active_tab().unwrap().terminal.clone();
        let connection = s.active_tab().unwrap().connection.clone();
        let a2 = s.create_tab(a_other, Credential::default(), None, cx);
        let b1 = s.create_tab(b, Credential::default(), None, cx);
        assert!(
            s.workspace
                .split(1, super::workspace::Axis::Horizontal, 1200., 800.)
        );
        s.workspace.move_tab(a1, s.workspace.active, None);
        s.activate_tab(a2);
        (a1, a2, b1, terminal, connection)
    });
    let mut state = view.read_with(ui, |s, _| s.active_tab().unwrap().connection.state());
    state.remote = serde_json::from_value(serde_json::json!({
        "sessions":[{"id":"$1","name":"one","attached":0},{"id":"$2","name":"two","attached":0}],
        "windows":[],"panes":[]
    }))
    .unwrap();
    let owner = view.clone();
    let (preview, ui) = cx.add_window_view(|_, _| HierarchyPreview {
        desktop: owner,
        state,
        width: 272.,
        scroll: None,
    });
    for _ in 0..4 {
        for (selector, expected) in [("session-$1", a1), ("session-$2", a2)] {
            redraw(ui);
            let bounds = ui.debug_bounds(selector).unwrap();
            ui.simulate_click(bounds.center(), Modifiers::default());
            view.read_with(ui, |s, _| {
                assert_eq!(s.workspace.selected(), Some(expected));
                assert_eq!(s.workspace.active, s.workspace.group_for(expected).unwrap());
                assert_eq!(s.tabs.len(), 3);
            });
        }
    }
    view.update(ui, |s, cx| {
        s.open(ordered_host("a", None), cx);
        assert_eq!(s.workspace.selected(), Some(a2));
        s.activate_tab(a1);
        s.open(ordered_host("a", None), cx);
        assert_eq!(s.workspace.selected(), Some(a1));
        assert!(Arc::ptr_eq(
            &s.active_tab().unwrap().connection,
            &connection
        ));
        assert_eq!(
            s.active_tab().unwrap().terminal.entity_id(),
            terminal.entity_id()
        );
        let generation = connection.state().generation.connection;
        assert_eq!(
            s.select_session_tab(&connection, generation + 1, "$2", None, None, cx),
            None
        );
        s.activate_tab(b1);
    });
    redraw(ui);
    let bounds = ui.debug_bounds("session-$1").unwrap();
    ui.simulate_click(bounds.center(), Modifiers::default());
    view.read_with(ui, |s, _| assert_eq!(s.workspace.selected(), Some(b1)));
    preview.update(ui, |p, cx| {
        p.state.remote.sessions[0].name = "renamed".into();
        p.state.phase = dgl_core::connection::Phase::Failed;
        cx.notify();
    });
    redraw(ui);
    let bounds = ui.debug_bounds("session-$1").unwrap();
    ui.simulate_click(bounds.center(), Modifiers::default());
    let renamed = preview.read_with(ui, |p, _| p.state.clone());
    view.update(ui, |s, cx| {
        s.tabs
            .iter_mut()
            .find(|t| t.id == b1)
            .unwrap()
            .update_session(&renamed);
        assert_eq!(s.session_tab("a", "$1"), Some(a1));
        assert_eq!(s.session_tab("b", "$1"), Some(b1));
        assert_eq!(s.active_tab().unwrap().label(), "b · renamed");
        assert_eq!(s.tabs.len(), 3);
        let mut metadata = renamed.clone();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v2.json"))
                .unwrap();
        metadata.remote = serde_json::from_value(fixture[0]["remote"].clone()).unwrap();
        metadata.remote.sessions[0].name = "renamed".into();
        let tab = s.tabs.iter_mut().find(|t| t.id == b1).unwrap();
        let terminal_id = tab.terminal.entity_id();
        let connection = tab.connection.clone();
        tab.update_session(&metadata);
        assert_eq!(tab.label(), "b · renamed · 편집 · 작업");
        metadata.remote.windows[0].name = "Editor".into();
        metadata.remote.panes[0].title = "Building 😀\u{202e}\n".into();
        tab.update_session(&metadata);
        assert_eq!(tab.label(), "b · renamed · Editor · Building 😀");
        assert_eq!(tab.terminal.entity_id(), terminal_id);
        assert!(Arc::ptr_eq(&tab.connection, &connection));
        let mut recovering = metadata.clone();
        recovering.phase = dgl_core::connection::Phase::Reconnecting;
        recovering.remote = Default::default();
        tab.update_session(&recovering);
        assert_eq!(tab.label(), "b · renamed · Editor · Building 😀");
        let mut missing = metadata;
        missing.phase = dgl_core::connection::Phase::MissingSession;
        missing.remote.sessions.clear();
        s.tabs
            .iter_mut()
            .find(|t| t.id == b1)
            .unwrap()
            .update_session(&missing);
        assert_eq!(s.session_tab("b", "$1"), Some(b1));
        assert_eq!(
            s.active_tab().unwrap().label(),
            "b · renamed · Editor · Building 😀"
        );
        let notice: dgl_core::agent::Notice = serde_json::from_value(serde_json::json!({
            "endpoint":"fixture@127.0.0.1:1", "read":false,
            "event":{"id":"reuse", "seq":1, "server":"old", "run":"old", "actor":"parent",
                "session":"$1", "window":"@1", "pane":"%1", "agent":"Codex", "kind":"Approval",
                "event":"test", "at":0}
        }))
        .unwrap();
        let notice_id = notice.id();
        s.inbox.recent.push_back(notice);
        s.open_agent_notice(&notice_id, cx);
        assert_eq!(s.workspace.selected(), Some(a1));
        assert_eq!(s.tabs.len(), 3);
        assert!(s.inbox.recent.back().unwrap().read);
        s.close_tab(a1);
        s.open(ordered_host("a", None), cx);
        assert_eq!(s.workspace.selected(), Some(a2));
        assert_eq!(s.session_tab("a", "$1"), None);
        let reopened = s.create_tab(
            ordered_host("a", Some("$1")),
            Credential::default(),
            None,
            cx,
        );
        assert_ne!(reopened, a1);
        assert_ne!(
            s.active_tab().unwrap().terminal.entity_id(),
            terminal.entity_id()
        );
        assert_eq!(s.tabs.len(), 3);
    });
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

struct HierarchyPreview {
    desktop: Entity<Desktop>,
    state: dgl_core::connection::State,
    width: f32,
    scroll: Option<gpui::ScrollHandle>,
}
impl gpui::Render for HierarchyPreview {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{
            InteractiveElement, ParentElement, StatefulInteractiveElement, Styled,
            prelude::FluentBuilder,
        };
        let state = self.state.clone();
        let tree = self
            .desktop
            .update(cx, |s, cx| s.render_tree_state(state, cx));
        gpui::div()
            .id("hierarchy-preview")
            .w(px(self.width))
            .when_some(self.scroll.as_ref(), |d, scroll| {
                d.h(px(240.)).overflow_y_scroll().track_scroll(scroll)
            })
            .child(tree)
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
        scroll: None,
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
            assert_eq!(session_x, px(0.));
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
    preview.read_with(ui, |p, _| {
        assert!(
            super::hierarchy::session_label(&p.state.remote, &p.state.remote.sessions[0])
                .subtitle
                .contains("메타데이터 갱신 🚀")
        );
        assert!(
            super::hierarchy::window_label(&p.state.remote, &p.state.remote.windows[0])
                .subtitle
                .contains("메타데이터 갱신 🚀")
        );
    });
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
        scroll: None,
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

#[gpui::test]
fn hierarchy_recovery_preserves_row_positions_expansion_and_scroll(cx: &mut TestAppContext) {
    use dgl_core::connection::Phase;
    let (desktop, ui) = setup(cx);
    let terminal = add_terminal(&desktop, ui);
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v1.json")).unwrap();
    let mut state = desktop.read_with(ui, |s, _| s.tabs[0].connection.state());
    state.remote = serde_json::from_value(fixture["remote"].clone()).unwrap();
    // Enough cached sessions to exercise a real, nonzero scroll offset.
    for i in 3..20 {
        let mut session = state.remote.sessions[0].clone();
        session.id = format!("${i}");
        state.remote.sessions.push(session);
    }
    state.phase = Phase::Ready;
    state.session = Some("$1".into());
    state.pane = Some("%2".into());
    desktop.update(ui, |s, _| {
        s.local.preferences.show_all_hierarchy = true;
        s.expanded.insert("1/$1".into());
    });
    let scroll = gpui::ScrollHandle::default();
    let owner = desktop.clone();
    let (preview, ui) = cx.add_window_view(|_, _| HierarchyPreview {
        desktop: owner,
        state,
        width: 272.,
        scroll: Some(scroll.clone()),
    });
    let focus = ui.update(|_, cx| cx.focus_handle());
    ui.update(|w, _| w.focus(&focus));
    for show_all in [true, false] {
        desktop.update(ui, |s, _| s.local.preferences.show_all_hierarchy = show_all);
        redraw(ui);
        scroll.set_offset(point(px(0.), px(-60.)));
        redraw(ui);
        assert!(scroll.offset().y < px(0.));
        let positions =
            ["session-$1", "window-$1-@2", "session-$3"].map(|id| ui.debug_bounds(id).unwrap());
        let tree_bounds = ui.debug_bounds("session-tree").unwrap();
        let offset = scroll.offset();
        let expanded = desktop.read_with(ui, |s, _| s.expanded.clone());
        for phase in [
            Phase::Restoring,
            Phase::Ready,
            Phase::Reconnecting,
            Phase::Connecting,
            Phase::Authenticating,
            Phase::Restoring,
            Phase::Ready,
        ] {
            preview.update(ui, |p, cx| {
                p.state.phase = phase;
                cx.notify();
            });
            redraw(ui);
            assert!(ui.debug_bounds("session-list-waiting").is_none());
            assert_eq!(ui.debug_bounds("session-tree").unwrap(), tree_bounds);
            assert_eq!(
                positions,
                ["session-$1", "window-$1-@2", "session-$3"].map(|id| ui.debug_bounds(id).unwrap())
            );
            assert_eq!(scroll.offset(), offset);
            assert_eq!(desktop.read_with(ui, |s, _| s.expanded.clone()), expanded);
            assert!(ui.update(|w, _| focus.is_focused(w)));
        }
    }
    preview.update(ui, |p, cx| {
        p.state.remote = Default::default();
        p.state.phase = Phase::Ready;
        cx.notify();
    });
    redraw(ui);
    let empty_height = ui.debug_bounds("session-tree").unwrap().size.height;
    for (phase, waiting) in [
        (Phase::Connecting, true),
        (Phase::Restoring, true),
        (Phase::Reconnecting, true),
        (Phase::Ready, false),
        (Phase::Selecting, false),
    ] {
        preview.update(ui, |p, cx| {
            p.state.phase = phase;
            cx.notify();
        });
        redraw(ui);
        // GPUI retains old debug bounds for removed elements. Compare the current
        // container height to prove the hint was actually inserted or removed.
        let height = ui.debug_bounds("session-tree").unwrap().size.height;
        if waiting {
            assert!(height > empty_height);
        } else {
            assert_eq!(height, empty_height);
        }
    }
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
}

#[gpui::test]
fn status_actions_stay_at_right_with_long_status_badges_themes_and_splits(cx: &mut TestAppContext) {
    use dgl_core::agent::{Agent, AgentEvent, Kind, Notice};
    let (view, cx) = setup(cx);
    add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.tabs[0].host.alias = "아주 긴 호스트 상태 문구 🚀 ".repeat(60);
        cx.notify();
    });
    let notice = Notice {
        endpoint: "fixture".into(),
        read: false,
        event: AgentEvent {
            id: "test".into(),
            seq: 1,
            server: "server".into(),
            run: "run".into(),
            actor: "parent".into(),
            session: "$1".into(),
            window: "@1".into(),
            pane: "%1".into(),
            agent: Agent::Codex,
            kind: Kind::Approval,
            event: "PermissionRequest".into(),
            at: dgl_core::agent::now_ms(),
        },
    };
    for split in [false, true] {
        if split {
            view.update(cx, |s, cx| {
                assert!(
                    s.workspace
                        .split(1, super::workspace::Axis::Horizontal, 1200., 800.)
                );
                cx.notify();
            });
            add_terminal(&view, cx);
            view.update(cx, |s, _| s.workspace.active = 1);
        }
        for width in [420., 640., 1200.] {
            cx.simulate_resize(gpui::size(px(width), px(600.)));
            for theme in ["light", "dark"] {
                view.update(cx, |s, _| s.local.preferences.theme = theme.into());
                let mut previous = None;
                for unread in [0, 1, 12, 100, 0] {
                    view.update(cx, |s, cx| {
                        s.inbox.recent = std::iter::repeat_n(notice.clone(), unread).collect();
                        cx.notify();
                    });
                    redraw(cx);
                    let bar = cx.debug_bounds("status-bar").unwrap();
                    let status = cx.debug_bounds("connection-status").unwrap();
                    let actions = cx.debug_bounds("status-actions").unwrap();
                    let notifications = cx.debug_bounds("agent-notifications").unwrap();
                    let settings = cx.debug_bounds("settings").unwrap();
                    assert_eq!(bar.size.height, px(26.));
                    assert_eq!(bar.size.width, px(width));
                    assert_eq!(bar.bottom(), px(600.));
                    assert_eq!(actions.right(), bar.right() - px(12.));
                    assert!(status.right() <= actions.left());
                    assert!(notifications.right() <= settings.left());
                    for button in [notifications, settings] {
                        assert!(button.size.width >= px(28.));
                        assert!(button.top() >= bar.top() && button.bottom() <= bar.bottom());
                        assert!(
                            button.top() >= cx.debug_bounds("session-sidebar").unwrap().bottom()
                        );
                    }
                    let icon = cx.debug_bounds("notification-icon").unwrap();
                    if unread > 0 {
                        assert!(icon.center().x < notifications.center().x);
                        let badge = cx.debug_bounds("unread-notifications").unwrap();
                        assert!(
                            badge.left() >= notifications.left()
                                && badge.right() <= notifications.right()
                        );
                        assert!(
                            badge.top() >= notifications.top()
                                && badge.bottom() <= notifications.bottom()
                        );
                    } else {
                        // A centered bell proves the badge no longer takes layout
                        // space, even though GPUI keeps its old debug bounds.
                        assert!(
                            (f32::from(icon.center().x - notifications.center().x)).abs() <= 1.
                        );
                    }
                    let positions = (notifications, settings);
                    if let Some(previous) = previous {
                        assert_eq!(positions, previous);
                    }
                    previous = Some(positions);
                }
            }
        }
    }
}

#[gpui::test]
fn status_action_dialogs_restore_focus_and_block_terminal_input(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    redraw(cx);
    cx.update(|w, cx| w.focus(&terminal.focus_handle(cx)));
    for id in ["agent-notifications", "settings"] {
        let button = cx.debug_bounds(id).unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        redraw(cx);
        assert!(view.read_with(cx, |s, _| match id {
            "agent-notifications" => matches!(s.dialog, Some(Dialog::Notifications)),
            _ => matches!(s.dialog, Some(Dialog::Settings)),
        }));
        cx.simulate_keystrokes("a ctrl-c");
        cx.simulate_input("차단");
        cx.simulate_keystrokes("escape");
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
        assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
        // Also exercise the dialog's close button.
        cx.simulate_click(button.center(), Modifiers::default());
        redraw(cx);
        let close = cx
            .debug_bounds(if id == "agent-notifications" {
                "close-notifications"
            } else {
                "cancel-dialog"
            })
            .unwrap();
        cx.simulate_click(close.center(), Modifiers::default());
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
        assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    }
    cx.simulate_keystrokes(if cfg!(target_os = "windows") {
        "ctrl-shift-p"
    } else {
        "cmd-,"
    });
    redraw(cx);
    assert!(view.read_with(cx, |s, _| matches!(s.dialog, Some(Dialog::Settings))));
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
}

struct AgentPanelPreview {
    desktop: Entity<Desktop>,
    connection: Arc<Connection>,
    state: dgl_core::connection::State,
}

#[gpui::test]
fn notification_panel_anchors_resizes_scrolls_and_dismisses_without_background_actions(
    cx: &mut TestAppContext,
) {
    use dgl_core::agent::{Agent, AgentEvent, Kind, Notice};
    let (view, ui) = setup(cx);
    let terminal = add_terminal(&view, ui);
    redraw(ui);
    ui.update(|w, cx| w.focus(&terminal.focus_handle(cx)));
    let button = ui.debug_bounds("agent-notifications").unwrap();
    ui.simulate_click(button.center(), Modifiers::default());
    redraw(ui);
    assert!(ui.debug_bounds("notifications-empty").is_some());
    // Resizing an already open panel must use the button's new bounds immediately.
    for (width, height) in [(1200., 700.), (420., 600.), (320., 300.)] {
        ui.simulate_resize(gpui::size(px(width), px(height)));
        redraw(ui);
        let panel = ui.debug_bounds("notification-panel").unwrap();
        let button = ui.debug_bounds("agent-notifications").unwrap();
        assert_eq!(panel.size.width, px(360_f32.min(width - 16.)));
        if width >= 400. {
            assert_eq!(panel.right(), button.right());
        }
        assert_eq!(panel.bottom(), button.top() - px(8.));
        assert!(panel.left() >= px(8.) && panel.top() >= px(8.));
        assert!(panel.right() <= px(width - 8.));
    }
    ui.simulate_resize(gpui::size(px(640.), px(600.)));
    view.update(ui, |s, cx| {
        s.inbox.recent = (0..100)
            .map(|i| Notice {
                endpoint: "fixture@127.0.0.1:1".into(),
                read: false,
                event: AgentEvent {
                    id: format!("panel-{i}"),
                    seq: i,
                    server: "old-server".into(),
                    run: "run".into(),
                    actor: "parent".into(),
                    session: "$0".into(),
                    window: "@0".into(),
                    pane: "%0".into(),
                    agent: Agent::Codex,
                    kind: Kind::Approval,
                    event: "PermissionRequest".into(),
                    at: dgl_core::agent::now_ms(),
                },
            })
            .collect();
        cx.notify();
    });
    redraw(ui);
    let panel = ui.debug_bounds("notification-panel").unwrap();
    let list = ui.debug_bounds("notification-list").unwrap();
    assert!(panel.size.height <= px(540.));
    assert!(list.bottom() <= panel.bottom());
    let scroll = view.read_with(ui, |s, _| s.modal.scroll.clone());
    assert!(scroll.max_offset().height > px(0.));
    ui.simulate_event(ScrollWheelEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-500.))),
        modifiers: Modifiers::default(),
        touch_phase: TouchPhase::Moved,
    });
    redraw(ui);
    assert!(scroll.offset().y < px(0.));
    ui.simulate_keystrokes("tab shift-tab a ctrl-c");
    ui.simulate_input("차단");
    // Reclick the bell through the transparent dismissal layer.
    let bell = ui.debug_bounds("agent-notifications").unwrap();
    ui.simulate_click(bell.center(), Modifiers::default());
    redraw(ui);
    assert!(view.read_with(ui, |s, _| s.dialog.is_none()));
    assert!(ui.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    // An outside click dismisses but cannot invoke a background control.
    ui.simulate_click(bell.center(), Modifiers::default());
    redraw(ui);
    let add = ui.debug_bounds("add").unwrap();
    ui.simulate_click(add.center(), Modifiers::default());
    redraw(ui);
    assert!(view.read_with(ui, |s, _| s.dialog.is_none()));
    assert!(ui.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));
    assert!(terminal.read_with(ui, |t, cx| t.captured_inputs(cx).is_empty()));
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
        s.reconcile_hosts();
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
            tokio::time::timeout(Duration::from_secs(4), async {
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
    assert!(view.read_with(cx, |s, _| s.snackbar.is_none()));
    assert!(!connection.state().can_stop_retrying());
    let retry = cx.debug_bounds("retry-now-1").unwrap();
    cx.simulate_click(retry.center(), Modifiers::default());
    wait(|s| !s.retry.stopped && s.retry.wait_seconds.is_some());
    view.update(cx, |_, cx| cx.notify());
    redraw(cx);
    assert!(cx.debug_bounds("stop-retry-1").is_some());
    assert_eq!(connection.state().handle, handle);
    assert_eq!(connection.state().retry.attempt, 1);
    assert!(view.read_with(cx, |s, _| {
        s.tabs.len() == 1
            && s.tabs[0].id == tab
            && s.tabs[0].terminal.entity_id() == terminal.entity_id()
    }));
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    wait(|s| s.retry_exhausted());
    for width in [320., 900.] {
        cx.simulate_resize(gpui::size(px(width), px(500.)));
        view.update(cx, |_, cx| cx.notify());
        redraw(cx);
        let retry = cx.debug_bounds("retry-center-1").unwrap();
        assert!(
            retry.left() >= px(0.) && retry.right() <= px(width),
            "width={width} retry={retry:?} area={:?}",
            cx.debug_bounds("terminal-area-1")
        );
        assert!(retry.top() > px(100.) && retry.bottom() < px(500.));
    }
    view.update(cx, |s, cx| {
        assert!(
            s.workspace
                .split(1, super::workspace::Axis::Horizontal, 900., 500.)
        );
        s.workspace.active = 1;
        cx.notify();
    });
    redraw(cx);
    let area = cx.debug_bounds("terminal-area-1").unwrap();
    let retry = cx.debug_bounds("retry-center-1").unwrap();
    assert!(retry.left() >= area.left() && retry.right() <= area.right());
    assert!((retry.center().x - area.center().x).abs() <= px(1.));
    cx.simulate_click(retry.center(), Modifiers::default());
    cx.simulate_click(retry.center(), Modifiers::default());
    wait(|s| !s.retry_exhausted() && s.retry.wait_seconds == Some(1));
    assert_eq!(connection.state().retry.attempt, 1);
    assert_eq!(connection.state().handle, handle);
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
    assert!(view.read_with(cx, |s, _| {
        s.snackbar
            .current
            .as_ref()
            .unwrap()
            .request
            .message
            .contains("종료")
    }));
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
    // Settings retains its initial close button even with an editable accent field.
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
            session: None,
            session_title: None,
            pending_target: None,
        });
        cx.notify();
        terminal
    })
}
#[gpui::test]
fn accent_presets_hex_modes_restore_and_modal_input_isolation(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    let input = view.read_with(cx, |s, _| s.accent_input.clone());
    let click = |selector: &'static str, cx: &mut VisualTestContext| {
        let bounds = cx.debug_bounds(selector).unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        redraw(cx);
    };
    click("accent-preset-#E6BB24", cx);
    assert_eq!(input.read_with(cx, |i, _| i.content.to_string()), "#E6BB24");
    for mode in ["accent-system", "accent-default", "accent-custom"] {
        click(mode, cx);
    }
    view.read_with(cx, |s, _| {
        assert_eq!(s.local.preferences.accent.mode, "custom");
        assert_eq!(
            s.local.preferences.accent.custom_color.as_deref(),
            Some("#E6BB24")
        );
    });
    cx.update(|w, cx| {
        w.focus(&input.focus_handle(cx));
        input.update(cx, |i, cx| i.set_content("#invalid", cx));
    });
    cx.simulate_keystrokes("enter");
    assert!(view.read_with(cx, |s, _| s.accent_error));
    assert_eq!(
        view.read_with(cx, |s, _| s.local.preferences.accent.custom_color.clone())
            .as_deref(),
        Some("#E6BB24")
    );
    cx.update(|_, cx| input.update(cx, |i, cx| i.set_content("#ab12cd", cx)));
    redraw(cx);
    // Typing and applying must retain the entity, caret focus, and modal.
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(cx.update(|w, cx| input.focus_handle(cx).is_focused(w)));
    view.read_with(cx, |s, _| {
        assert_eq!(input.entity_id(), s.accent_input.entity_id());
        assert_eq!(
            s.local.preferences.accent.custom_color.as_deref(),
            Some("#AB12CD")
        );
        assert_eq!(
            dgl_core::model::LocalState::read(&s.path)
                .unwrap()
                .preferences
                .accent,
            s.local.preferences.accent
        );
    });
    cx.simulate_keystrokes("tab shift-tab");
    assert!(cx.update(|w, cx| input.focus_handle(cx).is_focused(w)));
    assert!(terminal.read_with(cx, |t, cx| t.captured_inputs(cx).is_empty()));
    let (path, runtime) = view.read_with(cx, |s, _| (s.path.clone(), s.runtime.clone()));
    let restored = cx.update(|_, cx| cx.new(|cx| Desktop::new(runtime, path, cx)));
    assert_eq!(
        restored.read_with(cx, |s, cx| s.accent_input.read(cx).content.to_string()),
        "#AB12CD"
    );
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.dialog.is_none()));
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
        assert!(view.read_with(cx, |s, _| s.dialog.is_none() && s.snackbar.is_none()));
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
    // Capture-only transports must stop changing phase during the toggle checks:
    // a real reconnect is supposed to clear the pending modifiers.
    let (runtime, connections) = view.read_with(cx, |s, _| {
        (
            s.runtime.clone(),
            s.tabs
                .iter()
                .map(|tab| tab.connection.clone())
                .collect::<Vec<_>>(),
        )
    });
    for connection in &connections {
        connection.close();
    }
    for connection in connections {
        runtime.block_on(connection.join());
    }
    redraw(cx);
    cx.update(|w, _| w.activate_window());
    redraw(cx);
    for key in ["key-Ctrl", "key-Alt"] {
        let point = cx.debug_bounds(key).unwrap().center();
        cx.simulate_click(point, Modifiers::default());
        redraw(cx);
    }
    cx.simulate_resize(gpui::size(px(900.), px(700.)));
    redraw(cx);
    assert!(second.read_with(cx, |t, cx| t.modifiers(cx).control && t.modifiers(cx).alt));
    cx.simulate_input("c");
    expected.push(b"\x1b\x03".to_vec());
    cx.simulate_input("c");
    expected.push(b"c".to_vec());
    assert_eq!(
        second.read_with(cx, |t, cx| t.captured_inputs(cx)),
        expected
    );
    assert_eq!(
        second.read_with(cx, |t, cx| t.modifiers(cx)),
        dgl_core::input::Modifiers::default()
    );
    let point = cx.debug_bounds("key-Ctrl").unwrap().center();
    cx.simulate_click(point, Modifiers::default());
    redraw(cx);
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    assert_eq!(
        second.read_with(cx, |t, cx| t.modifiers(cx)),
        dgl_core::input::Modifiers::default()
    );
}

#[gpui::test]
fn shortcut_settings_change_the_bar_and_survive_reload(cx: &mut TestAppContext) {
    use dgl_core::{
        input::{TerminalKey, TerminalShortcut},
        model::LocalState,
    };
    let (view, cx) = setup(cx);
    let terminal = add_terminal(&view, cx);
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Shortcuts);
        cx.notify();
    });
    redraw(cx);
    for selector in [
        "shortcut-setting-Ctrl",
        "shortcut-setting-Shift",
        "shortcut-setting-Alt",
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
                .contains(&TerminalShortcut::Key(TerminalKey::Enter))
        );
    });
    let close = cx.debug_bounds("cancel-dialog").unwrap();
    cx.simulate_click(close.center(), Modifiers::default());
    redraw(cx);
    cx.simulate_keystrokes("escape");
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
        s.dialog = Some(Dialog::Shortcuts);
        cx.notify();
    });
    redraw(cx);
    let reset = cx.debug_bounds("reset-shortcuts").unwrap();
    let body = cx.debug_bounds("dialog-body").unwrap();
    if reset.bottom() > body.bottom() {
        view.update(cx, |s, cx| {
            let offset = s.modal.scroll.offset();
            s.modal.scroll.set_offset(point(
                offset.x,
                offset.y - (reset.bottom() - body.bottom()) - px(8.),
            ));
            cx.notify();
        });
        redraw(cx);
    }
    let reset = cx.debug_bounds("reset-shortcuts").unwrap();
    cx.simulate_click(reset.center(), Modifiers::default());
    redraw(cx);
    view.read_with(cx, |s, _| {
        assert_eq!(
            LocalState::read(&s.path)
                .unwrap()
                .preferences
                .terminal_shortcuts,
            TerminalShortcut::defaults()
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
            DockEdge::Top => point(
                before.center().x,
                cx.debug_bounds("group-header-1").unwrap().bottom() + px(8.),
            ),
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
    view.update(cx, |s, _| {
        s.tabs[0].host.alias = "긴 호스트 이름".repeat(8);
        s.tabs[0].session = Some("$long-title".into());
        s.tabs[0].session_title = Some("한".repeat(20) + " 제목 뒷부분");
    });
    cx.simulate_resize(gpui::size(px(640.), px(480.)));
    redraw(cx);
    let header = cx.debug_bounds("group-header-1").unwrap();
    let title = cx.debug_bounds("tab-title-1").unwrap();
    let badge = cx.debug_bounds("tab-host-1").unwrap();
    assert!(title.bottom() <= badge.top());
    assert!(title.top() >= header.top() && badge.bottom() <= header.bottom());
    assert!(badge.size.width <= px(120.));
    assert!(view.read_with(cx, |s, _| s.tabs[0].label().ends_with("제목 뒷부분")));
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
    // This disconnected layout fixture must have a stable launch context even
    // when a retry deadline would otherwise expire between simulated clicks.
    let stop_fixture = |cx: &mut VisualTestContext| {
        let (runtime, connection) = view.read_with(cx, |s, _| {
            (
                s.runtime.clone(),
                s.active_tab().unwrap().connection.clone(),
            )
        });
        connection.close();
        runtime.block_on(connection.join());
        redraw(cx);
    };
    stop_fixture(cx);
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
        for badge in [
            "launch-Claude Code-test",
            "launch-Codex-test",
            "launch-AGY-test",
            "launch-Grok Build-test",
        ] {
            assert!(cx.debug_bounds(badge).is_none());
        }
        let longest = cx.debug_bounds("launch-Claude Code-bypass").unwrap();
        let shortest = cx.debug_bounds("launch-AGY-bypass").unwrap();
        assert!(shortest.left() < longest.left());
        assert!((menu.right() - longest.right() - px(1.)).abs() <= px(1.));
        assert!(menu.size.width < px(300.));
        for (normal, bypass) in [
            ("launch-Claude Code-normal", "launch-Claude Code-bypass"),
            ("launch-Codex-normal", "launch-Codex-bypass"),
            ("launch-AGY-normal", "launch-AGY-bypass"),
            ("launch-Grok Build-normal", "launch-Grok Build-bypass"),
        ] {
            let normal = cx.debug_bounds(normal).unwrap();
            let bypass = cx.debug_bounds(bypass).unwrap();
            assert_eq!(bypass.size.width, px(44.));
            assert_eq!(normal.right(), bypass.left());
            assert_eq!(normal.top(), bypass.top());
            assert!(normal.size.width >= px(44.) && normal.size.height >= px(44.));
            assert!(menu.contains(&normal.center()) && menu.contains(&bypass.center()));
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
        stop_fixture(cx);
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
        s.local.preferences.terminal_shortcuts = dgl_core::input::TerminalShortcut::all();
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
    let checkbox = cx.debug_bounds("composer-enter-box").unwrap();
    let label = cx.debug_bounds("composer-enter-label").unwrap();
    assert_eq!(checkbox.size, gpui::size(px(18.), px(18.)));
    assert!(checkbox.right() < label.left());
    assert_eq!(checkbox.center().y, label.center().y);
    cx.simulate_click(checkbox.center(), Modifiers::default());
    redraw(cx);
    assert!(view.read_with(cx, |s, _| s.composers[&1].include_enter));
    assert_eq!(checkbox, cx.debug_bounds("composer-enter-box").unwrap());
    assert_eq!(label, cx.debug_bounds("composer-enter-label").unwrap());
    cx.simulate_click(label.center(), Modifiers::default());
    redraw(cx);
    assert!(!view.read_with(cx, |s, _| s.composers[&1].include_enter));
    cx.simulate_keystrokes("tab shift-tab space");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
    });
    redraw(cx);
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
    use super::composer::Pending;
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
        composer.pending = Some(Pending::Text {
            request: 700,
            enter: false,
        });
        composer.input.update(cx, |i, _| i.read_only = true);
        cx.notify();
    });
    redraw(cx);
    cx.simulate_input("blocked");
    let send = cx.debug_bounds("send-composer").unwrap();
    cx.simulate_click(send.center(), Modifiers::default());
    cx.simulate_click(send.center(), Modifiers::default());
    let option = cx.debug_bounds("composer-enter").unwrap();
    cx.simulate_click(option.center(), Modifiers::default());
    for key in ["space", "enter"] {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
    assert!(!view.read_with(cx, |s, _| s.composers[&2].include_enter));
    assert_eq!(
        input.read_with(cx, |i, _| i.content.to_string()),
        "second draft"
    );
    assert_eq!(
        view.read_with(cx, |s, _| s.composers[&2].pending),
        Some(Pending::Text {
            request: 700,
            enter: false
        })
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
            &s.tabs[1],
            false,
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
        composer.pending = Some(Pending::Text {
            request: 701,
            enter: false,
        });
        assert!(super::composer::complete(
            &mut s.composers,
            &mut s.dialog,
            &s.tabs[0],
            true,
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
        s.composers.get_mut(&1).unwrap().pending = Some(Pending::Text {
            request: 702,
            enter: false,
        });
        assert!(super::composer::complete(
            &mut s.composers,
            &mut s.dialog,
            &s.tabs[0],
            true,
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

#[gpui::test]
fn shortcut_editor_drag_cancel_remove_readd_and_back_restore(cx: &mut TestAppContext) {
    use dgl_core::input::{Modifier, TerminalKey, TerminalShortcut as S};
    let (view, cx) = setup(cx);
    for theme in ["light", "dark"] {
        let original = vec![
            S::Modifier(Modifier::Ctrl),
            S::Modifier(Modifier::Shift),
            S::Key(TerminalKey::Enter),
        ];
        view.update(cx, |s, cx| {
            s.local.preferences.theme = theme.into();
            s.local.preferences.terminal_shortcuts = original.clone();
            s.dialog = Some(Dialog::Shortcuts);
            cx.notify();
        });
        redraw(cx);
        let from = cx.debug_bounds("shortcut-drag-Enter").unwrap().center();
        let row = cx.debug_bounds("shortcut-row-0").unwrap();
        let to = point(row.center().x, row.top() + px(4.));
        cx.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(from.x + px(20.), from.y),
            gpui::MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert_eq!(s.local.preferences.terminal_shortcuts, original)
        });
        cx.simulate_keystrokes("escape");
        redraw(cx);
        cx.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert_eq!(s.local.preferences.terminal_shortcuts, original)
        });
        let from = cx.debug_bounds("shortcut-drag-Enter").unwrap().center();
        cx.simulate_mouse_down(from, gpui::MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(from.x + px(20.), from.y),
            gpui::MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        cx.simulate_mouse_up(to, gpui::MouseButton::Left, Modifiers::default());
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert_eq!(
                s.local.preferences.terminal_shortcuts[0],
                S::Key(TerminalKey::Enter)
            );
            assert_eq!(
                dgl_core::model::LocalState::read(&s.path)
                    .unwrap()
                    .preferences
                    .terminal_shortcuts,
                s.local.preferences.terminal_shortcuts
            );
        });
        let remove = cx.debug_bounds("shortcut-setting-Enter").unwrap().center();
        cx.simulate_click(remove, Modifiers::default());
        redraw(cx);
        let add = cx.debug_bounds("shortcut-setting-Enter").unwrap();
        view.update(cx, |s, cx| {
            let body = s.modal.scroll.bounds();
            let mut offset = s.modal.scroll.offset();
            if add.bottom() > body.bottom() {
                offset.y -= add.bottom() - body.bottom() + px(8.);
                s.modal.scroll.set_offset(offset);
            }
            cx.notify();
        });
        redraw(cx);
        let point = cx.debug_bounds("shortcut-setting-Enter").unwrap().center();
        cx.simulate_click(point, Modifiers::default());
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert_eq!(
                s.local.preferences.terminal_shortcuts.last(),
                Some(&S::Key(TerminalKey::Enter))
            )
        });
        // Esc from detail returns to main, then the next Esc closes settings.
        cx.simulate_keystrokes("escape");
        redraw(cx);
        view.read_with(cx, |s, _| {
            assert!(matches!(s.dialog, Some(Dialog::Settings)))
        });
        cx.simulate_keystrokes("escape");
        redraw(cx);
        view.read_with(cx, |s, _| assert!(s.dialog.is_none()));
    }
    view.update(cx, |s, cx| {
        s.dialog = Some(Dialog::Settings);
        cx.notify();
    });
    redraw(cx);
    let entry = cx.debug_bounds("edit-shortcuts").unwrap();
    view.update(cx, |s, cx| {
        let body = s.modal.scroll.bounds();
        s.modal
            .scroll
            .set_offset(point(px(0.), -(entry.bottom() - body.bottom() + px(16.))));
        cx.notify();
    });
    redraw(cx);
    let saved = view.read_with(cx, |s, _| s.modal.scroll.offset());
    let point = cx.debug_bounds("edit-shortcuts").unwrap().center();
    cx.simulate_click(point, Modifiers::default());
    redraw(cx);
    cx.simulate_keystrokes("escape");
    redraw(cx);
    view.read_with(cx, |s, _| assert_eq!(s.modal.scroll.offset(), saved));
    cx.update(|w, cx| {
        view.update(cx, |s, cx| {
            assert!(s.modal.button("edit-shortcuts", cx).is_focused(w));
        })
    });
}
