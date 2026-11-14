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
                    Ok(config) => (config, "sync.waiting_check".into()),
                    Err(_) => (None, "sync.connection_read_failed_reselect".into()),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (None, "common.disconnected".into())
            }
            _ => (None, "sync.connection_read_failed".into()),
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
        self.status = "sync.stopped_local_unreadable".into();
    }
}
fn read_file(path: &std::path::Path) -> std::result::Result<Vec<u8>, &'static str> {
    let file = std::fs::File::open(path).map_err(|_| "sync.file_open_failed")?;
    let mut bytes = vec![];
    file.take((sync::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "sync.read_failed_retry")?;
    Document::decode(&bytes).map_err(|_| "sync.invalid_file")?;
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
        return Err("sync.export_to_connected_file");
    }
    let document = sync::export_settings(local).map_err(|_| "sync.export_settings_failed")?;
    sync::save_private(path, document.as_bytes()).map_err(|_| "sync.save_failed")
}
fn read_import_file(path: &std::path::Path) -> std::result::Result<(Vec<u8>, usize), &'static str> {
    let bytes = read_file(path)?;
    let preview =
        sync::import_settings(&LocalState::default(), &bytes).map_err(|_| "sync.import_failed")?;
    Ok((bytes, preview.hosts.len()))
}
fn checkpoint(
    path: &std::path::Path,
    config: &Option<Config>,
) -> std::result::Result<(), &'static str> {
    let bytes = serde_json::to_vec(config).map_err(|_| "sync.history_save_failed")?;
    sync::save_private(path, &bytes).map_err(|_| "sync.history_save_failed")
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
                    .map_err(|_| "sync.create_failed")?;
                file.write_all(json.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "sync.create_save_failed")?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                return Err("sync.folder_not_writable");
            }
        }
    }
    let mut resolutions = resolutions;
    for _ in 0..3 {
        let before = read_file(&config.file)?;
        let out = sync::exchange(config.cache.clone(), &local, &before, &resolutions)
            .map_err(|_| "sync.merge_file_failed")?;
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
                .map_err(|_| "sync.write_failed")?;
            file.write_all(out.document.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|_| "sync.write_failed")?;
        }
        return Ok((config, out.document));
    }
    Err("sync.retry_changed")
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
                                .map_err(|_| "sync.merge_failed")?;
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
                                    "sync.file_updated".into()
                                }
                                Ok(()) => "sync.conflicts_hint".into(),
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
                    self.cloud_sync.status = "sync.task_stopped".into();
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
        self.cloud_sync.status = "sync.checking_file".into();
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
            prompt: Some(t("sync.choose_folder").into()),
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
            .map_err(|_| "sync.local_save_failed")?;
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
                    TransferResult::Exported => {
                        self.cloud_sync.status = "sync.export_complete".into()
                    }
                    TransferResult::Imported(bytes, count) => {
                        if matches!(self.dialog, Some(Dialog::ExportSync)) {
                            self.cloud_sync.pending_import = Some((bytes, count));
                            self.cloud_sync.status = "sync.import_review".into();
                        } else {
                            self.cloud_sync.status = "sync.import_canceled".into();
                        }
                    }
                    TransferResult::Failed(message) => self.cloud_sync.status = message.into(),
                }
                cx.notify();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.cloud_sync.transfer = None;
                self.cloud_sync.status = "sync.interrupted".into();
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
                prompt: Some(t("sync.choose_import_file").into()),
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
                        this.cloud_sync.status = "sync.file_picker_failed".into();
                        return;
                    }
                };
                let local = this.local.clone();
                let linked = this.cloud_sync.config.as_ref().map(|c| c.file.clone());
                let (sender, receiver) = mpsc::channel();
                this.cloud_sync.transfer = Some(receiver);
                this.cloud_sync.status = "sync.processing_file".into();
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
            self.cloud_sync.status = "sync.import_canceled".into();
        }
    }
    fn confirm_settings_import(&mut self, cx: &mut Context<Self>) {
        let Some((bytes, _)) = &self.cloud_sync.pending_import else {
            return;
        };
        let result = sync::import_settings(&self.local, bytes)
            .map_err(|_| "sync.import_failed")
            .and_then(|next| self.apply_shared_settings(next, cx));
        self.cloud_sync.pending_import = None;
        self.cloud_sync.status = match result {
            Ok(()) => "sync.import_complete",
            Err(e) => e,
        }
        .into();
    }
    pub(super) fn export_sync_settings(&self, cx: &mut Context<Self>) -> Div {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        self.button("export-sync-back", t("common.back"), cx, |s, _, _| {
                            s.settings_back()
                        }),
                    )
                    .child(div().text_xl().child(t("sync.export_and_sync"))),
            )
            .child(t("sync.description"));
        if !self.cloud_sync.busy() && !self.cloud_sync.blocked {
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.button(
                        "export-settings",
                        t("sync.export_ellipsis"),
                        cx,
                        |s, _, cx| s.select_settings_file(true, cx),
                    ))
                    .child(self.button(
                        "import-settings",
                        t("sync.import_ellipsis"),
                        cx,
                        |s, _, cx| s.select_settings_file(false, cx),
                    )),
            );
        }
        if let Some((_, count)) = &self.cloud_sync.pending_import {
            body = body.child(
                div()
                    .id("import-preview")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(tr!("sync.import_confirm", count))
                    .child(t("sync.import_note"))
                    .child(
                        self.button("confirm-import", t("common.import"), cx, |s, _, cx| {
                            s.confirm_settings_import(cx)
                        }),
                    )
                    .child(
                        self.button("cancel-import", t("common.cancel"), cx, |s, _, _| {
                            s.cloud_sync.pending_import = None;
                            s.cloud_sync.status = "sync.import_canceled".into();
                        }),
                    ),
            );
        }
        body.child(div().text_lg().child(t("sync.file_sync")))
            .child(t("sync.choose_folder_hint"))
            .child(self.cloud_sync_settings(cx))
    }
    pub(super) fn cloud_sync_settings(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.cloud_sync.blocked {
            return div()
                .id("cloud-sync-blocked")
                .child(t(&self.cloud_sync.status).to_owned());
        }
        let busy = self.cloud_sync.busy();
        let mut body = div()
            .id("cloud-sync-settings")
            .flex()
            .flex_col()
            .gap_2()
            .native_tooltip(t("sync.theme_note"))
            .child(t("sync.select_same_file_hint"));
        if !busy {
            body = body.child(self.button(
                "sync-folder",
                t("sync.choose_folder_ellipsis"),
                cx,
                |s, _, cx| s.select_cloud_folder(cx),
            ));
        }
        if let Some(config) = &self.cloud_sync.config {
            body = body.child(config.file.display().to_string());
            if !busy {
                body = body
                    .child(self.button("sync-now", t("sync.sync_now"), cx, |s, _, cx| {
                        s.start_cloud_sync(vec![], false, cx)
                    }))
                    .child(
                        self.button("sync-disconnect", t("sync.disconnect"), cx, |s, _, _| {
                            match checkpoint(&s.sync_path(), &None) {
                                Ok(()) => {
                                    s.cloud_sync.config = None;
                                    s.cloud_sync.conflicts.clear();
                                    s.cloud_sync.status = "sync.disconnected_file_kept".into();
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
                body = body.child(t("sync.conflicts_title"));
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
