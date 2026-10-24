use super::{Connection, TermMode, Terminal, key_sequence, mouse_sequence};
use gpui::{
    Entity, EntityInputHandler, Keystroke, Modifiers, MouseButton, MouseMoveEvent, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
};

#[test]
fn modified_keys_keep_xterm_modifiers_and_application_cursor_mode() {
    for (key, expected) in [
        ("shift-up", "\x1b[1;2A"),
        ("ctrl-left", "\x1b[1;5D"),
        ("alt-right", "\x1b[1;3C"),
        ("ctrl-shift-down", "\x1b[1;6B"),
        ("ctrl-alt-shift-home", "\x1b[1;8H"),
        ("alt-shift-end", "\x1b[1;4F"),
        ("shift-f1", "\x1b[1;2P"),
        ("ctrl-alt-f4", "\x1b[1;7S"),
        ("shift-f5", "\x1b[15;2~"),
        ("ctrl-f12", "\x1b[24;5~"),
        ("ctrl-shift-insert", "\x1b[2;6~"),
        ("alt-delete", "\x1b[3;3~"),
        ("shift-pageup", "\x1b[5;2~"),
        ("ctrl-pagedown", "\x1b[6;5~"),
        ("ctrl-space", "\0"),
        ("ctrl-2", "\0"),
        ("ctrl-/", "\x1f"),
        ("ctrl-6", "\x1e"),
        ("ctrl-8", "\x7f"),
        ("ctrl-c", "\x03"),
        ("ctrl-alt-c", "\x1b\x03"),
        ("ctrl-backspace", "\x08"),
        ("alt-backspace", "\x1b\x7f"),
        ("alt-enter", "\x1b\r"),
        ("alt-space", "\x1b "),
        ("alt-shift-a", "\x1bA"),
        ("shift-tab", "\x1b[Z"),
    ] {
        let key = Keystroke::parse(key).unwrap();
        for mode in [TermMode::NONE, TermMode::APP_CURSOR] {
            assert_eq!(
                key_sequence(&key, mode).unwrap(),
                expected.as_bytes(),
                "{key:?} {mode:?}"
            );
        }
    }
    for (key, normal, application) in [
        ("up", "\x1b[A", "\x1bOA"),
        ("home", "\x1b[H", "\x1bOH"),
        ("end", "\x1b[F", "\x1bOF"),
        ("f1", "\x1bOP", "\x1bOP"),
        ("insert", "\x1b[2~", "\x1b[2~"),
    ] {
        let key = Keystroke::parse(key).unwrap();
        assert_eq!(
            key_sequence(&key, TermMode::NONE).unwrap(),
            normal.as_bytes()
        );
        assert_eq!(
            key_sequence(&key, TermMode::APP_CURSOR).unwrap(),
            application.as_bytes()
        );
    }
    for key in ["a", "shift-a", "space", "cmd-a", "한"] {
        assert_eq!(
            key_sequence(&Keystroke::parse(key).unwrap(), TermMode::NONE),
            None
        );
    }
}

#[test]
fn mouse_encodings_preserve_raw_coordinates_and_modifiers() {
    let plain = Modifiers::default();
    let mods = Modifiers {
        control: true,
        alt: true,
        ..plain
    };
    let normal = TermMode::MOUSE_REPORT_CLICK;
    let sgr = normal | TermMode::SGR_MOUSE;
    assert_eq!(
        mouse_sequence(sgr, 100, 20, 2, false, mods).unwrap(),
        b"\x1b[<26;101;21M"
    );
    assert_eq!(
        mouse_sequence(sgr, 100, 20, 2, true, mods).unwrap(),
        b"\x1b[<26;101;21m"
    );
    assert_eq!(
        mouse_sequence(normal, 100, 20, 2, false, mods).unwrap(),
        [27, 91, 77, 58, 133, 53]
    );
    assert_eq!(
        mouse_sequence(normal, 100, 20, 2, true, mods).unwrap(),
        [27, 91, 77, 59, 133, 53]
    );
    assert_eq!(
        mouse_sequence(normal | TermMode::UTF8_MOUSE, 100, 20, 2, false, mods).unwrap(),
        "\x1b[M:\u{85}5".as_bytes()
    );
    assert!(mouse_sequence(normal, 222, 222, 0, false, plain).is_some());
    assert!(mouse_sequence(normal, 223, 0, 0, false, plain).is_none());
    assert!(mouse_sequence(normal | TermMode::UTF8_MOUSE, 2015, 0, 0, false, plain).is_none());
    assert!(mouse_sequence(sgr, 499, 249, 0, false, plain).is_some());
    assert!(mouse_sequence(TermMode::SGR_MOUSE, 0, 0, 0, false, plain).is_none());
}

