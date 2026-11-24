use super::*;
use crate::workspace::FileId;
use dgl_core::sftp::{self, Browser, CopyRequest, Listing, LocalPaths, Progress, Transfer};
use std::collections::BTreeSet;

mod drag;
#[cfg(target_os = "linux")]
pub(crate) use drag::LinuxDrag;

pub(super) struct FileBrowser {
    host: HostProfile,
    session: Option<String>,
    pane: Option<String>,
    pub(super) title: String,
    browser: Arc<Browser>,
    listing: Listing,
    selected: BTreeSet<String>,
    loading: bool,
    error: Option<String>,
    jobs: Vec<Job>,
    outgoing: drag::Outgoing,
    pressed: Option<(String, Point<Pixels>)>,
    #[cfg(target_os = "linux")]
    bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}
struct Job {
    request: CopyRequest,
    transfer: Arc<Transfer>,
    progress: Progress,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.transfer.cancel();
    }
}
impl FileBrowser {
    fn selection(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|name| sftp::child_path(&self.listing.path, name).ok())
            .collect()
    }
    fn upload_paths(&mut self, paths: &[PathBuf], runtime: &tokio::runtime::Runtime) -> bool {
        if self.listing.path.is_empty() || paths.is_empty() {
            return false;
        }
        let sources: Option<Vec<_>> = paths
            .iter()
            .map(|p| p.to_str().map(str::to_owned))
            .collect();
        if let Some(sources) = sources {
            self.start(
                CopyRequest {
                    upload: true,
                    sources,
                    destination: self.listing.path.clone(),
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
        });
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
            let id = self.create_file_browser(host, session, pane, name, credentials);
            self.browse_files(id, None, cx);
            id
        };
        self.workspace.file_view = Some(id);
        window.focus(&self.focus);
    }
    pub(super) fn return_to_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.file_view = None;
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
                listing: Listing::default(),
                selected: Default::default(),
                loading: false,
                error: None,
                jobs: vec![],
                outgoing: Default::default(),
                pressed: None,
                #[cfg(target_os = "linux")]
                bounds: Default::default(),
            },
        );
        id
    }
    pub(super) fn refresh_files(&mut self, id: FileId, cx: &mut Context<Self>) {
        if let Some(tab) = self.files.get(&id) {
            let path = &tab.listing.path;
            self.browse_files(id, (!path.is_empty()).then(|| path.clone()), cx);
        }
    }
    fn browse_files(&mut self, id: FileId, path: Option<String>, cx: &mut Context<Self>) {
        let Some(tab) = self.files.get_mut(&id) else {
            return;
        };
        if tab.loading {
            return;
        }
        tab.loading = true;
        tab.error = None;
        let browser = tab.browser.clone();
        let task = self
            .runtime
            .spawn(async move { browser.browse(path.as_deref()).await });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = view.update(cx, |s, cx| {
                let Some(tab) = s.files.get_mut(&id) else {
                    return;
                };
                tab.loading = false;
                match result {
                    Ok(listing) => {
                        if tab.listing.path != listing.path {
                            tab.selected.clear();
                        }
                        tab.selected
                            .retain(|n| listing.entries.iter().any(|e| &e.name == n));
                        tab.listing = listing;
                    }
                    Err(e) => tab.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn trust_files(&mut self, id: FileId, cx: &mut Context<Self>) {
        let tab = &self.files[&id];
        let Some(fingerprint) = tab
            .browser
            .observed()
            .filter(|_| tab.error.as_deref() == Some("HostKeyUnknown"))
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
        self.browse_files(id, None, cx);
    }
    pub(super) fn tick_files(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        #[cfg(target_os = "linux")]
        {
            changed |= self.linux_file_drag.pending();
        }
        let mut refresh = Vec::new();
        for (&id, tab) in &mut self.files {
            changed |= tab.outgoing.poll(&mut tab.jobs);
            for job in &mut tab.jobs {
                let progress = job.transfer.progress();
                if progress != job.progress {
                    if progress.done
                        && progress.error.is_none()
                        && job.request.upload
                        && job.request.destination == tab.listing.path
                    {
                        refresh.push((id, tab.listing.path.clone()));
                    }
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
        let destination = tab.listing.path.clone();
        let sources = tab.selection();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: upload,
            directories: true,
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
    pub(super) fn file_panel(&self, id: FileId, cx: &mut Context<Self>) -> Stateful<Div> {
        let tab = &self.files[&id];
        let path = tab.listing.path.clone();
        let mut tools = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .child(self.button(
                format!("files-refresh-{id}"),
                UiIcon::Refresh,
                cx,
                move |s, _, cx| s.refresh_files(id, cx),
            ));
        if !path.is_empty() {
            let parent = path
                .rsplit_once('/')
                .map(|(p, _)| if p.is_empty() { "/" } else { p })
                .unwrap_or("/")
                .to_string();
            tools = tools
                .child(self.button(
                    format!("files-up-{id}"),
                    t("files.up"),
                    cx,
                    move |s, _, cx| s.browse_files(id, Some(parent.clone()), cx),
                ))
                .child(self.button(
                    format!("files-upload-{id}"),
                    t("files.upload"),
                    cx,
                    move |s, _, cx| s.file_picker(id, true, cx),
                ));
            if !tab.selected.is_empty() {
                tools = tools.child(self.button(
                    format!("files-download-{id}"),
                    t("files.download"),
                    cx,
                    move |s, _, cx| s.file_picker(id, false, cx),
                ));
            }
        }
        tools = tools.child(self.button(
            format!("close-files-{id}"),
            t("common.close"),
            cx,
            Self::close_file_view,
        ));
        let mut panel = div()
            .id(SharedString::from(format!("files-{id}")))
            .debug_selector(|| "file-panel".into())
            .relative()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .child(tools)
            .child(div().child(path.clone()))
            .when(tab.loading, |d| d.child(t("files.loading")))
            .when_some(tab.listing.fallback.as_ref(), |d, reason| {
                d.child(tr!("files.fallback", reason))
            })
            .when_some(tab.error.clone(), |d, e| d.child(e));
        #[cfg(target_os = "linux")]
        {
            let bounds = tab.bounds.clone();
            panel = panel.child(
                canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            );
        }
        if tab.error.as_deref() == Some("HostKeyUnknown")
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
        let mut rows = div()
            .id(SharedString::from(format!("files-rows-{id}")))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, entry) in tab.listing.entries.iter().enumerate() {
            let name = entry.name.clone();
            let selected = tab.selected.contains(&name);
            let source = sftp::child_path(&path, &name).expect("validated directory entry");
            let press_name = name.clone();
            let mut row = div()
                .id(SharedString::from(format!("file-{id}-{i}")))
                .flex()
                .items_center()
                .gap_2()
                .when(selected, |d| d.bg(rgb(self.palette.selection)))
                .child(self.button(
                    format!("file-select-{id}-{i}"),
                    if selected { "☑" } else { "☐" },
                    cx,
                    move |s, _, _| {
                        let tab = s.files.get_mut(&id).unwrap();
                        if !tab.selected.remove(&name) {
                            tab.selected.insert(name.clone());
                        }
                    },
                ))
                .child(div().flex_1().child(format!(
                    "{}{}",
                    entry.name,
                    if entry.directory { "/" } else { "" }
                )))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, e: &MouseDownEvent, _, _| {
                        s.files.get_mut(&id).unwrap().pressed =
                            Some((press_name.clone(), e.position));
                    }),
                );
            if entry.directory {
                row = row.child(self.button(
                    format!("file-enter-{id}-{i}"),
                    UiIcon::ChevronRight,
                    cx,
                    move |s, _, cx| s.browse_files(id, Some(source.clone()), cx),
                ));
            }
            rows = rows.child(row);
        }
        let mut jobs = div()
            .id(SharedString::from(format!("file-jobs-{id}")))
            .max_h(px(170.))
            .overflow_y_scroll();
        for (index, job) in tab.jobs.iter().enumerate().rev() {
            let p = &job.progress;
            let label = if let Some(err) = &p.error {
                format!("{} · {err}", p.item)
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
                let req = job.request.clone();
                line = line.child(self.button(
                    format!("retry-{id}-{index}"),
                    t("files.retry"),
                    cx,
                    move |s, _, _| {
                        let tab = s.files.get_mut(&id).unwrap();
                        let _next = tab.start(req.clone(), &s.runtime);
                        #[cfg(not(target_os = "macos"))]
                        tab.outgoing
                            .retry(&tab.jobs[index].transfer, tab.jobs[_next].transfer.clone());
                    },
                ));
            }
            jobs = jobs.child(line);
        }
        #[cfg(not(target_os = "macos"))]
        {
            panel = panel.capture_any_mouse_up(cx.listener(move |s, e: &MouseUpEvent, _, _| {
                if e.button == MouseButton::Left
                    && let Some(tab) = s.files.get_mut(&id)
                {
                    tab.outgoing.release();
                    tab.pressed = None;
                }
            }));
        }
        panel = panel.on_mouse_move(cx.listener(move |s, e: &MouseMoveEvent, w, cx| {
            let tab = s.files.get_mut(&id).unwrap();
            if !e.dragging() {
                tab.pressed = None;
            } else if tab
                .pressed
                .as_ref()
                .is_some_and(|(_, p)| (e.position - *p).magnitude() > 3.0)
            {
                let (name, _) = tab.pressed.take().unwrap();
                s.drag_files(id, &name, w, cx);
            }
        }));
        panel.child(rows).child(jobs).on_drop(cx.listener(
            move |s, files: &ExternalPaths, _, cx| {
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
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Credential, HostProfile};
    use crate::ui_tests::{redraw, setup};
    use gpui::{Modifiers, TestAppContext};

    #[gpui::test]
    fn host_file_menu_preserves_terminal_tabs_and_resumes_the_selected_browser(
        cx: &mut TestAppContext,
    ) {
        let (view, ui) = setup(cx);
        let host = HostProfile {
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
        };
        let (terminal, host_files, session_files) = view.update(ui, |s, cx| {
            s.local.hosts.push(host.clone());
            s.reconcile_hosts();
            let host_files = s.create_file_browser(
                host.clone(),
                None,
                None,
                None,
                (Credential::default(), None),
            );
            s.files.get_mut(&host_files).unwrap().listing.path = "/home/fixture".into();
            let session = Some("$1".to_string());
            let session_files = s.create_file_browser(
                host.clone(),
                session.clone(),
                None,
                None,
                (Credential::default(), None),
            );
            s.files.get_mut(&session_files).unwrap().listing.path = "/work".into();
            let terminal = s.create_tab(
                HostProfile {
                    last_session: session,
                    ..host
                },
                Credential::default(),
                None,
                cx,
            );
            cx.notify();
            (terminal, host_files, session_files)
        });
        redraw(ui);
        assert!(ui.debug_bounds("switch-to-sftp").is_none());
        assert!(ui.debug_bounds("switch-to-terminal").is_none());
        let open_host_files = |ui: &mut gpui::VisualTestContext| {
            let menu = ui.debug_bounds("host-menu-remote").unwrap().center();
            ui.simulate_click(menu, Modifiers::default());
            redraw(ui);
            let button = ui.debug_bounds("open-files-remote-None").unwrap().center();
            ui.simulate_click(button, Modifiers::default());
            redraw(ui);
        };
        for showing_files in [true, false, true] {
            if showing_files {
                open_host_files(ui);
                let panel = ui.debug_bounds("file-panel").unwrap();
                assert!(panel.left() >= ui.debug_bounds("session-sidebar").unwrap().right());
                assert!(panel.bottom() <= ui.debug_bounds("status-bar").unwrap().top());
            } else {
                let button = ui.debug_bounds("switch-to-terminal").unwrap().center();
                ui.simulate_click(button, Modifiers::default());
                redraw(ui);
            }
            view.read_with(ui, |s, cx| {
                assert!(s.menu.is_none());
                assert_eq!(s.tabs[0].terminal.read(cx).enabled, !showing_files);
                assert_eq!(s.workspace.groups[&s.workspace.active].tabs, vec![terminal]);
                assert_eq!(s.workspace.file_view, showing_files.then_some(host_files));
                assert_eq!(s.workspace.selected(), (!showing_files).then_some(terminal));
                assert_eq!(s.workspace.visible_tabs().is_empty(), showing_files);
                assert_eq!(s.files[&host_files].listing.path, "/home/fixture");
                assert_eq!(s.files[&session_files].listing.path, "/work");
            });
        }
        ui.simulate_keystrokes("escape");
        redraw(ui);
        view.read_with(ui, |s, _| {
            assert_eq!(s.workspace.selected(), Some(terminal))
        });

        // A saved host also has a file view when no terminal is open.
        view.update(ui, |s, cx| {
            s.close_tab(terminal);
            cx.notify();
        });
        redraw(ui);
        open_host_files(ui);
        ui.update(|w, cx| {
            view.update(cx, |s, cx| {
                assert_eq!(s.workspace.file_view, Some(host_files));
                assert!(s.workspace.groups[&s.workspace.active].tabs.is_empty());
                s.close_file_view(w, cx);
                assert!(s.workspace.file_view.is_none());
                assert!(!s.files.contains_key(&host_files));
                assert!(s.files.contains_key(&session_files));
            })
        });
    }
}
