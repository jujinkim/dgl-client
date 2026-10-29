use super::{
    Feedback, INVALID_KEY, KEY_PATH, MISSING_KEY, PASSPHRASE, PASSWORD, credential, focus_inputs,
};
use crate::ui_tests::{redraw, setup};
use crate::{AuthKind, Credential, Desktop, Dialog, LocalState, TextInput, ssh};
use gpui::{Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, px, size};

fn fields(view: &Entity<Desktop>, cx: &mut VisualTestContext) -> Vec<Entity<TextInput>> {
    view.read_with(cx, |s, _| match &s.dialog {
        Some(Dialog::Host { fields, .. }) => fields.clone(),
        _ => panic!("host dialog"),
    })
}

fn feedback(view: &Entity<Desktop>, cx: &mut VisualTestContext) -> Feedback {
    view.read_with(cx, |s, _| match &s.dialog {
        Some(Dialog::Host { feedback, .. }) => feedback.clone(),
        _ => panic!("host dialog"),
    })
}

fn valid_form(view: &Entity<Desktop>, cx: &mut VisualTestContext) -> Vec<Entity<TextInput>> {
    view.update(cx, |s, cx| s.edit(None, cx));
    let fields = fields(view, cx);
    for (field, value) in fields.iter().zip(["별칭", "example.com", "22", "tester"]) {
        field.update(cx, |i, cx| i.set_content(value, cx));
    }
    view.update(cx, |s, _| {
        if let Some(Dialog::Host { key_auth, .. }) = &mut s.dialog {
            *key_auth = true;
        }
    });
    fields
}

fn save_without_credentials(view: &Entity<Desktop>, cx: &mut VisualTestContext) {
    view.update(cx, |s, cx| {
        s.save_host_with(
            cx,
            |_| Ok(None),
            |_, _| panic!("invalid form must not write credentials"),
        )
    });
}

#[gpui::test]
fn key_import_cancel_errors_and_corrected_save(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let fields = valid_form(&view, cx);
    save_without_credentials(&view, cx);
    assert_eq!(feedback(&view, cx).auth_error, Some(MISSING_KEY));
    assert!(view.read_with(cx, |s, _| s.snackbar.is_none() && s.local.hosts.is_empty()));
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("한글 개인키");
    std::fs::write(&path, "invalid key").unwrap();
    let id = fields[KEY_PATH].entity_id();
    fields[KEY_PATH].update(cx, |input, cx| {
        input.set_content(&path.to_string_lossy(), cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |s, cx| s.import_key(false, window, cx));
    });
    assert_eq!(
        fields[KEY_PATH].read_with(cx, |i, _| i.content.to_string()),
        path.to_string_lossy()
    );
    assert!(
        feedback(&view, cx)
            .key_status
            .unwrap()
            .contains("한글 개인키")
    );
    save_without_credentials(&view, cx);
    assert_eq!(feedback(&view, cx).auth_error, Some(INVALID_KEY));
    let before = feedback(&view, cx);
    view.update(cx, |s, cx| {
        if let Some(Dialog::Host { feedback, .. }) = &mut s.dialog {
            feedback.selecting = true;
        }
        s.finish_key_selection(id, Ok(None), cx);
    });
    assert_eq!(feedback(&view, cx).key_status, before.key_status);
    assert_eq!(feedback(&view, cx).auth_error, before.auth_error);
    assert!(!feedback(&view, cx).selecting);
    view.update(cx, |s, cx| {
        s.finish_key_selection(id, Ok(Some(folder.path().join("missing"))), cx)
    });
    assert!(
        feedback(&view, cx)
            .auth_error
            .unwrap()
            .contains("찾을 수 없습니다")
    );
    assert!(view.read_with(cx, |s, _| matches!(&s.dialog, Some(Dialog::Host { private_key, .. }) if private_key == "invalid key")));
    assert_eq!(
        fields[KEY_PATH].read_with(cx, |i, _| i.content.to_string()),
        path.to_string_lossy()
    );
    let key = ssh::generate_key().unwrap();
    let encrypted = russh::keys::PrivateKey::from_openssh(&key.private_key)
        .unwrap()
        .encrypt(&mut rand::rng(), "synthetic passphrase")
        .unwrap()
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .unwrap()
        .to_string();
    std::fs::write(&path, &encrypted).unwrap();
    view.update(cx, |s, cx| {
        s.finish_key_selection(id, Ok(Some(path.clone())), cx)
    });
    assert!(feedback(&view, cx).auth_error.is_none());
    save_without_credentials(&view, cx);
    assert_eq!(feedback(&view, cx).auth_error, Some(INVALID_KEY));
    fields[PASSPHRASE].update(cx, |i, cx| i.set_content("synthetic passphrase", cx));
    let mut written = None;
    view.update(cx, |s, cx| {
        s.save_host_with(
            cx,
            |_| Ok(None),
            |reference, credential| {
                written = Some((reference.to_string(), credential.clone()));
                Ok(())
            },
        )
    });
    let written = written.unwrap().1;
    assert_eq!(written.private_key, encrypted);
    assert_eq!(written.passphrase, "synthetic passphrase");
    assert!(view.read_with(cx, |s, _| s.dialog.is_none() && s.local.hosts.len() == 1));
    let path = view.read_with(cx, |s, _| s.path.clone());
    assert_eq!(
        LocalState::read(&path).unwrap().hosts[0].auth,
        AuthKind::PrivateKey
    );
}