fn setup(
    cx: &mut TestAppContext,
) -> (
    tokio::runtime::Runtime,
    Entity<Terminal>,
    &mut VisualTestContext,
) {
    use dgl_core::model::{AuthKind, Credential, HostProfile};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _enter = runtime.enter();
    let host = HostProfile {
        id: "input-fixture".into(),
        alias: "input".into(),
        hostname: "127.0.0.1".into(),
        port: 1,
        username: "test".into(),
        auth: AuthKind::Password,
        credential_ref: "input".into(),
        favorite: false,
        last_connected: 0,
        last_session: None,
        last_pane: None,
    };
    let connection = Connection::start(host, Credential::default(), None);
    let (terminal, cx) = cx.add_window_view(|window, cx| {
        let terminal = Terminal::new(connection, 14, 5000, cx);
        window.focus(&terminal.focus);
        terminal
    });
    cx.run_until_parked();
    drop(_enter);
    (runtime, terminal, cx)
}

fn output(terminal: &Entity<Terminal>, cx: &mut VisualTestContext, bytes: &[u8]) {
    terminal.update(cx, |t, cx| {
        t.parser.advance(&mut t.term, bytes);
        cx.notify();
    });
    cx.run_until_parked();
}

fn take_sent(terminal: &Entity<Terminal>, cx: &mut VisualTestContext) -> Vec<u8> {
    terminal.update(cx, |t, _| std::mem::take(&mut t.sent).concat())
}

fn position(
    terminal: &Entity<Terminal>,
    cx: &VisualTestContext,
    column: usize,
    row: usize,
) -> Point<Pixels> {
    terminal.read_with(cx, |t, _| {
        let (w, h) = t.cell_size();
        t.bounds.origin + point(px((column as f32 + 0.5) * w), px((row as f32 + 0.5) * h))
    })
}

#[gpui::test]
fn key_events_and_ime_commit_do_not_duplicate_input(cx: &mut TestAppContext) {
    let (_runtime, terminal, cx) = setup(cx);
    cx.simulate_keystrokes("ctrl-shift-left alt-enter ctrl-space ctrl-alt-c shift-f5");
    assert_eq!(
        take_sent(&terminal, cx),
        b"\x1b[1;6D\x1b\r\0\x1b\x03\x1b[15;2~"
    );
    cx.simulate_input("A+한글");
    assert_eq!(take_sent(&terminal, cx), "A+한글".as_bytes());
    cx.update(|w, cx| {
        terminal.update(cx, |t, cx| {
            t.replace_and_mark_text_in_range(None, "한", None, w, cx)
        })
    });
    // The IME supplies its commit separately; simulate_keystrokes would inject
    // an extra newline through the test platform's plain-text fallback.
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: false,
    });
    assert!(take_sent(&terminal, cx).is_empty());
    cx.update(|w, cx| terminal.update(cx, |t, cx| t.replace_text_in_range(None, "한", w, cx)));
    assert_eq!(take_sent(&terminal, cx), "한".as_bytes());
}

