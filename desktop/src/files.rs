use super::*;
use crate::workspace::FileId;
use dgl_core::sftp::{self, Browser, CopyRequest, Listing, LocalPaths, Progress, Transfer};

mod columns;
mod drag;
use columns::{BrowseAttempt, Folders, ParentColumn};
#[cfg(target_os = "linux")]
pub(crate) use drag::LinuxDrag;

pub(super) struct FileBrowser {
    host: HostProfile,
    session: Option<String>,
    pane: Option<String>,
    pub(super) title: String,
    browser: Arc<Browser>,
    focus: FocusHandle,
    folders: Folders,
    error: Option<String>,
    jobs: Vec<Job>,
    jobs_open: bool,
    outgoing: drag::Outgoing,
    pressed: Option<(String, String, Point<Pixels>)>,
    dragged: bool,
    #[cfg(target_os = "linux")]
    bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}
struct Job {
    request: CopyRequest,
    transfer: Arc<Transfer>,
    progress: Progress,
    #[cfg(target_os = "macos")]
    promise: Option<Arc<sftp::DownloadBatch>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.transfer.cancel();
    }
}
impl FileBrowser {
    fn upload_paths(&mut self, paths: &[PathBuf], runtime: &tokio::runtime::Runtime) -> bool {
        if self.folders.listing.path.is_empty() || paths.is_empty() {
            return false;
        }
        let sources: Option<Vec<_>> = paths
            .iter()
            .map(|p| p.to_str().map(str::to_owned))
            .collect();
        if let Some(sources) = sources {
            self.error = None;
            self.start(
                CopyRequest {
                    upload: true,
                    sources,
                    destination: self.folders.listing.path.clone(),
                    name: None,
                },
                runtime,
            );
            true
        } else {
            self.error = Some(t("files.invalid_path").into());
            false
        }
    }
    fn start(&mut self, request: CopyRequest, runtime: &tokio::runtime::Runtime) -> usize {
        let _guard = runtime.enter();
        let transfer = self.browser.start(Arc::new(LocalPaths), request.clone());
        self.jobs.push(Job {
            request,
            transfer,
            progress: Progress::default(),
            #[cfg(target_os = "macos")]
            promise: None,
        });
        self.jobs_open = true;
        self.jobs.len() - 1
    }
}
impl Desktop {
    pub(super) fn file_open_button(
        &self,
        host: HostProfile,
        session: Option<String>,
        pane: Option<String>,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.sidebar_menu_button(
            format!("open-files-{}-{:?}", host.id, session),
            "sFTP",
            cx,
            move |s, w, cx| {
                s.open_files(
                    host.clone(),
                    session.clone(),
                    pane.clone(),
                    name.clone(),
                    w,
                    cx,
                );
            },
        )
    }
    fn open_files(
        &mut self,
        host: HostProfile,
        session: Option<String>,
        pane: Option<String>,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        let existing = self
            .files
            .iter()
            .find(|(_, browser)| browser.host.id == host.id && browser.session == session)
            .map(|(&id, _)| id);
        let id = if let Some(id) = existing {
            id
        } else {
            let Some(credentials) = self.credentials(&host, cx) else {
                return;
            };
            let id = self.create_file_browser(host, session, pane, name, credentials, cx);
            self.browse_files(id, None, cx);
            id
        };
        self.workspace.file_view = Some(id);
        window.focus(&self.files[&id].focus);
        cx.notify();
    }
    pub(super) fn return_to_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.workspace.file_view.take()
            && let Some(tab) = self.files.get_mut(&id)
        {
            tab.pressed = None;
        }
        self.group_focus(self.workspace.active, window, cx);
    }
    pub(super) fn close_file_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.workspace.file_view {
            self.files.remove(&id);
            self.return_to_terminals(window, cx);
        }
    }
    fn create_file_browser(
        &mut self,
        host: HostProfile,
        session: Option<String>,
        pane: Option<String>,
        name: Option<String>,
        credentials: (Credential, Option<String>),
        cx: &mut Context<Self>,
    ) -> FileId {
        let (credential, trusted) = credentials;
        let id = self.next_file_id;
        self.next_file_id += 1;
        let browser = Arc::new(Browser::new(
            host.clone(),
            credential,
            trusted,
            session.clone(),
            pane.clone(),
        ));
        let title = format!(
            "SFTP · {}{}",
            host.alias,
            name.as_ref()
                .or(session.as_ref())
                .map(|n| format!(" · {n}"))
                .unwrap_or_default()
        );
        self.files.insert(
            id,
            FileBrowser {
                host,
                session,
                pane,
                title,
                browser,
                focus: cx.focus_handle(),
                folders: Folders::default(),
                error: None,
                jobs: vec![],
                jobs_open: false,
                outgoing: Default::default(),
                pressed: None,
                dragged: false,
                #[cfg(target_os = "linux")]
                bounds: Default::default(),
            },
        );
        id
    }
    pub(super) fn refresh_files(&mut self, id: FileId, cx: &mut Context<Self>) {
        if let Some(tab) = self.files.get(&id) {
            self.browse_files(id, tab.folders.refresh_path(), cx);
        }
    }
    fn browse_files(&mut self, id: FileId, path: Option<String>, cx: &mut Context<Self>) {
        let Some(tab) = self.files.get_mut(&id) else {
            return;
        };
        tab.error = None;
        let serial = tab.folders.next_request();
        let task = self.fetch_folder(id, path.clone(), serial, false, cx);
        self.files.get_mut(&id).unwrap().folders.attempt =
            Some(BrowseAttempt::loading(serial, path, task));
        cx.notify();
    }
    fn fetch_folder(
        &self,
        id: FileId,
        path: Option<String>,
        serial: u64,
        parent: bool,
        cx: &mut Context<Self>,
    ) -> tokio::task::AbortHandle {
        let browser = self.files[&id].browser.clone();
        let target = path.clone();
        let task = self
            .runtime
            .spawn(async move { browser.browse(target.as_deref()).await });
        let abort = task.abort_handle();
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = view.update(cx, |s, cx| {
                let Some(tab) = s.files.get_mut(&id) else {
                    return;
                };
                if parent {
                    tab.folders.finish_parent(path.unwrap(), serial, result);
                } else if tab.folders.finish(serial, result) {
                    s.load_parents(id, cx);
                }
                cx.notify();
            });
        })
        .detach();
        abort
    }
    fn load_parents(&mut self, id: FileId, cx: &mut Context<Self>) {
        let folders = &mut self.files.get_mut(&id).unwrap().folders;
        let requests = folders.missing_parents();
        for (path, serial) in requests {
            let task = self.fetch_folder(id, Some(path.clone()), serial, true, cx);
            self.files.get_mut(&id).unwrap().folders.parents.insert(
                path.clone(),
                ParentColumn::Pending(BrowseAttempt::loading(serial, Some(path), task)),
            );
        }
    }
    fn trust_files(&mut self, id: FileId, cx: &mut Context<Self>) {
        let tab = &self.files[&id];
        let Some(fingerprint) = tab
            .browser
            .observed()
            .filter(|_| tab.folders.host_key_unknown())
        else {
            return;
        };
        let host = tab.host.clone();
        let endpoint = host.endpoint();
        if !self.local.trust.contains_key(&endpoint) {
            if let Err(e) = storage::write(
                &storage::trust_ref(&endpoint),
                &Credential {
                    password: fingerprint.clone(),
                    private_key: String::new(),
                    passphrase: String::new(),
                },
            ) {
                self.files.get_mut(&id).unwrap().error = Some(storage::describe(&e).into());
                return;
            }
            self.local.trust.insert(endpoint, fingerprint);
            if !self.save() {
                return;
            }
        }
        let Some((cred, trust)) = self.credentials(&host, cx) else {
            return;
        };
        let tab = self.files.get_mut(&id).unwrap();
        tab.browser = Arc::new(Browser::new(
            host,
            cred,
            trust,
            tab.session.clone(),
            tab.pane.clone(),
        ));
        self.refresh_files(id, cx);
    }
    pub(super) fn tick_files(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        #[cfg(target_os = "linux")]
        {
            changed |= self.linux_file_drag.pending();
        }
        let mut refresh = Vec::new();
        for (&id, tab) in &mut self.files {
            if tab.outgoing.poll(&mut tab.jobs) {
                tab.jobs_open = true;
                changed = true;
            }
            for job in &mut tab.jobs {
                let progress = job.transfer.progress();
                if progress != job.progress {
                    if progress.done
                        && progress.error.is_none()
                        && job.request.upload
                        && tab.folders.attempt.is_none()
                        && job.request.destination == tab.folders.listing.path
                    {
                        refresh.push((id, tab.folders.listing.path.clone()));
                    }
                    tab.jobs_open |=
                        progress.conflict.is_some() && progress.conflict != job.progress.conflict;
                    job.progress = progress;
                    changed = true;
                }
            }
        }
        for (id, path) in refresh {
            self.browse_files(id, Some(path), cx);
        }
        changed
    }
    fn file_picker(&mut self, id: FileId, upload: bool, cx: &mut Context<Self>) {
        let tab = &self.files[&id];
        let destination = tab.folders.listing.path.clone();
        let sources = tab.folders.selection();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: upload,
            directories: !upload,
            multiple: upload,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let result = picker
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = view.update(cx, |s, cx| {
                let Some(tab) = s.files.get_mut(&id) else {
                    return;
                };
                match result {
                    Ok(Some(paths)) => {
                        let paths: Option<Vec<_>> = paths
                            .iter()
                            .map(|p| p.to_str().map(str::to_owned))
                            .collect();
                        if let Some(paths) = paths
                            && !paths.is_empty()
                        {
                            let request = if upload {
                                CopyRequest {
                                    upload,
                                    sources: paths,
                                    destination,
                                    name: None,
                                }
                            } else {
                                CopyRequest {
                                    upload,
                                    sources,
                                    destination: paths[0].clone(),
                                    name: None,
                                }
                            };
                            tab.error = None;
                            tab.start(request, &s.runtime);
                        } else {
                            tab.error = Some(t("files.invalid_path").into());
                        }
                    }
                    Err(e) => tab.error = Some(e),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn files_focused(&self, window: &Window, cx: &App) -> bool {
        self.workspace
            .file_view
            .and_then(|id| self.files.get(&id))
            .is_some_and(|tab| {
                tab.focus.is_focused(window) || tab.focus.contains_focused(window, cx)
            })
    }

    pub(super) fn file_panel(
        &self,
        id: FileId,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let tab = &self.files[&id];
        let p = self.palette;
        let focused = self.files_focused(window, cx);
        let tools = div()
            .flex()
            .flex_wrap()
            .flex_none()
            .items_center()
            .gap_1()
            .child(self.button(
                format!("files-refresh-{id}"),
                UiIcon::Refresh,
                cx,
                move |s, _, cx| s.refresh_files(id, cx),
            ))
            .child(self.file_picker_button(id, true, cx))
            .child(self.file_picker_button(id, false, cx));
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .flex_none()
            .child(
                div()
                    .flex_1()
                    .min_w(px(115.))
                    .truncate()
                    .text_sm()
                    .child(tab.title.clone()),
            )
            .child(self.button(
                "collapse-files",
                t("files.collapse"),
                cx,
                Self::return_to_terminals,
            ))
            .child(self.button(
                format!("close-files-{id}"),
                t("common.close"),
                cx,
                Self::close_file_view,
            ));
        let mut panel = div()
            .id(("files", id))
            .debug_selector(|| "file-panel".into())
            .track_focus(&tab.focus)
            .relative()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .bg(rgb(p.panel))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, _, w, cx| {
                    if s.dialog.is_none() {
                        let tab = s.files.get_mut(&id).unwrap();
                        tab.dragged = false;
                        w.focus(&tab.focus);
                        cx.notify();
                    }
                }),
            )
            .on_key_down(cx.listener(move |s, e: &KeyDownEvent, w, cx| {
                if s.dialog.is_some() {
                    return;
                }
                if e.keystroke.key == "escape" {
                    s.return_to_terminals(w, cx);
                } else if !s.file_arrows(id, width, &e.keystroke, cx) {
                    return;
                }
                w.prevent_default();
                cx.stop_propagation();
                cx.notify();
            }))
            .child(header)
            .child(self.file_breadcrumbs(id, cx))
            .child(tools)
            .when_some(tab.error.clone(), |d, e| d.child(div().text_xs().child(e)));
        #[cfg(target_os = "linux")]
        {
            let bounds = tab.bounds.clone();
            panel = panel.child(
                canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            );
        }
        if tab.folders.host_key_unknown()
            && let Some(key) = tab.browser.observed()
        {
            panel = panel
                .child(tr!(
                    "workspace.first_connection_prompt",
                    tab.host.endpoint(),
                    key
                ))
                .child(self.button(
                    format!("trust-files-{id}"),
                    t("workspace.trust_after_verify"),
                    cx,
                    move |s, _, cx| s.trust_files(id, cx),
                ));
        }
        panel = panel.child(self.file_panel_folders(id, width, focused, cx));
        if !tab.jobs.is_empty() {
            panel = panel.child(
                self.button(
                    "file-transfers",
                    tr!("files.transfers", tab.jobs.len()),
                    cx,
                    move |s, _, _| {
                        let tab = s.files.get_mut(&id).unwrap();
                        tab.jobs_open = !tab.jobs_open;
                    },
                )
                .flex()
                .items_center()
                .justify_between()
                .child(
                    if tab.jobs_open {
                        UiIcon::ChevronDown
                    } else {
                        UiIcon::ChevronRight
                    }
                    .render(p.fg),
                ),
            );
            if tab.jobs_open {
                panel = panel.child(self.file_jobs(id, cx));
            }
        }
        panel
            .capture_any_mouse_up(cx.listener(move |s, e: &MouseUpEvent, _, _| {
                if e.button == MouseButton::Left
                    && let Some(tab) = s.files.get_mut(&id)
                {
                    #[cfg(not(target_os = "macos"))]
                    tab.outgoing.release();
                    tab.pressed = None;
                }
            }))
            .on_mouse_move(cx.listener(move |s, e: &MouseMoveEvent, w, cx| {
                let tab = s.files.get_mut(&id).unwrap();
                if !e.dragging() {
                    tab.pressed = None;
                } else if tab
                    .pressed
                    .as_ref()
                    .is_some_and(|(_, _, p)| (e.position - *p).magnitude() > 3.0)
                {
                    let (path, name, _) = tab.pressed.take().unwrap();
                    tab.dragged = true;
                    s.drag_files(id, &path, &name, w, cx);
                }
            }))
            .on_drop(cx.listener(move |s, files: &ExternalPaths, _, cx| {
                #[cfg(target_os = "linux")]
                if s.linux_file_drag.handles_drops() {
                    return;
                }
                if s.workspace.file_view == Some(id)
                    && s.dialog.is_none()
                    && let Some(tab) = s.files.get_mut(&id)
                {
                    tab.upload_paths(files.paths(), &s.runtime);
                }
                cx.notify();
            }))
    }

    fn file_picker_button(
        &self,
        id: FileId,
        upload: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let folders = &self.files[&id].folders;
        let (label, key, enabled) = if upload {
            (
                t("files.upload"),
                "files-upload",
                !folders.listing.path.is_empty(),
            )
        } else {
            (
                t("files.download"),
                "files-download",
                !folders.selection().is_empty(),
            )
        };
        let name = format!("{key}-{id}");
        if enabled {
            self.button(name, label, cx, move |s, _, cx| {
                s.file_picker(id, upload, cx)
            })
        } else {
            div()
                .id(SharedString::from(name.clone()))
                .debug_selector(move || name.clone())
                .px_2()
                .py_1()
                .border_1()
                .rounded_sm()
                .border_color(rgb(self.palette.line))
                .text_color(rgb(self.palette.muted))
                .child(label)
        }
    }

    fn file_jobs(&self, id: FileId, cx: &mut Context<Self>) -> Stateful<Div> {
        let tab = &self.files[&id];
        let mut jobs = div()
            .id(SharedString::from(format!("file-jobs-{id}")))
            .max_h(relative(0.35))
            .overflow_y_scroll();
        for (index, job) in tab.jobs.iter().enumerate().rev() {
            let p = &job.progress;
            let label = if let Some(err) = &p.error {
                if p.item.is_empty() {
                    err.clone()
                } else {
                    format!("{} · {err}", p.item)
                }
            } else if p.done {
                tr!("files.completed", p.completed)
            } else {
                format!(
                    "{} · {} / {} B",
                    p.item,
                    p.bytes,
                    p.size.map(|s| s.to_string()).unwrap_or_else(|| "…".into())
                )
            };
            let mut line = div().flex().flex_wrap().items_center().gap_1().child(label);
            if let Some(conflict) = p.conflict.clone() {
                line = line.child(self.button(
                    format!("overwrite-{id}-{index}"),
                    tr!("files.overwrite", conflict),
                    cx,
                    move |s, _, _| s.files[&id].jobs[index].transfer.overwrite(&conflict),
                ));
            }
            if !p.done {
                line = line.child(self.button(
                    format!("cancel-{id}-{index}"),
                    t("common.cancel"),
                    cx,
                    move |s, _, _| s.files[&id].jobs[index].transfer.cancel(),
                ));
            }
            if p.error.is_some() {
                line = line.child(self.button(
                    format!("retry-{id}-{index}"),
                    t("files.retry"),
                    cx,
                    move |s, _, _| {
                        let tab = s.files.get_mut(&id).unwrap();
                        let request = tab.jobs[index].request.clone();
                        #[cfg(target_os = "macos")]
                        if let Some(previous) = &tab.jobs[index].promise {
                            let batch = Arc::new(previous.retry());
                            let _guard = s.runtime.enter();
                            let transfer = tab.browser.start_file_promise(
                                Arc::new(LocalPaths),
                                request.clone(),
                                batch.clone(),
                            );
                            tab.jobs.push(Job {
                                request,
                                transfer,
                                progress: Progress::default(),
                                promise: Some(batch),
                            });
                            tab.jobs_open = true;
                            return;
                        }
                        let _next = tab.start(request, &s.runtime);
                        #[cfg(not(target_os = "macos"))]
                        tab.outgoing
                            .retry(&tab.jobs[index].transfer, tab.jobs[_next].transfer.clone());
                    },
                ));
            }
            jobs = jobs.child(line);
        }
        jobs
    }
}

#[cfg(test)]
mod tests {
    use super::{Credential, HostProfile, Listing, ParentColumn};
    use crate::ui_tests::{redraw, setup};
    use crate::workspace;
    use dgl_core::sftp::Entry;
    use gpui::{Focusable, Modifiers, TestAppContext, VisualTestContext, point, px, size};

    fn fixture_host() -> HostProfile {
        HostProfile {
            id: "remote".into(),
            alias: "remote".into(),
            hostname: "127.0.0.1".into(),
            port: 1,
            username: "fixture".into(),
            auth: Default::default(),
            credential_ref: "fixture".into(),
            favorite: false,
            last_connected: 0,
            last_session: None,
            last_pane: None,
        }
    }

    fn listing(path: &str, entries: &[(&str, bool, bool)]) -> Listing {
        Listing {
            path: path.into(),
            entries: entries
                .iter()
                .map(|&(name, directory, regular)| Entry {
                    name: name.into(),
                    directory,
                    regular,
                    size: None,
                })
                .collect(),
            fallback: None,
        }
    }

    fn click(ui: &mut VisualTestContext, selector: &'static str) {
        let point = ui.debug_bounds(selector).unwrap().center();
        ui.simulate_click(point, Modifiers::default());
        redraw(ui);
    }

    fn open_host(ui: &mut VisualTestContext) {
        click(ui, "host-menu-remote");
        click(ui, "open-files-remote-None");
    }

    #[gpui::test]
    fn file_panel_resizes_around_existing_splits_and_preserves_both_focus_paths(
        cx: &mut TestAppContext,
    ) {
        let (view, ui) = setup(cx);
        ui.simulate_resize(size(px(1500.), px(800.)));
        let (id, first, second, tabs) = view.update(ui, |s, cx| {
            let host = fixture_host();
            s.local.hosts.push(host.clone());
            s.reconcile_hosts();
            let id = s.create_file_browser(
                host.clone(),
                None,
                None,
                None,
                (Credential::default(), None),
                cx,
            );
            let folders = &mut s.files.get_mut(&id).unwrap().folders;
            folders.listing = listing("/home/work", &[("document", false, true)]);
            folders.parents.insert(
                "/home".into(),
                ParentColumn::Ready(listing("/home", &[("work", true, false)])),
            );
            folders.parents.insert(
                "/".into(),
                ParentColumn::Ready(listing("/", &[("home", true, false)])),
            );
            let first = s.create_tab(host.clone(), Credential::default(), None, cx);
            let other = s.create_tab(host.clone(), Credential::default(), None, cx);
            assert!(
                s.workspace
                    .split_tab(other, 1, workspace::DockEdge::Right, 1100., 800.)
            );
            let second = s.workspace.active;
            let third = s.create_tab(host, Credential::default(), None, cx);
            (id, first, second, [first, other, third])
        });
        redraw(ui);
        open_host(ui);
        let sidebar = ui.debug_bounds("session-sidebar").unwrap();
        let panel = ui.debug_bounds("file-panel").unwrap();
        let status = ui.debug_bounds("status-bar").unwrap();
        assert_eq!(panel.top(), sidebar.top());
        assert!(
            (f32::from(panel.size.width) - 0.44 * (1500. - f32::from(sidebar.size.width))).abs()
                < 1.,
            "side panel {panel:?}, sidebar {sidebar:?}"
        );
        assert_eq!(panel.bottom(), status.top());
        assert!(ui.debug_bounds("file-column-2").is_some());
        view.read_with(ui, |s, cx| {
            assert_eq!(s.workspace.visible_tabs(), vec![first, tabs[2]]);
            for group in &s.workspace.layout.groups() {
                assert!(
                    f32::from(s.pointer.group_bounds[group].size.width) >= workspace::MIN_WIDTH
                );
            }
            assert!(s.tabs.iter().all(|t| t.terminal.read(cx).enabled));
        });

        click(ui, "file-row-/home/work/document");
        click(ui, "tab-2");
        let terminal = view.read_with(ui, |s, _| s.active_tab().unwrap().terminal.clone());
        ui.simulate_keystrokes("escape");
        redraw(ui);
        assert_eq!(
            terminal.read_with(ui, |t, cx| t.captured_inputs(cx).clone()),
            vec![vec![0x1b]]
        );
        view.read_with(ui, |s, _| {
            assert_eq!(s.workspace.file_view, Some(id));
            assert_eq!(s.workspace.active, second);
            assert_eq!(s.workspace.selected(), Some(tabs[1]));
            assert_eq!(
                s.files[&id].folders.selection(),
                vec!["/home/work/document"]
            );
        });

        ui.simulate_resize(size(px(850.), px(720.)));
        redraw(ui);
        let panel = ui.debug_bounds("file-panel").unwrap();
        let sidebar = ui.debug_bounds("session-sidebar").unwrap();
        let status = ui.debug_bounds("status-bar").unwrap();
        assert_eq!(panel.left(), sidebar.left());
        assert_eq!(panel.right(), status.right());
        assert!(panel.top() >= sidebar.bottom());
        assert_eq!(panel.bottom(), status.top());
        assert!((f32::from(panel.size.height) - f32::from(status.top()) * 0.45).abs() < 1.);

        open_host(ui);
        ui.simulate_keystrokes("left");
        view.read_with(ui, |s, _| {
            assert_eq!(s.files[&id].folders.cursor_path, "/home")
        });
        ui.simulate_keystrokes("right down");
        view.read_with(ui, |s, _| {
            assert_eq!(s.files[&id].folders.cursor_path, "/home/work")
        });
        assert_eq!(
            terminal.read_with(ui, |t, cx| t.captured_inputs(cx).len()),
            1
        );
        ui.simulate_keystrokes("escape");
        redraw(ui);
        view.read_with(ui, |s, _| assert_eq!(s.workspace.file_view, None));
        assert!(ui.update(|w, cx| terminal.focus_handle(cx).is_focused(w)));

        open_host(ui);
        click(ui, "collapse-files");
        view.update(ui, |s, cx| {
            for tab in tabs {
                s.close_tab(tab);
            }
            cx.notify();
        });
        redraw(ui);
        open_host(ui);
        let panel = ui.debug_bounds("file-panel").unwrap();
        assert_eq!(
            panel.left(),
            ui.debug_bounds("session-sidebar").unwrap().right()
        );
        assert_eq!(panel.bottom(), ui.debug_bounds("status-bar").unwrap().top());
        view.read_with(ui, |s, _| {
            assert_eq!(
                s.files[&id].folders.selection(),
                vec!["/home/work/document"]
            )
        });
        click(ui, "close-files-1");
        view.read_with(ui, |s, _| assert!(!s.files.contains_key(&id)));
    }

    #[gpui::test]
    fn file_rows_select_to_their_end_and_only_the_latest_folder_response_applies(
        cx: &mut TestAppContext,
    ) {
        let (view, ui) = setup(cx);
        ui.simulate_resize(size(px(1200.), px(700.)));
        let id = view.update(ui, |s, cx| {
            let host = fixture_host();
            s.local.hosts.push(host.clone());
            s.reconcile_hosts();
            let id =
                s.create_file_browser(host, None, None, None, (Credential::default(), None), cx);
            s.files.get_mut(&id).unwrap().folders.listing = listing(
                "/work",
                &[
                    ("first", true, false),
                    ("next", true, false),
                    ("a", false, true),
                    ("link", false, false),
                    ("b", false, true),
                    ("c", false, true),
                ],
            );
            id
        });
        redraw(ui);
        open_host(ui);
        let row_end = |ui: &mut VisualTestContext, selector, modifiers| {
            let bounds = ui.debug_bounds(selector).unwrap();
            ui.simulate_click(point(bounds.right() - px(3.), bounds.center().y), modifiers);
            redraw(ui);
        };
        row_end(ui, "file-row-/work/a", Modifiers::default());
        row_end(
            ui,
            "file-row-/work/c",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        let selection = |ui: &mut VisualTestContext| {
            view.read_with(ui, |s, _| s.files[&id].folders.selection())
        };
        assert_eq!(selection(ui), ["/work/a", "/work/b", "/work/c"]);
        row_end(
            ui,
            "file-row-/work/b",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(selection(ui), ["/work/a", "/work/c"]);
        row_end(
            ui,
            "file-row-/work/b",
            Modifiers {
                platform: true,
                ..Default::default()
            },
        );
        assert_eq!(selection(ui), ["/work/a", "/work/b", "/work/c"]);
        row_end(ui, "file-row-/work/link", Modifiers::default());
        assert_eq!(selection(ui), ["/work/a", "/work/b", "/work/c"]);

        let pending = |ui: &mut VisualTestContext| {
            view.read_with(ui, |s, _| {
                let attempt = s.files[&id].folders.attempt.as_ref().unwrap();
                (attempt.serial, attempt.path.clone().unwrap())
            })
        };
        row_end(ui, "file-row-/work/first", Modifiers::default());
        let (first, path) = pending(ui);
        assert_eq!(path, "/work/first");
        row_end(ui, "file-row-/work/next", Modifiers::default());
        let (next, path) = pending(ui);
        assert_eq!(path, "/work/next");
        view.update(ui, |s, cx| {
            let folders = &mut s.files.get_mut(&id).unwrap().folders;
            assert!(folders.finish(next, Ok(listing("/work/next", &[("readme", false, true)]))));
            assert!(!folders.finish(first, Ok(listing("/work/first", &[]))));
            cx.notify();
        });
        redraw(ui);
        assert!(ui.debug_bounds("file-row-/work/next/readme").is_some());
        assert!(selection(ui).is_empty());

        row_end(ui, "file-row-/work/first", Modifiers::default());
        let (failed, _) = pending(ui);
        view.update(ui, |s, cx| {
            let folders = &mut s.files.get_mut(&id).unwrap().folders;
            assert!(!folders.finish(failed, Err("Denied".into())));
            assert_eq!(folders.listing.path, "/work/next");
            assert_eq!(folders.refresh_path().as_deref(), Some("/work/first"));
            cx.notify();
        });
        redraw(ui);
        assert!(ui.debug_bounds("file-row-/work/next/readme").is_some());
        assert!(ui.debug_bounds("file-column-status-2").is_some());
    }
}
