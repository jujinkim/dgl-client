use super::*;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;
#[cfg(windows)]
#[path = "windows.rs"]
mod windows;

struct Stage {
    sources: Vec<String>,
    dir: Arc<tempfile::TempDir>,
    transfer: Arc<Transfer>,
    start_when_ready: bool,
}
#[derive(Default)]
pub(in crate::files) struct Outgoing {
    stage: Option<Stage>,
    // Some file managers continue reading after acknowledging the native drop.
    delivered: Option<Arc<tempfile::TempDir>>,
}
impl Outgoing {
    pub(in crate::files) fn poll(&mut self, _: &mut Vec<Job>) -> bool {
        false
    }
    pub(in crate::files) fn begin(
        &mut self,
        sources: Vec<String>,
        browser: Arc<Browser>,
        runtime: Arc<tokio::runtime::Runtime>,
        jobs: &mut Vec<Job>,
        _window: &mut Window,
    ) -> Result<(), String> {
        if let Some(stage) = &mut self.stage {
            if !stage.transfer.progress().done {
                stage.start_when_ready = stage.sources == sources;
                return Ok(());
            }
            if stage.sources == sources && stage.transfer.progress().error.is_none() {
                stage.start_when_ready = true;
                return Ok(());
            }
        }
        let dir = Arc::new(
            tempfile::Builder::new()
                .prefix("dgl-file-drag-")
                .tempdir()
                .map_err(|e| e.to_string())?,
        );
        let _runtime = runtime.enter();
        let request = CopyRequest {
            sources: sources.clone(),
            destination: dir.path().to_str().ok_or(t("files.invalid_path"))?.into(),
            upload: false,
            name: None,
        };
        let transfer = browser.start(Arc::new(LocalPaths), request.clone());
        jobs.push(Job {
            request,
            transfer: transfer.clone(),
            progress: Default::default(),
        });
        self.stage = Some(Stage {
            sources,
            dir,
            transfer,
            start_when_ready: true,
        });
        Ok(())
    }
    pub(in crate::files) fn release(&mut self) {
        if let Some(stage) = &mut self.stage {
            stage.start_when_ready = false;
        }
    }
    pub(in crate::files) fn retry(&mut self, previous: &Arc<Transfer>, next: Arc<Transfer>) {
        if let Some(stage) = &mut self.stage
            && Arc::ptr_eq(&stage.transfer, previous)
        {
            stage.transfer = next;
            stage.start_when_ready = false;
        }
    }
    fn ready(&mut self) -> Option<(Vec<PathBuf>, Arc<tempfile::TempDir>)> {
        let stage = self.stage.as_mut()?;
        let p = stage.transfer.progress();
        if !stage.start_when_ready || !p.done || p.error.is_some() {
            return None;
        }
        stage.start_when_ready = false;
        let paths = stage
            .sources
            .iter()
            .map(|s| stage.dir.path().join(s.rsplit('/').next().unwrap()))
            .collect();
        Some((paths, stage.dir.clone()))
    }
}

impl Desktop {
    pub(crate) fn drive_file_drag(&mut self, w: &mut Window) {
        #[cfg(target_os = "linux")]
        for event in self
            .linux_file_drag
            .events(w, self.workspace.file_view.is_some())
        {
            match event {
                linux::DragEvent::Drop(x, y, paths, completion) => {
                    let tab = self
                        .workspace
                        .file_view
                        .and_then(|id| self.files.get_mut(&id))
                        .filter(|tab| {
                            self.dialog.is_none() && tab.bounds.get().contains(&point(px(x), px(y)))
                        });
                    let accepted = tab.is_some_and(|tab| tab.upload_paths(&paths, &self.runtime));
                    if let Some(id) = completion {
                        self.linux_file_drag.complete(id, accepted);
                    }
                }
                linux::DragEvent::Error(message) => self.snackbar.show_snackbar(message),
            }
        }
        for (&id, tab) in &mut self.files {
            if self.workspace.file_view != Some(id) || self.dialog.is_some() {
                tab.outgoing.release();
                continue;
            }
            if let Some((paths, dir)) = tab.outgoing.ready() {
                #[cfg(target_os = "linux")]
                let result = self.linux_file_drag.start(w, paths, dir);
                #[cfg(windows)]
                let result = windows::start(w, paths, dir);
                match result {
                    Ok(true) => {
                        tab.outgoing.delivered = tab.outgoing.stage.take().map(|stage| stage.dir);
                        tab.error = None;
                    }
                    Ok(false) => tab.error = Some(t("files.drag_ready").into()),
                    Err(e) => tab.error = Some(e),
                }
            }
        }
    }
}
#[cfg(target_os = "linux")]
pub(crate) use linux::LinuxDrag;
