use super::*;
use dgl_core::sync::{self, Cache, Conflict, Document, Resolution};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::{fs::OpenOptions, io::Write, sync::mpsc, time::Instant};

#[derive(Clone, Serialize, Deserialize)]
struct Config {
    file: PathBuf,
    cache: Option<Cache>,
}
type Completion = std::result::Result<(Config, String), &'static str>;
pub struct CloudSync {
    config: Option<Config>,
    receiver: Option<mpsc::Receiver<Completion>>,
    next: Instant,
    status: String,
    conflicts: Vec<Conflict>,
    blocked: bool,
    selecting: bool,
    transfer: Option<mpsc::Receiver<TransferResult>>,
    pending_import: Option<(Vec<u8>, usize)>,
}
enum TransferResult {
    Exported,
    Imported(Vec<u8>, usize),
    Failed(&'static str),
}
impl CloudSync {
    fn busy(&self) -> bool {
        self.receiver.is_some()
            || self.selecting
            || self.transfer.is_some()
            || self.pending_import.is_some()
    }
    pub fn load(path: &std::path::Path) -> Self {
        let (config, status) = match std::fs::read(path) {
            Ok(bytes) if bytes.len() <= sync::MAX_BYTES * 2 => {
                match serde_json::from_slice::<Option<Config>>(&bytes) {
                    Ok(config) => (config, "연결한 파일 확인 대기".into()),
                    Err(_) => (
                        None,
                        "동기화 연결 정보를 읽지 못했습니다. 파일을 다시 선택하세요.".into(),
                    ),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, "연결 안 됨".into()),
            _ => (None, "동기화 연결 정보를 읽지 못했습니다.".into()),
        };
        Self {
            config,
            receiver: None,
            next: Instant::now(),
            status,
            conflicts: vec![],
            blocked: false,
            selecting: false,
            transfer: None,
            pending_import: None,
        }
    }
    pub fn block_invalid_local(&mut self) {
        self.blocked = true;
        self.status =
            "로컬 설정을 읽지 못해 동기화를 중지했습니다. 설정 파일 복구 후 앱을 다시 실행하세요."
                .into();
    }
}
fn read_file(path: &std::path::Path) -> std::result::Result<Vec<u8>, &'static str> {
    let file = std::fs::File::open(path).map_err(
        |_| "파일을 열 수 없습니다. 동기화 앱·오프라인 사용 설정·선택한 경로를 확인하세요.",
    )?;
    let mut bytes = vec![];
    file.take((sync::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "파일을 읽지 못했습니다. 다음 동기화에서 재시도합니다.")?;
    Document::decode(&bytes).map_err(
        |_| "공유 파일이 비어 있거나 형식·버전이 올바르지 않습니다. 원본을 보존했습니다.",
    )?;
    Ok(bytes)
}
fn export_file(
    path: &std::path::Path,
    linked: Option<&std::path::Path>,
    local: &LocalState,
) -> std::result::Result<(), &'static str> {
    if linked.is_some_and(|linked| {
        path == linked
            || path
                .canonicalize()
                .ok()
                .zip(linked.canonicalize().ok())
                .is_some_and(|(a, b)| a == b)
    }) {
        return Err("연결 중인 동기화 파일에는 내보낼 수 없습니다. 다른 파일을 선택하세요.");
    }
    let document = sync::export_settings(local).map_err(|_| "설정을 내보내지 못했습니다.")?;
    sync::save_private(path, document.as_bytes())
        .map_err(|_| "파일을 저장하지 못했습니다. 저장 위치와 권한을 확인하세요.")
}
fn read_import_file(path: &std::path::Path) -> std::result::Result<(Vec<u8>, usize), &'static str> {
    let bytes = read_file(path)?;
    let preview = sync::import_settings(&LocalState::default(), &bytes)
        .map_err(|_| "가져오기 실패. 파일 형식·버전·크기·미해결 충돌을 확인하세요.")?;
    Ok((bytes, preview.hosts.len()))
}
fn checkpoint(
    path: &std::path::Path,
    config: &Option<Config>,
) -> std::result::Result<(), &'static str> {
    let bytes = serde_json::to_vec(config).map_err(|_| "동기화 기록 저장 실패")?;
    sync::save_private(path, &bytes).map_err(|_| "동기화 기록 저장 실패")
}
fn exchange_file(
    mut config: Config,
    local: LocalState,
    checkpoint_path: PathBuf,
    resolutions: Vec<Resolution>,
    create: bool,
) -> Completion {
    if create {
        // Explicit folder selection is the only operation allowed to create a
        // missing document. A temporarily offline/moved file is never recreated.
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config.file)
        {
            Ok(mut file) => {
                let json = Document::default()
                    .encode()
                    .map_err(|_| "새 동기화 파일 생성 실패")?;
                file.write_all(json.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "새 동기화 파일 저장 실패")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                return Err("선택한 폴더에 쓸 수 없습니다. 폴더 권한을 확인하세요.");
            }
        }
    }
    let mut resolutions = resolutions;
    for _ in 0..3 {
        let before = read_file(&config.file)?;
        let out = sync::exchange(config.cache.clone(), &local, &before, &resolutions).map_err(
            |_| "동기화 파일을 병합하지 못했습니다. 다른 동기화 파일로 교체했다면 다시 연결하세요.",
        )?;
        config.cache = Some(out.cache);
        checkpoint(&checkpoint_path, &Some(config.clone()))?;
        resolutions.clear();
        if read_file(&config.file)? != before {
            continue;
        }
        if out.document.as_bytes() != before {
            // Preserve cloud file identity (and Android's persisted document URI).
            // The durable journal above is retained even if this write fails.
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&config.file)
                .map_err(|_| "공유 파일 쓰기 실패. 변경 기록은 기기에 보존됩니다.")?;
            file.write_all(out.document.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|_| "공유 파일 쓰기 실패. 변경 기록은 기기에 보존됩니다.")?;
        }
        return Ok((config, out.document));
    }
    Err("다른 변경이 감지되어 다음 동기화에서 재시도합니다.")
}

