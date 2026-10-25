use super::*;
use crate::key_file::ImportTarget;

const PASSWORD: usize = 4;
const PASSPHRASE: usize = 5;
const KEY_PATH: usize = 6;
const MISSING_KEY: &str = "개인키 파일을 먼저 가져오세요";
const INVALID_KEY: &str = "개인키 형식 또는 키 암호를 확인하세요";

enum ErrorArea {
    Connection,
    Authentication,
    Storage,
}

#[derive(Clone, Default)]
pub struct Feedback {
    pub connection_error: Option<&'static str>,
    pub auth_error: Option<&'static str>,
    pub storage_error: Option<&'static str>,
    pub key_status: Option<String>,
    pub selecting: bool,
}

fn validate_connection(host: &HostProfile) -> Result<(), &'static str> {
    if host.alias.is_empty() || host.alias.encode_utf16().count() > 128 {
        return Err("별칭을 1~128자로 입력하세요.");
    }
    if host.hostname.is_empty()
        || host.hostname.len() > 253
        || host.hostname.chars().any(|c| c.is_whitespace() || c < ' ')
    {
        return Err("호스트 / IP를 공백 없이 올바르게 입력하세요.");
    }
    if host.port == 0 {
        return Err("포트는 1~65535 사이의 숫자로 입력하세요.");
    }
    if host.username.is_empty()
        || host.username.encode_utf16().count() > 128
        || host.username.chars().any(|c| c < ' ')
    {
        return Err("사용자 이름을 제어 문자 없이 1~128자로 입력하세요.");
    }
    host.validate()
        .map_err(|_| "호스트 연결 정보를 확인하세요.")
}

fn credential(
    mut saved: Credential,
    key_auth: bool,
    private_key: &str,
    password: &str,
    passphrase: &str,
) -> Result<Credential, &'static str> {
    if key_auth {
        if !private_key.is_empty() {
            saved.private_key = private_key.into();
            saved.passphrase = passphrase.into();
        } else if !passphrase.is_empty() {
            saved.passphrase = passphrase.into();
        }
        if saved.private_key.is_empty() {
            return Err(MISSING_KEY);
        }
        ssh::public_key(&saved).map_err(|_| INVALID_KEY)?;
    } else {
        if !password.is_empty() {
            saved.password = password.into();
        }
        if saved.password.is_empty() {
            return Err("비밀번호를 입력하세요.");
        }
    }
    Ok(saved)
}