#[gpui::test]
fn connection_and_storage_errors_are_distinct_and_recoverable(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let saved = tempfile::tempdir().unwrap();
    view.update(cx, |s, _| s.path = saved.path().join("settings.json"));
    let fields = valid_form(&view, cx);
    for (index, invalid, expected, valid) in [
        (0, "", "별칭", "alias"),
        (1, "bad host", "호스트 / IP", "example.com"),
        (2, "65536", "포트", "22"),
        (3, "", "사용자 이름", "user"),
    ] {
        fields[index].update(cx, |i, cx| i.set_content(invalid, cx));
        save_without_credentials(&view, cx);
        assert!(
            feedback(&view, cx)
                .connection_error
                .unwrap()
                .contains(expected)
        );
        assert!(feedback(&view, cx).auth_error.is_none());
        fields[index].update(cx, |i, cx| i.set_content(valid, cx));
    }
    let key = ssh::generate_key().unwrap();
    view.update(cx, |s, cx| {
        if let Some(Dialog::Host { private_key, .. }) = &mut s.dialog {
            *private_key = key.private_key.clone().into();
        }
        s.save_host_with(
            cx,
            |_| Err(dgl_core::Error::Storage),
            |_, _| panic!("read failed"),
        );
    });
    assert!(
        feedback(&view, cx)
            .storage_error
            .unwrap()
            .contains("읽지 못했습니다")
    );
    view.update(cx, |s, cx| {
        s.save_host_with(cx, |_| Ok(None), |_, _| Err(dgl_core::Error::Storage))
    });
    assert!(
        feedback(&view, cx)
            .storage_error
            .unwrap()
            .contains("저장하지 못했습니다")
    );
    assert!(feedback(&view, cx).auth_error.is_none());
    let path = view.read_with(cx, |s, _| s.path.clone());
    let blocked = tempfile::tempdir().unwrap();
    view.update(cx, |s, cx| {
        s.path = blocked.path().to_path_buf();
        s.save_host_with(cx, |_| Ok(None), |_, _| Ok(()));
    });
    assert!(
        feedback(&view, cx)
            .storage_error
            .unwrap()
            .contains("설정 파일")
    );
    view.update(cx, |s, cx| {
        s.path = path;
        s.save_host_with(cx, |_| Ok(None), |_, _| Ok(()));
    });
    assert!(
        view.read_with(cx, |s, _| s.dialog.is_none()),
        "save error: {:?}",
        feedback(&view, cx).storage_error
    );
}

