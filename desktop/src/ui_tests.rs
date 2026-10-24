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

fn setup(cx: &mut TestAppContext) -> (Entity<Desktop>, &mut VisualTestContext) {
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
fn redraw(cx: &mut VisualTestContext) {
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
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
        terminal.read_with(cx, |t, _| t.sent.clone()),
        vec![vec![b'\t']]
    );
    terminal.update(cx, |t, _| t.sent.clear());
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
    assert!(terminal.read_with(cx, |t, _| t.sent.is_empty()));
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
    assert!(terminal.read_with(cx, |t, _| t.sent.is_empty()));
    assert_eq!(
        input(&view, cx).read_with(cx, |i, _| i.content.to_string()),
        "secret"
    );
    cx.simulate_keystrokes("escape");
    redraw(cx);
    cx.simulate_keystrokes("tab");
    assert_eq!(
        terminal.read_with(cx, |t, _| t.sent.clone()),
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
        for (normal, bypass) in [
            ("launch-Claude Code-normal", "launch-Claude Code-bypass"),
            ("launch-Codex-normal", "launch-Codex-bypass"),
        ] {
            let normal = cx.debug_bounds(normal).unwrap();
            let bypass = cx.debug_bounds(bypass).unwrap();
            assert!(
                (f32::from(normal.size.width) / f32::from(bypass.size.width) - 1.5).abs() < 0.02
            );
            cx.simulate_click(normal.center(), Modifiers::default());
            cx.simulate_click(bypass.center(), Modifiers::default());
            assert!(terminal.read_with(cx, |t, _| t.sent.is_empty()));
            assert!(view.read_with(cx, |s, _| s.launcher.opened.is_some()));
        }
        cx.simulate_keystrokes("escape");
        redraw(cx);
        assert!(view.read_with(cx, |s, _| s.launcher.opened.is_none()));
        assert!(terminal.read_with(cx, |t, _| t.sent.is_empty()));
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