impl Desktop {
    fn host_input(&self, index: usize, field: &Entity<TextInput>, cx: &App) -> Stateful<Div> {
        div()
            .on_children_prepainted(self.modal.reveal_on_focus(field.focus_handle(cx)))
            .id(("host-input", index))
            .debug_selector(move || format!("host-input-{index}"))
            .flex_shrink_0()
            .child(field.clone())
    }
    pub(super) fn edit(&mut self, original: Option<HostProfile>, cx: &mut Context<Self>) {
        let values = original
            .as_ref()
            .map(|h| {
                vec![
                    h.alias.clone(),
                    h.hostname.clone(),
                    h.port.to_string(),
                    h.username.clone(),
                ]
            })
            .unwrap_or_else(|| vec![String::new(), String::new(), "22".into(), String::new()]);
        let ssh_directory = platform::ssh_directory_placeholder();
        let labels = [
            "별칭",
            "호스트 / IP",
            "포트",
            "사용자 이름",
            "비밀번호 (편집 시 비우면 기존 유지)",
            "키 암호 (필요한 경우)",
            &ssh_directory,
        ];
        let fields = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                cx.new(|cx| {
                    TextInput::new(
                        values.get(i).map(String::as_str).unwrap_or(""),
                        label,
                        i == PASSWORD || i == PASSPHRASE,
                        cx,
                    )
                })
            })
            .collect();
        let key_auth = original
            .as_ref()
            .is_some_and(|h| h.auth == AuthKind::PrivateKey);
        let favorite = original.as_ref().is_some_and(|h| h.favorite);
        self.dialog = Some(Dialog::Host {
            original,
            fields,
            private_key: String::new(),
            key_auth,
            favorite,
            feedback: Feedback::default(),
        });
    }

    pub(super) fn save_host(&mut self, cx: &mut Context<Self>) {
        self.save_host_with(cx, storage::read_optional, storage::write);
    }

    fn save_host_with(
        &mut self,
        cx: &mut Context<Self>,
        read: impl FnOnce(&str) -> dgl_core::Result<Option<Credential>>,
        write: impl FnOnce(&str, &Credential) -> dgl_core::Result<()>,
    ) {
        let Some(Dialog::Host {
            original,
            fields,
            private_key,
            key_auth,
            favorite,
            feedback,
        }) = self.dialog.clone()
        else {
            return;
        };
        if feedback.selecting {
            return;
        }
        if let Some(Dialog::Host { feedback, .. }) = &mut self.dialog {
            feedback.connection_error = None;
            feedback.auth_error = None;
            feedback.storage_error = None;
        }
        let values: Vec<_> = fields
            .iter()
            .map(|f| f.read(cx).content.to_string())
            .collect();
        let id = original.as_ref().map(|h| h.id.clone()).unwrap_or_else(|| {
            format!(
                "host-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )
        });
        let host = HostProfile {
            id: id.clone(),
            alias: values[0].trim().into(),
            hostname: values[1].trim().into(),
            port: values[2].trim().parse().unwrap_or(0),
            username: values[3].trim().into(),
            auth: if key_auth {
                AuthKind::PrivateKey
            } else {
                AuthKind::Password
            },
            credential_ref: original
                .as_ref()
                .map(|h| h.credential_ref.clone())
                .unwrap_or(id),
            favorite,
            last_connected: original.as_ref().map(|h| h.last_connected).unwrap_or(0),
            last_session: original.as_ref().and_then(|h| h.last_session.clone()),
            last_pane: original.as_ref().and_then(|h| h.last_pane.clone()),
        };
        let result = (|| {
            validate_connection(&host).map_err(|e| (ErrorArea::Connection, e))?;
            let saved = read(&host.credential_ref).map_err(|_| (ErrorArea::Storage, "보호 저장소에서 자격 증명을 읽지 못했습니다. 저장소 접근 상태를 확인하세요."))?.unwrap_or_default();
            let credential = credential(
                saved,
                key_auth,
                &private_key,
                &values[PASSWORD],
                &values[PASSPHRASE],
            )
            .map_err(|e| (ErrorArea::Authentication, e))?;
            write(&host.credential_ref, &credential).map_err(|_| {
                (
                    ErrorArea::Storage,
                    "보호 저장소에 자격 증명을 저장하지 못했습니다. 저장소 접근 상태를 확인하세요.",
                )
            })?;
            let mut next = self.local.clone();
            next.hosts.retain(|h| h.id != host.id);
            next.hosts.push(host);
            next.save(&self.path).map_err(|_| {
                (
                    ErrorArea::Storage,
                    "호스트 설정 파일을 저장하지 못했습니다. 저장 위치와 쓰기 권한을 확인하세요.",
                )
            })?;
            self.local = next;
            Ok(())
        })();
        match result {
            Ok(()) => self.dialog = None,
            Err((area, error)) => {
                if let Some(Dialog::Host { feedback, .. }) = &mut self.dialog {
                    match area {
                        ErrorArea::Connection => feedback.connection_error = Some(error),
                        ErrorArea::Authentication => feedback.auth_error = Some(error),
                        ErrorArea::Storage => feedback.storage_error = Some(error),
                    }
                }
            }
        }
    }

    fn import_key(&mut self, choose: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Host {
            fields, feedback, ..
        }) = &mut self.dialog
        else {
            return;
        };
        if feedback.selecting {
            return;
        }
        let input = fields[KEY_PATH].read(cx).content.to_string();
        let target = key_file::target(&input, platform::user_home());
        let target = if choose {
            ImportTarget::Choose(match target {
                ImportTarget::Choose(directory) => directory,
                ImportTarget::Read(path) => path
                    .parent()
                    .filter(|p| p.is_dir())
                    .map(PathBuf::from)
                    .or_else(|| key_file::default_directory(platform::user_home())),
            })
        } else {
            target
        };
        let id = fields[KEY_PATH].entity_id();
        match target {
            ImportTarget::Read(path) => self.finish_key_selection(id, Ok(Some(path)), cx),
            ImportTarget::Choose(directory) => {
                feedback.selecting = true;
                let picker = file_picker::select(directory, window, cx);
                cx.spawn(async move |view, cx| {
                    let result = picker.await;
                    let _ = view.update(cx, |this, cx| {
                        this.finish_key_selection(id, result, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    fn finish_key_selection(
        &mut self,
        id: EntityId,
        result: file_picker::Selection,
        cx: &mut Context<Self>,
    ) {
        let Some(Dialog::Host {
            fields,
            private_key,
            feedback,
            ..
        }) = &mut self.dialog
        else {
            return;
        };
        if fields[KEY_PATH].entity_id() != id {
            return;
        }
        feedback.selecting = false;
        let path = match result {
            Ok(Some(path)) => path,
            Ok(None) => return, // Cancellation preserves all input, key, status and errors.
            Err(()) => {
                feedback.auth_error =
                    Some("파일 선택창을 열지 못했습니다. 경로를 입력해 가져오세요.");
                return;
            }
        };
        match key_file::read(&path) {
            Ok(key) => {
                *private_key = key;
                fields[KEY_PATH].update(cx, |input, cx| {
                    input.set_content(&path.to_string_lossy(), cx)
                });
                feedback.key_status = Some(format!(
                    "가져옴: {} · 저장할 때 형식과 키 암호를 확인합니다.",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                ));
                feedback.auth_error = None;
            }
            Err(error) => feedback.auth_error = Some(error),
        }
    }

    fn copy_public_key(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Host {
            original,
            private_key,
            fields,
            ..
        }) = &self.dialog
        else {
            return;
        };
        let result = (|| {
            let saved = match original {
                Some(host) => storage::read_optional(&host.credential_ref)
                    .map_err(|_| "보호 저장소에서 자격 증명을 읽지 못했습니다.")?
                    .unwrap_or_default(),
                None => Credential::default(),
            };
            let credential = credential(
                saved,
                true,
                private_key,
                "",
                &fields[PASSPHRASE].read(cx).content,
            )?;
            ssh::public_key(&credential).map_err(|_| INVALID_KEY)
        })();
        match result {
            Ok(key) => {
                cx.write_to_clipboard(ClipboardItem::new_string(key));
                if let Some(Dialog::Host { feedback, .. }) = &mut self.dialog {
                    feedback.auth_error = None;
                }
            }
            Err(error) => {
                if let Some(Dialog::Host { feedback, .. }) = &mut self.dialog {
                    feedback.auth_error = Some(error);
                }
            }
        }
    }

    fn host_section(&self, id: &'static str, title: &'static str) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(rgb(self.palette.line))
            .rounded_md()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
    }

    fn secret_toggle(
        &self,
        index: usize,
        field: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let input = field.clone();
        self.button(
            format!("show-secret-{index}"),
            if field.read(cx).secret_visible() {
                "☑ 비밀번호 표시"
            } else {
                "☐ 비밀번호 표시"
            },
            cx,
            move |_, _, cx| {
                input.update(cx, |input, cx| input.toggle_secret_visibility(cx));
            },
        )
    }

    pub(super) fn render_host_dialog(&self, dialog: Dialog, cx: &mut Context<Self>) -> Div {
        let Dialog::Host {
            original,
            fields,
            key_auth,
            favorite,
            feedback,
            ..
        } = dialog
        else {
            unreachable!()
        };
        let mut connection = self.host_section("host-connection", "연결 정보");
        for (i, label) in ["별칭", "호스트 / IP", "포트", "사용자 이름"]
            .iter()
            .enumerate()
        {
            connection = connection
                .child(div().text_sm().child(*label))
                .child(self.host_input(i, &fields[i], cx));
        }
        connection = connection.child(self.button(
            "favorite",
            if favorite {
                "★ 즐겨찾기"
            } else {
                "☆ 즐겨찾기"
            },
            cx,
            |s, _, _| {
                if let Some(Dialog::Host { favorite, .. }) = &mut s.dialog {
                    *favorite = !*favorite;
                }
            },
        ));
        if let Some(error) = feedback.connection_error {
            connection = connection.child(self.host_message("host-connection-error", error));
        }
        let mut auth = self.host_section("host-auth", "인증");
        let mut choices = div().flex().gap_2();
        for (value, id, label) in [
            (false, "auth-password", "비밀번호"),
            (true, "auth-key", "개인키"),
        ] {
            choices = choices.child(
                self.button(
                    id,
                    if value == key_auth {
                        format!("● {label}")
                    } else {
                        format!("○ {label}")
                    },
                    cx,
                    move |s, _, _| {
                        if let Some(Dialog::Host {
                            key_auth, feedback, ..
                        }) = &mut s.dialog
                        {
                            if *key_auth != value {
                                feedback.auth_error = None;
                            }
                            *key_auth = value;
                        }
                    },
                )
                .when(value == key_auth, |b| {
                    b.bg(rgb(self.palette.selection))
                        .border_color(rgb(self.palette.accent))
                        .font_weight(FontWeight::SEMIBOLD)
                }),
            );
        }
        auth = auth.child(choices);
        if key_auth {
            auth = auth
                .child(div().text_sm().child("개인키 파일 경로"))
                .child(self.host_input(KEY_PATH, &fields[KEY_PATH], cx))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(self.button("select-key", "파일 선택…", cx, |s, w, cx| {
                            s.import_key(true, w, cx)
                        }))
                        .child(self.button(
                            "import",
                            "경로에서 가져오기",
                            cx,
                            |s, w, cx| s.import_key(false, w, cx),
                        )),
                );
            let status = if feedback.selecting {
                "파일 선택 중…".into()
            } else {
                feedback.key_status.unwrap_or_else(|| {
                    if original
                        .as_ref()
                        .is_some_and(|h| h.auth == AuthKind::PrivateKey)
                    {
                        "새 키를 가져오지 않으면 저장된 개인키를 사용합니다.".into()
                    } else {
                        "아직 개인키를 가져오지 않았습니다.".into()
                    }
                })
            };
            auth = auth.child(div().id("key-import-status").debug_selector(|| "key-import-status".into()).text_sm().text_color(rgb(self.palette.muted)).child(status))
                .child(div().text_sm().child("키 암호"))
                .child(self.host_input(PASSPHRASE, &fields[PASSPHRASE], cx))
                .child(div().text_xs().text_color(rgb(self.palette.muted)).child("암호화된 키는 키 암호를 입력하세요. 기존 키를 유지할 때 비우면 저장된 암호를 사용합니다."))
                .child(self.secret_toggle(PASSPHRASE, &fields[PASSPHRASE], cx))
                .child(div().flex().flex_wrap().gap_2()
                    .child(self.button("generate", "Ed25519 생성", cx, |s, _, cx| {
                        match ssh::generate_key() {
                            Ok(key) => if let Some(Dialog::Host { private_key, fields, feedback, .. }) = &mut s.dialog {
                                *private_key = key.private_key.clone();
                                fields[KEY_PATH].update(cx, |input, cx| input.set_content("", cx));
                                fields[PASSPHRASE].update(cx, |input, cx| input.set_content("", cx));
                                feedback.key_status = Some("Ed25519 개인키를 생성했습니다.".into());
                                feedback.auth_error = None;
                            },
                            Err(_) => if let Some(Dialog::Host { feedback, .. }) = &mut s.dialog { feedback.auth_error = Some("개인키를 생성하지 못했습니다. 다시 시도하세요."); },
                        }
                    }))
                    .child(self.button("public-key", "공개키 복사", cx, |s, _, cx| s.copy_public_key(cx))));
        } else {
            auth = auth
                .child(div().text_sm().child("비밀번호"))
                .child(self.host_input(PASSWORD, &fields[PASSWORD], cx))
                .child(self.secret_toggle(PASSWORD, &fields[PASSWORD], cx));
        }
        if let Some(error) = feedback.auth_error {
            auth = auth.child(self.host_message("host-auth-error", error));
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child(if original.is_some() {
                "호스트 편집"
            } else {
                "SSH 호스트 추가"
            }))
            .child(connection)
            .child(auth);
        if let Some(error) = feedback.storage_error {
            body = body.child(self.host_message("host-storage-error", error));
        }
        body
    }

    fn host_message(&self, id: &'static str, text: &'static str) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .p_2()
            .text_sm()
            .border_1()
            .border_color(rgb(self.palette.accent))
            .bg(rgb(self.palette.selection))
            .child(text)
    }
}

#[cfg(test)]
mod tests;