#[test]
fn encrypted_key_validation_and_existing_credential_preservation() {
    let generated = ssh::generate_key().unwrap();
    let key = russh::keys::PrivateKey::from_openssh(&generated.private_key).unwrap();
    let encrypted = key
        .encrypt(&mut rand::rng(), "synthetic passphrase")
        .unwrap()
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .unwrap()
        .to_string();
    assert_eq!(
        credential(Credential::default(), true, &encrypted, "", "").unwrap_err(),
        INVALID_KEY
    );
    assert_eq!(
        credential(Credential::default(), true, &encrypted, "", "wrong").unwrap_err(),
        INVALID_KEY
    );
    let saved = credential(
        Credential::default(),
        true,
        &encrypted,
        "",
        "synthetic passphrase",
    )
    .unwrap();
    let kept = credential(saved.clone(), true, "", "", "").unwrap();
    assert_eq!(kept.private_key, saved.private_key);
    assert_eq!(kept.passphrase, saved.passphrase);
    let replacement = credential(saved, true, &generated.private_key, "", "").unwrap();
    assert!(replacement.passphrase.is_empty());
    let password = credential(replacement, false, "", "password draft", "").unwrap();
    let password = credential(password, false, "", "", "").unwrap();
    assert_eq!(password.password, "password draft");
    assert_eq!(password.private_key, generated.private_key);
}

#[gpui::test]
fn edit_save_keeps_existing_key_and_ignores_stale_picker(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let old_fields = valid_form(&view, cx);
    let key = ssh::generate_key().unwrap();
    view.update(cx, |s, cx| {
        if let Some(Dialog::Host { private_key, .. }) = &mut s.dialog {
            *private_key = key.private_key.clone().into();
        }
        s.save_host_with(cx, |_| Ok(None), |_, _| Ok(()));
        let mut original = s.local.hosts[0].clone();
        original.last_connected = 123;
        original.last_session = Some("$7".into());
        original.last_pane = Some("%9".into());
        s.edit(Some(original), cx);
        s.finish_key_selection(old_fields[KEY_PATH].entity_id(), Err(()), cx);
    });
    assert!(feedback(&view, cx).auth_error.is_none());
    let fields = fields(&view, cx);
    fields[0].update(cx, |i, cx| i.set_content("edited", cx));
    let mut written = None;
    view.update(cx, |s, cx| {
        s.save_host_with(
            cx,
            |_| Ok(Some(key.clone())),
            |_, credential| {
                written = Some(credential.clone());
                Ok(())
            },
        )
    });
    assert_eq!(written.unwrap().private_key, key.private_key);
    let host = view.read_with(cx, |s, _| s.local.hosts[0].clone());
    assert_eq!(host.alias, "edited");
    assert_eq!(host.last_connected, 123);
    assert_eq!(host.last_session.as_deref(), Some("$7"));
    assert_eq!(host.last_pane.as_deref(), Some("%9"));
}