#[gpui::test]
fn mouse_tracking_distinguishes_click_drag_and_hover(cx: &mut TestAppContext) {
    let (_runtime, terminal, cx) = setup(cx);
    let p = position(&terminal, cx, 0, 0);
    let q = position(&terminal, cx, 1, 0);
    let plain = Modifiers::default();
    output(&terminal, cx, b"\x1b[?1000h\x1b[?1006h");
    cx.simulate_event(MouseMoveEvent {
        position: p,
        ..Default::default()
    });
    assert!(take_sent(&terminal, cx).is_empty());
    cx.simulate_mouse_down(p, MouseButton::Right, plain);
    cx.simulate_mouse_move(q, MouseButton::Right, plain);
    cx.simulate_mouse_up(q, MouseButton::Right, plain);
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<2;1;1M\x1b[<2;2;1m");
    output(&terminal, cx, b"\x1b[?1002h");
    cx.simulate_mouse_down(p, MouseButton::Middle, plain);
    cx.simulate_mouse_move(p, MouseButton::Middle, plain);
    cx.simulate_mouse_move(q, MouseButton::Middle, plain);
    cx.simulate_mouse_up(q, MouseButton::Middle, plain);
    assert_eq!(
        take_sent(&terminal, cx),
        b"\x1b[<1;1;1M\x1b[<33;2;1M\x1b[<1;2;1m"
    );
    output(&terminal, cx, b"\x1b[?1003h");
    let mods = Modifiers {
        control: true,
        alt: true,
        ..plain
    };
    for _ in 0..2 {
        cx.simulate_event(MouseMoveEvent {
            position: p,
            modifiers: mods,
            ..Default::default()
        });
    }
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<59;1;1M");
    let shifted = Modifiers {
        shift: true,
        ..plain
    };
    cx.simulate_mouse_down(p, MouseButton::Left, shifted);
    cx.simulate_mouse_move(q, MouseButton::Left, shifted);
    cx.simulate_mouse_up(q, MouseButton::Left, shifted);
    assert!(take_sent(&terminal, cx).is_empty());
    assert!(terminal.read_with(cx, |t, _| t.term.selection.is_some()));
    cx.simulate_mouse_down(p, MouseButton::Left, plain);
    assert!(terminal.read_with(cx, |t, _| t.term.selection.is_none()));
    // Releasing outside the terminal still completes the remote press.
    cx.simulate_mouse_up(point(px(-5.), px(-5.)), MouseButton::Left, shifted);
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<0;1;1M\x1b[<4;1;1m");
    terminal.update(cx, |t, _| t.enabled = false);
    cx.simulate_event(MouseMoveEvent {
        position: q,
        ..Default::default()
    });
    assert!(take_sent(&terminal, cx).is_empty());
}

#[gpui::test]
fn wheel_keeps_magnitude_fraction_direction_and_local_override(cx: &mut TestAppContext) {
    let (_runtime, terminal, cx) = setup(cx);
    let p = position(&terminal, cx, 0, 0);
    let plain = Modifiers::default();
    output(&terminal, cx, b"\x1b[?1000h\x1b[?1006h");
    let wheel = |cx: &mut VisualTestContext, delta, modifiers| {
        cx.simulate_event(ScrollWheelEvent {
            position: p,
            delta,
            modifiers,
            touch_phase: TouchPhase::Moved,
        });
    };
    wheel(cx, ScrollDelta::Lines(point(0., 3.)), plain);
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<64;1;1M".repeat(3));
    wheel(cx, ScrollDelta::Lines(point(-2., 0.)), plain);
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<67;1;1M".repeat(2));
    wheel(cx, ScrollDelta::Pixels(point(px(0.), px(0.))), plain);
    assert!(take_sent(&terminal, cx).is_empty());
    let height = terminal.read_with(cx, |t, _| t.cell_size().1);
    for _ in 0..3 {
        wheel(
            cx,
            ScrollDelta::Pixels(point(px(0.), px(height / 4.))),
            plain,
        );
    }
    assert!(take_sent(&terminal, cx).is_empty());
    wheel(
        cx,
        ScrollDelta::Pixels(point(px(0.), px(height / 4.))),
        plain,
    );
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<64;1;1M");
    wheel(
        cx,
        ScrollDelta::Lines(point(0., -2.)),
        Modifiers {
            control: true,
            ..plain
        },
    );
    assert_eq!(take_sent(&terminal, cx), b"\x1b[<81;1;1M".repeat(2));
    output(&terminal, cx, "history\r\n".repeat(100).as_bytes());
    wheel(
        cx,
        ScrollDelta::Lines(point(0., 4.)),
        Modifiers {
            shift: true,
            ..plain
        },
    );
    assert!(take_sent(&terminal, cx).is_empty());
    assert_eq!(
        terminal.read_with(cx, |t, _| t.term.grid().display_offset()),
        4
    );
    output(&terminal, cx, b"\x1b[?1000l\x1b[?1049h\x1b[?1007h\x1b[?1h");
    wheel(cx, ScrollDelta::Lines(point(0., -2.)), plain);
    assert_eq!(take_sent(&terminal, cx), b"\x1bOB\x1bOB");
}