impl Desktop {
    fn sync_path(&self) -> PathBuf {
        self.path.with_file_name("cloud-sync.json")
    }
    pub(super) fn tick_cloud_sync(&mut self, cx: &mut Context<Self>) {
        self.tick_settings_transfer(cx);
        if self.cloud_sync.blocked
            || self.cloud_sync.selecting
            || self.cloud_sync.transfer.is_some()
            || self.cloud_sync.pending_import.is_some()
        {
            return;
        }
        // Do not replace host forms while the user is editing them.
        if self
            .dialog
            .as_ref()
            .is_some_and(|d| !matches!(d, Dialog::Settings | Dialog::ExportSync))
        {
            return;
        }
        if let Some(receiver) = &self.cloud_sync.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.cloud_sync.receiver = None;
                    match result {
                        Ok((mut config, document)) => {
                            let result = (|| {
                                // Include edits made while file I/O was in flight;
                                // preserve credentials, trust and runtime state.
                                let out = sync::exchange(
                                    config.cache.clone(),
                                    &self.local,
                                    document.as_bytes(),
                                    &[],
                                )
                                .map_err(|_| "동기화 병합 실패")?;
                                config.cache = Some(out.cache);
                                checkpoint(&self.sync_path(), &Some(config.clone()))?;
                                self.apply_shared_settings(out.local, cx)?;
                                if let Some(cache) = config.cache.as_mut() {
                                    cache.acknowledge(&self.local);
                                }
                                checkpoint(&self.sync_path(), &Some(config.clone()))?;
                                self.cloud_sync.conflicts = out.conflicts;
                                self.cloud_sync.config = Some(config);
                                Ok::<_, &'static str>(())
                            })();
                            self.cloud_sync.status = match result {
                                Ok(()) if self.cloud_sync.conflicts.is_empty() => {
                                    "파일 반영 완료 · 클라우드 전송은 동기화 앱에서 확인하세요."
                                        .into()
                                }
                                Ok(()) => "동시 수정된 항목을 아래에서 선택하세요.".into(),
                                Err(e) => e.into(),
                            };
                        }
                        Err(e) => {
                            // Reload a durable pending journal after a failed write.
                            let restored = CloudSync::load(&self.sync_path());
                            self.cloud_sync.config = restored.config;
                            self.cloud_sync.status = e.into();
                        }
                    }
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.cloud_sync.receiver = None;
                    self.cloud_sync.status =
                        "동기화 작업이 종료되었습니다. 다시 시도하세요.".into();
                    cx.notify();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.cloud_sync.receiver.is_none()
            && Instant::now() >= self.cloud_sync.next
            && self.cloud_sync.config.is_some()
        {
            self.start_cloud_sync(vec![], false, cx);
        }
    }
    fn start_cloud_sync(
        &mut self,
        resolutions: Vec<Resolution>,
        create: bool,
        cx: &mut Context<Self>,
    ) {
        if self.cloud_sync.blocked || self.cloud_sync.busy() {
            return;
        }
        let Some(config) = self.cloud_sync.config.clone() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.cloud_sync.receiver = Some(receiver);
        self.cloud_sync.status = "파일 확인 중…".into();
        self.cloud_sync.next = Instant::now() + Duration::from_secs(15);
        let local = self.local.clone();
        let path = self.sync_path();
        self.runtime.spawn_blocking(move || {
            let _ = sender.send(exchange_file(config, local, path, resolutions, create));
        });
        cx.notify();
    }
    fn select_cloud_folder(&mut self, cx: &mut Context<Self>) {
        if self.cloud_sync.busy() {
            return;
        }
        self.cloud_sync.selecting = true;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t("동기화 폴더 선택").into()),
        });
        cx.spawn(async move |view, cx| {
            let result = picker.await;
            let _ = view.update(cx, |this, cx| {
                this.cloud_sync.selecting = false;
                cx.notify();
                let Ok(Ok(Some(paths))) = result else {
                    return;
                };
                let Some(folder) = paths.first() else {
                    return;
                };
                let file = folder.join(sync::FILE_NAME);
                if this.cloud_sync.receiver.is_some() {
                    return;
                }
                let cache = this
                    .cloud_sync
                    .config
                    .as_ref()
                    .filter(|old| old.file == file)
                    .and_then(|old| old.cache.clone());
                let config = Config { file, cache };
                // A provider switch starts a separate dataset; no old journal is
                // copied into a different account/folder.
                // Keep the previous checkpoint until the new file validates.
                this.cloud_sync.config = Some(config);
                this.cloud_sync.conflicts.clear();
                this.start_cloud_sync(vec![], true, cx);
            });
        })
        .detach();
    }
    fn apply_shared_settings(
        &mut self,
        next: LocalState,
        cx: &mut Context<Self>,
    ) -> std::result::Result<(), &'static str> {
        if next == self.local {
            return Ok(());
        }
        next.save(&self.path)
            .map_err(|_| "받은 설정의 로컬 저장 실패")?;
        let previous = self
            .local
            .preferences
            .accent
            .custom_color
            .as_deref()
            .unwrap_or("#2468C9");
        if self.accent_input.read(cx).content.as_ref() == previous {
            let color = next
                .preferences
                .accent
                .custom_color
                .as_deref()
                .unwrap_or("#2468C9");
            self.accent_input
                .update(cx, |i, cx| i.set_content(color, cx));
        }
        self.local = next;
        self.reconcile_hosts();
        for tab in &self.tabs {
            tab.terminal
                .update(cx, |t, _| t.set_font(self.local.preferences.font_size));
        }
        Ok(())
    }
    fn tick_settings_transfer(&mut self, cx: &mut Context<Self>) {
        let Some(receiver) = &self.cloud_sync.transfer else {
            return;
        };
        match receiver.try_recv() {
            Ok(result) => {
                self.cloud_sync.transfer = None;
                match result {
                    TransferResult::Exported => self.cloud_sync.status = "내보내기 완료".into(),
                    TransferResult::Imported(bytes, count) => {
                        if matches!(self.dialog, Some(Dialog::ExportSync)) {
                            self.cloud_sync.pending_import = Some((bytes, count));
                            self.cloud_sync.status = "가져올 파일을 확인하세요.".into();
                        } else {
                            self.cloud_sync.status = "가져오기를 취소했습니다.".into();
                        }
                    }
                    TransferResult::Failed(message) => self.cloud_sync.status = message.into(),
                }
                cx.notify();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.cloud_sync.transfer = None;
                self.cloud_sync.status = "파일 작업이 중단되었습니다.".into();
                cx.notify();
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    fn select_settings_file(&mut self, export: bool, cx: &mut Context<Self>) {
        if self.cloud_sync.busy() || self.cloud_sync.blocked {
            return;
        }
        self.cloud_sync.selecting = true;
        let directory = self.path.parent().unwrap_or(std::path::Path::new("."));
        let save_picker =
            export.then(|| cx.prompt_for_new_path(directory, Some("dgl-export.json")));
        let open_picker = (!export).then(|| {
            cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(t("가져올 설정 파일 선택").into()),
            })
        });
        cx.spawn(async move |view, cx| {
            let result = if let Some(picker) = save_picker {
                picker.await.ok()
            } else if let Some(picker) = open_picker {
                picker
                    .await
                    .ok()
                    .map(|result| result.map(|paths| paths.and_then(|p| p.into_iter().next())))
            } else {
                None
            };
            let _ = view.update(cx, |this, cx| {
                this.cloud_sync.selecting = false;
                cx.notify();
                let path = match result {
                    Some(Ok(Some(path))) => path,
                    Some(Ok(None)) => return,
                    _ => {
                        this.cloud_sync.status = "파일 선택창을 열지 못했습니다.".into();
                        return;
                    }
                };
                let local = this.local.clone();
                let linked = this.cloud_sync.config.as_ref().map(|c| c.file.clone());
                let (sender, receiver) = mpsc::channel();
                this.cloud_sync.transfer = Some(receiver);
                this.cloud_sync.status = "파일 처리 중…".into();
                this.runtime.spawn_blocking(move || {
                    let result = if export {
                        match export_file(&path, linked.as_deref(), &local) {
                            Ok(()) => TransferResult::Exported,
                            Err(e) => TransferResult::Failed(e),
                        }
                    } else {
                        match read_import_file(&path) {
                            Ok((bytes, count)) => TransferResult::Imported(bytes, count),
                            Err(e) => TransferResult::Failed(e),
                        }
                    };
                    let _ = sender.send(result);
                });
            });
        })
        .detach();
    }
    pub(super) fn cancel_import_preview(&mut self) {
        if self.cloud_sync.pending_import.take().is_some() {
            self.cloud_sync.status = "가져오기를 취소했습니다.".into();
        }
    }
    fn confirm_settings_import(&mut self, cx: &mut Context<Self>) {
        let Some((bytes, _)) = &self.cloud_sync.pending_import else {
            return;
        };
        let result = sync::import_settings(&self.local, bytes)
            .map_err(|_| "가져오기 실패. 파일 형식·버전·크기·미해결 충돌을 확인하세요.")
            .and_then(|next| self.apply_shared_settings(next, cx));
        self.cloud_sync.pending_import = None;
        self.cloud_sync.status = match result {
            Ok(()) => "가져오기 완료",
            Err(e) => e,
        }
        .into();
    }
    pub(super) fn export_sync_settings(&self, cx: &mut Context<Self>) -> Div {
        let mut body = div().flex().flex_col().gap_3()
            .child(div().flex().items_center().gap_2()
                .child(self.button("export-sync-back", t("뒤로"), cx, |s, _, _| s.settings_back()))
                .child(div().text_xl().child(t("내보내기 및 동기화"))))
            .child(t("호스트 정보와 공통 설정을 공유합니다. 비밀번호·개인키 등 민감한 정보는 내보내기와 동기화에 포함되지 않습니다."));
        if !self.cloud_sync.busy() && !self.cloud_sync.blocked {
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        self.button("export-settings", t("내보내기…"), cx, |s, _, cx| {
                            s.select_settings_file(true, cx)
                        }),
                    )
                    .child(
                        self.button("import-settings", t("가져오기…"), cx, |s, _, cx| {
                            s.select_settings_file(false, cx)
                        }),
                    ),
            );
        }
        if let Some((_, count)) = &self.cloud_sync.pending_import {
            body = body.child(div().id("import-preview").flex().flex_col().gap_2()
                .child(tr!("호스트 {0}개를 가져옵니다. 기존 목록에 병합하고 공통 설정은 파일 값으로 갱신합니다.", count))
                .child(t("파일에 없는 기존 호스트는 유지합니다. 연결 중인 동기화가 있으면 가져온 변경도 동기화됩니다."))
                .child(self.button("confirm-import", t("가져오기"), cx, |s, _, cx| s.confirm_settings_import(cx)))
                .child(self.button("cancel-import", t("취소"), cx, |s, _, _| {
                    s.cloud_sync.pending_import = None;
                    s.cloud_sync.status = "가져오기를 취소했습니다.".into();
                })));
        }
        body.child(div().text_lg().child(t("파일 동기화")))
            .child(t("Google Drive·iCloud Drive 등에서 동기화하는 폴더를 선택하세요. 실제 클라우드 전송은 해당 서비스의 앱이 처리합니다."))
            .child(self.cloud_sync_settings(cx))
    }
    pub(super) fn cloud_sync_settings(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.cloud_sync.blocked {
            return div()
                .id("cloud-sync-blocked")
                .child(t(&self.cloud_sync.status).to_owned());
        }
        let busy = self.cloud_sync.busy();
        let mut body = div().id("cloud-sync-settings").flex().flex_col().gap_2()
            .native_tooltip(t("테마·강조 색상 선택을 공유합니다. OS 실제 색상·비밀번호·개인키·신뢰 키·매크로·OS 알림·기기별 화면 배치는 공유하지 않습니다."))
            .child(t("동기화 폴더의 같은 dgl-sync.json을 선택하세요. 첫 연결 시 공통 설정을 적용하고 호스트를 병합합니다."));
        if !busy {
            body = body.child(self.button(
                "sync-folder",
                t("동기화 폴더 선택…"),
                cx,
                |s, _, cx| s.select_cloud_folder(cx),
            ));
        }
        if let Some(config) = &self.cloud_sync.config {
            body = body.child(config.file.display().to_string());
            if !busy {
                body = body
                    .child(
                        self.button("sync-now", t("지금 파일 동기화"), cx, |s, _, cx| {
                            s.start_cloud_sync(vec![], false, cx)
                        }),
                    )
                    .child(
                        self.button("sync-disconnect", t("연결 해제"), cx, |s, _, _| {
                            match checkpoint(&s.sync_path(), &None) {
                                Ok(()) => {
                                    s.cloud_sync.config = None;
                                    s.cloud_sync.conflicts.clear();
                                    s.cloud_sync.status =
                                        "연결 해제됨 · 공유 파일은 유지됩니다.".into();
                                }
                                Err(e) => s.cloud_sync.status = e.into(),
                            }
                        }),
                    );
            }
        }
        body = body.child(t(&self.cloud_sync.status).to_owned());
        if !busy {
            for conflict in &self.cloud_sync.conflicts {
                body = body.child(t("동시 수정 · 유지할 값을 선택하세요"));
                for choice in &conflict.choices {
                    let resolution = Resolution {
                        key: conflict.key.clone(),
                        revision: choice.revision.clone(),
                    };
                    body = body.child(self.button(
                        format!("sync-resolve-{}", choice.revision),
                        choice.label.clone(),
                        cx,
                        move |s, _, cx| s.start_cloud_sync(vec![resolution.clone()], false, cx),
                    ));
                }
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CloudSync, Config, Dialog, Document, Duration, LocalState, exchange_file, export_file,
        read_file, read_import_file, sync,
    };
    #[gpui::test]
    fn detail_page_sync_runs_and_failed_import_save_keeps_local_state(
        cx: &mut gpui::TestAppContext,
    ) {
        let (view, ui) = crate::ui_tests::setup(cx);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(sync::FILE_NAME);
        std::fs::write(&file, Document::default().encode().unwrap()).unwrap();
        view.update(ui, |s, cx| {
            s.dialog = Some(Dialog::ExportSync);
            s.cloud_sync.config = Some(Config { file, cache: None });
            s.tick_cloud_sync(cx);
            assert!(
                s.cloud_sync.receiver.is_some(),
                "detail page must allow automatic synchronization"
            );
        });
        // Complete the actual transport before exercising the independent import save.
        for _ in 0..1000 {
            let complete = view.update(ui, |s, cx| {
                s.tick_cloud_sync(cx);
                s.cloud_sync.receiver.is_none()
            });
            if complete {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        view.update(ui, |s, cx| {
            assert!(s.cloud_sync.receiver.is_none());
            let before = s.local.clone();
            let mut next = before.clone();
            next.preferences.theme = "light".into();
            // Renaming a settings file onto an existing directory must fail atomically.
            std::fs::remove_file(&s.path).ok();
            std::fs::create_dir_all(&s.path).unwrap();
            assert!(s.apply_shared_settings(next, cx).is_err());
            assert_eq!(s.local, before);
            std::fs::remove_dir(&s.path).unwrap();
        });
    }

    #[::core::prelude::v1::test]
    fn legacy_provider_checkpoints_preserve_connections_and_portable_files_do_not_replace_them() {
        let dir = tempfile::tempdir().unwrap();
        let linked = dir.path().join(sync::FILE_NAME);
        let checkpoint_path = dir.path().join("cloud-sync.json");
        let local = LocalState::default();
        let (config, original) = exchange_file(
            Config {
                file: linked.clone(),
                cache: None,
            },
            local.clone(),
            checkpoint_path.clone(),
            vec![],
            true,
        )
        .unwrap();
        for provider in ["GoogleDrive", "ICloudDrive"] {
            let mut legacy = serde_json::to_value(&config).unwrap();
            legacy["provider"] = provider.into();
            std::fs::write(&checkpoint_path, serde_json::to_vec(&Some(legacy)).unwrap()).unwrap();
            let loaded = CloudSync::load(&checkpoint_path).config.unwrap();
            assert_eq!(loaded.file, linked);
            assert_eq!(
                loaded.cache.unwrap().encode().unwrap(),
                config.cache.as_ref().unwrap().encode().unwrap()
            );
        }
        assert!(export_file(&linked, Some(&linked), &local).is_err());
        assert_eq!(std::fs::read_to_string(&linked).unwrap(), original);
        let exported = dir.path().join("dgl-export.json");
        export_file(&exported, Some(&linked), &local).unwrap();
        let (bytes, count) = read_import_file(&exported).unwrap();
        assert_eq!(count, 0);
        assert_eq!(sync::import_settings(&local, &bytes).unwrap(), local);
        std::fs::write(&exported, b"invalid").unwrap();
        assert!(read_import_file(&exported).is_err());
        assert_eq!(std::fs::read(&exported).unwrap(), b"invalid");
        assert!(read_import_file(&dir.path().join("missing.json")).is_err());
        assert!(export_file(dir.path(), None, &local).is_err());
        assert_eq!(std::fs::read_to_string(&linked).unwrap(), original);
    }
    #[::core::prelude::v1::test]
    fn file_transport_preserves_identity_rejects_corruption_and_keeps_pending_changes() {
        let dir = std::env::temp_dir().join(format!("dgl-sync-test-{}", dgl_core::agent::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(sync::FILE_NAME);
        let checkpoint_path = dir.join("checkpoint.json");
        let config = Config {
            file: path.clone(),
            cache: None,
        };
        let (config, doc) = exchange_file(
            config,
            LocalState::default(),
            checkpoint_path.clone(),
            vec![],
            true,
        )
        .unwrap();
        let mut local = LocalState::default();
        local.preferences.theme = "dark".into();
        let (config, next) = exchange_file(
            config,
            local.clone(),
            checkpoint_path.clone(),
            vec![],
            false,
        )
        .unwrap();
        assert_ne!(doc, next);
        assert_eq!(read_file(&path).unwrap(), next.as_bytes());
        std::fs::write(&path, b"invalid").unwrap();
        assert!(
            exchange_file(
                config.clone(),
                local.clone(),
                checkpoint_path.clone(),
                vec![],
                false
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
        std::fs::remove_file(&path).unwrap();
        assert!(exchange_file(config, local, checkpoint_path, vec![], false).is_err());
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