#[gpui::test]
fn auth_switch_preserves_drafts_and_small_themed_form_scrolls_to_focus(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    cx.simulate_resize(size(px(600.), px(900.)));
    let fields = valid_form(&view, cx);
    fields[PASSWORD].update(cx, |i, cx| i.set_content("password draft", cx));
    fields[PASSPHRASE].update(cx, |i, cx| i.set_content("암호 draft", cx));
    fields[KEY_PATH].update(cx, |i, cx| i.set_content("/한글 경로/id_ed25519", cx));
    view.update(cx, |s, cx| {
        if let Some(Dialog::Host { private_key, .. }) = &mut s.dialog {
            *private_key = "key draft".into();
        }
        cx.notify();
    });
    redraw(cx);
    for id in ["auth-password", "auth-key", "auth-password", "auth-key"] {
        let button = cx.debug_bounds(id).unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        redraw(cx);
    }
    assert_eq!(
        fields[PASSWORD].read_with(cx, |i, _| i.content.to_string()),
        "password draft"
    );
    assert_eq!(
        fields[PASSPHRASE].read_with(cx, |i, _| i.content.to_string()),
        "암호 draft"
    );
    assert_eq!(
        fields[KEY_PATH].read_with(cx, |i, _| i.content.to_string()),
        "/한글 경로/id_ed25519"
    );
    assert!(view.read_with(cx, |s, _| matches!(&s.dialog, Some(Dialog::Host { private_key, key_auth: true, .. }) if private_key == "key draft")));
    for theme in ["light", "dark"] {
        view.update(cx, |s, cx| {
            s.local.preferences.theme = theme.into();
            cx.notify();
        });
        cx.simulate_resize(size(px(420.), px(440.)));
        redraw(cx);
        let connection = cx.debug_bounds("host-connection").unwrap();
        let auth = cx.debug_bounds("host-auth").unwrap();
        assert!(connection.bottom() < auth.top());
        assert!(auth.right() <= px(420.) && auth.left() >= px(0.));
        let save = cx.debug_bounds("save-host").unwrap();
        assert!(save.bottom() <= px(440.));
        cx.update(|window, cx| window.focus(&fields[3].focus_handle(cx)));
        redraw(cx);
        cx.simulate_keystrokes("tab");
        redraw(cx);
        redraw(cx);
        assert!(cx.update(|window, cx| fields[KEY_PATH].focus_handle(cx).is_focused(window)));
        cx.simulate_keystrokes("tab");
        redraw(cx);
        redraw(cx);
        assert!(cx.update(|window, cx| fields[PASSPHRASE].focus_handle(cx).is_focused(window)));
        let body = cx.debug_bounds("dialog-body").unwrap();
        let input = cx.debug_bounds("host-input-5").unwrap();
        assert!(
            input.top() >= body.top() && input.bottom() <= body.bottom(),
            "focused input outside viewport: {input:?}, {body:?}, scroll={:?}",
            view.read_with(cx, |s, _| (
                s.modal.scroll.offset(),
                s.modal.scroll.max_offset()
            ))
        );
        assert!(view.read_with(cx, |s, _| s.modal.scroll.offset().y < px(0.)));
        cx.simulate_resize(size(px(420.), px(340.)));
        redraw(cx);
        redraw(cx);
        let body = cx.debug_bounds("dialog-body").unwrap();
        let input = cx.debug_bounds("host-input-5").unwrap();
        assert!(input.top() >= body.top() && input.bottom() <= body.bottom());
    }
}

#[gpui::test]
fn render_shares_the_private_key_and_closing_releases_it(cx: &mut TestAppContext) {
    let (view, cx) = setup(cx);
    let fields = valid_form(&view, cx);
    let ids =
        |inputs: Vec<Entity<TextInput>>| inputs.iter().map(|i| i.entity_id()).collect::<Vec<_>>();
    let expected = |slots: &[usize]| {
        slots
            .iter()
            .map(|&i| fields[i].entity_id())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(focus_inputs(&fields, true)),
        expected(&[0, 1, 2, 3, KEY_PATH, PASSPHRASE])
    );
    assert_eq!(
        ids(focus_inputs(&fields, false)),
        expected(&[0, 1, 2, 3, PASSWORD])
    );
    let key = ssh::generate_key().unwrap();
    view.update(cx, |s, cx| {
        if let Some(Dialog::Host { private_key, .. }) = &mut s.dialog {
            *private_key = key.private_key.clone().into();
        }
        cx.notify();
    });
    redraw(cx);
    redraw(cx);
    let released = view.read_with(cx, |s, _| match &s.dialog {
        Some(Dialog::Host { private_key, .. }) => {
            assert_eq!(std::rc::Rc::strong_count(&private_key.0), 1);
            std::rc::Rc::downgrade(&private_key.0)
        }
        _ => panic!("host dialog"),
    });
    view.update(cx, |s, cx| s.dismiss_dialog(cx));
    redraw(cx);
    assert!(released.upgrade().is_none());
}
