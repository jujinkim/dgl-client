use super::*;
use dgl_core::sync::{self, Cache, Conflict, Document, Resolution};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::{fs::OpenOptions, io::Write, sync::mpsc, time::Instant};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Provider {
    GoogleDrive,
    ICloudDrive,
}
impl Provider {
    fn label(self) -> &'static str {
        match self {
            Self::GoogleDrive => "Google Drive",
            Self::ICloudDrive => "iCloud Drive",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Config {
    provider: Provider,
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
}
impl CloudSync {
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
        |_| "파일을 열 수 없습니다. Drive 실행·오프라인 사용 설정·선택한 경로를 확인하세요.",
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
            Err(_) => return Err("선택한 폴더에 쓸 수 없습니다. Drive 폴더 권한을 확인하세요."),
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
        if self.cloud_sync.blocked || self.cloud_sync.selecting {
            return;
        }
        // Do not replace host forms while the user is editing them.
        if self
            .dialog
            .as_ref()
            .is_some_and(|d| !matches!(d, Dialog::Settings))
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
                                if out.local != self.local {
                                    out.local
                                        .save(&self.path)
                                        .map_err(|_| "받은 설정의 로컬 저장 실패")?;
                                    self.local = out.local;
                                    for tab in &self.tabs {
                                        tab.terminal.update(cx, |t, _| {
                                            t.set_font(self.local.preferences.font_size)
                                        });
                                    }
                                }
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
                                    "파일 반영 완료 · 클라우드 전송은 Drive 앱에서 확인하세요."
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
        if self.cloud_sync.blocked || self.cloud_sync.receiver.is_some() {
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
    fn select_cloud_folder(&mut self, provider: Provider, cx: &mut Context<Self>) {
        if self.cloud_sync.receiver.is_some() || self.cloud_sync.selecting {
            return;
        }
        self.cloud_sync.selecting = true;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("{} 동기화 폴더 선택", provider.label()).into()),
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
                let config = Config {
                    provider,
                    file,
                    cache,
                };
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
    pub(super) fn cloud_sync_settings(&self, cx: &mut Context<Self>) -> Div {
        if self.cloud_sync.blocked {
            return div().child(self.cloud_sync.status.clone());
        }
        let busy = self.cloud_sync.receiver.is_some() || self.cloud_sync.selecting;
        let mut body=div().flex().flex_col().gap_2().child("설정·호스트 동기화")
            .child("Google Drive 또는 iCloud Drive 앱의 동기화 폴더를 선택하세요. 모든 기기에서 같은 dgl-sync.json을 사용합니다.")
            .child("첫 연결에서는 공유 파일의 공통 설정을 적용하고 이 기기의 호스트 목록을 병합합니다.")
            .child("비밀번호·개인키·신뢰 키·매크로·OS 알림·기기별 화면 배치는 공유하지 않습니다.");
        if !busy {
            for provider in [Provider::GoogleDrive, Provider::ICloudDrive] {
                body = body.child(self.button(
                    format!("sync-{:?}", provider),
                    format!("{} 폴더 선택…", provider.label()),
                    cx,
                    move |s, _, cx| s.select_cloud_folder(provider, cx),
                ));
            }
        }
        if let Some(config) = &self.cloud_sync.config {
            body = body.child(format!(
                "{} · {}",
                config.provider.label(),
                config.file.display()
            ));
            if !busy {
                body = body
                    .child(self.button("sync-now", "지금 파일 동기화", cx, |s, _, cx| {
                        s.start_cloud_sync(vec![], false, cx)
                    }))
                    .child(self.button("sync-disconnect", "연결 해제", cx, |s, _, _| {
                        match checkpoint(&s.sync_path(), &None) {
                            Ok(()) => {
                                s.cloud_sync.config = None;
                                s.cloud_sync.conflicts.clear();
                                s.cloud_sync.status =
                                    "연결 해제됨 · 공유 파일은 유지됩니다.".into();
                            }
                            Err(e) => s.cloud_sync.status = e.into(),
                        }
                    }));
            }
        }
        body = body.child(self.cloud_sync.status.clone());
        if !busy {
            for conflict in &self.cloud_sync.conflicts {
                body = body.child("동시 수정 · 유지할 값을 선택하세요");
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
    use super::*;
    #[::core::prelude::v1::test]
    fn file_transport_preserves_identity_rejects_corruption_and_keeps_pending_changes() {
        let dir = std::env::temp_dir().join(format!("dgl-sync-test-{}", dgl_core::agent::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(sync::FILE_NAME);
        let checkpoint_path = dir.join("checkpoint.json");
        let config = Config {
            provider: Provider::GoogleDrive,
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
