use super::*;
use dgl_core::sftp::Entry;
use std::collections::BTreeSet;

const MAX_COLUMNS: usize = 3;
const COLUMN_WIDTH: f32 = 170.;

pub(super) struct BrowseAttempt {
    pub(super) serial: u64,
    pub(super) path: Option<String>,
    pub(super) error: Option<String>,
    task: tokio::task::AbortHandle,
}
impl BrowseAttempt {
    pub(super) fn loading(
        serial: u64,
        path: Option<String>,
        task: tokio::task::AbortHandle,
    ) -> Self {
        Self {
            serial,
            path,
            error: None,
            task,
        }
    }
}
impl Drop for BrowseAttempt {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) enum ParentColumn {
    Ready(Listing),
    Pending(BrowseAttempt),
}

#[derive(Default)]
pub(super) struct Folders {
    pub(super) listing: Listing,
    pub(super) attempt: Option<BrowseAttempt>,
    pub(super) parents: HashMap<String, ParentColumn>,
    serial: u64,
    selected: BTreeSet<String>,
    selection_path: String,
    anchor: Option<String>,
    pub(super) cursor_path: String,
    cursor_name: Option<String>,
    scroll: HashMap<String, ScrollHandle>,
    breadcrumb_scroll: ScrollHandle,
}

fn parent(path: &str) -> Option<&str> {
    let (prefix, leaf) = path.rsplit_once('/')?;
    (!leaf.is_empty()).then_some(if prefix.is_empty() { "/" } else { prefix })
}

fn breadcrumbs(path: &str) -> impl Iterator<Item = &str> {
    std::iter::once("/")
        .chain(path.match_indices('/').skip(1).map(|(idx, _)| &path[..idx]))
        .chain((path.len() > 1).then_some(path))
}

fn leaf(path: &str) -> &str {
    if path == "/" {
        path
    } else {
        path.rsplit('/').next().unwrap_or(path)
    }
}

impl Folders {
    pub(super) fn next_request(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    pub(super) fn refresh_path(&self) -> Option<String> {
        if let Some(attempt) = &self.attempt {
            return attempt.path.clone();
        }
        (!self.listing.path.is_empty()).then(|| self.listing.path.clone())
    }

    pub(super) fn host_key_unknown(&self) -> bool {
        self.attempt.as_ref().and_then(|a| a.error.as_deref()) == Some("HostKeyUnknown")
    }

    pub(super) fn finish(&mut self, serial: u64, result: Result<Listing, String>) -> bool {
        let Some(attempt) = self.attempt.as_mut().filter(|a| a.serial == serial) else {
            return false;
        };
        let listing = match result {
            Ok(listing) => listing,
            Err(error) => {
                attempt.error = Some(error);
                return false;
            }
        };
        self.attempt = None;
        if self.listing.path != listing.path {
            self.clear_selection();
            self.cursor_path.clone_from(&listing.path);
            self.cursor_name = None;
            if !self.listing.path.is_empty() {
                let old = std::mem::take(&mut self.listing);
                self.parents
                    .insert(old.path.clone(), ParentColumn::Ready(old));
            }
        } else if self.selection_path == listing.path {
            self.selected
                .retain(|name| listing.entries.iter().any(|e| e.regular && e.name == *name));
        }
        self.parents.remove(&listing.path);
        self.listing = listing;
        self.breadcrumb_scroll
            .scroll_to_item((breadcrumbs(&self.listing.path).count() - 1) * 2);
        let paths: Vec<_> = self
            .recent_paths(MAX_COLUMNS)
            .into_iter()
            .map(str::to_owned)
            .collect();
        self.scroll.retain(|path, _| paths.contains(path));
        for path in paths {
            self.scroll.entry(path).or_default();
        }
        true
    }

    pub(super) fn missing_parents(&mut self) -> Vec<(String, u64)> {
        let parents: Vec<_> = std::iter::successors(parent(&self.listing.path), |p| parent(p))
            .take(MAX_COLUMNS - 1)
            .map(str::to_owned)
            .collect();
        self.parents.retain(|path, _| parents.contains(path));
        let mut requests = Vec::new();
        for path in parents {
            if !self.parents.contains_key(&path) {
                requests.push((path, self.next_request()));
            }
        }
        requests
    }

    pub(super) fn finish_parent(
        &mut self,
        path: String,
        serial: u64,
        result: Result<Listing, String>,
    ) {
        let Some(ParentColumn::Pending(attempt)) = self.parents.get_mut(&path) else {
            return;
        };
        if attempt.serial != serial {
            return;
        }
        match result {
            Ok(listing) => {
                self.parents.insert(path, ParentColumn::Ready(listing));
            }
            Err(error) => attempt.error = Some(error),
        }
    }

    fn recent_paths(&self, count: usize) -> Vec<&str> {
        let mut paths: Vec<_> =
            std::iter::successors(Some(self.listing.path.as_str()), |p| parent(p))
                .take(count)
                .collect();
        paths.reverse();
        paths
    }

    fn pending_child(&self, columns: usize) -> Option<&str> {
        self.attempt
            .as_ref()
            .and_then(|a| a.path.as_deref())
            .filter(|path| columns > 1 && parent(path) == Some(self.listing.path.as_str()))
    }

    fn listing_at(&self, path: &str) -> Option<&Listing> {
        if path == self.listing.path {
            Some(&self.listing)
        } else if let Some(ParentColumn::Ready(listing)) = self.parents.get(path) {
            Some(listing)
        } else {
            None
        }
    }

    fn clear_selection(&mut self) {
        self.selected.clear();
        self.selection_path.clear();
        self.anchor = None;
    }

    pub(super) fn selection(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|name| sftp::child_path(&self.selection_path, name).ok())
            .collect()
    }

    pub(super) fn select_for_drag(&mut self, path: &str, name: &str) -> bool {
        if !self
            .listing_at(path)
            .is_some_and(|l| l.entries.iter().any(|e| e.name == name && e.regular))
        {
            return false;
        }
        if self.selection_path != path || !self.selected.contains(name) {
            self.select(path, name, Modifiers::default());
        }
        true
    }

    fn select(&mut self, path: &str, name: &str, modifiers: Modifiers) {
        if self.selection_path != path {
            self.clear_selection();
            self.selection_path = path.to_owned();
        }
        let extend = modifiers.platform || modifiers.control;
        if modifiers.shift
            && let Some(listing) = self.listing_at(path)
            && let Some(a) = listing
                .entries
                .iter()
                .position(|e| Some(&e.name) == self.anchor.as_ref())
            && let Some(b) = listing.entries.iter().position(|e| e.name == name)
        {
            let range: Vec<_> = listing.entries[a.min(b)..=a.max(b)]
                .iter()
                .filter(|e| e.regular)
                .map(|e| e.name.clone())
                .collect();
            if !extend {
                self.selected.clear();
            }
            self.selected.extend(range);
            return;
        }
        if extend {
            if !self.selected.remove(name) {
                self.selected.insert(name.to_owned());
            }
        } else {
            self.selected.clear();
            self.selected.insert(name.to_owned());
        }
        self.anchor = Some(name.to_owned());
    }

    fn set_cursor(&mut self, path: &str, name: Option<String>) {
        self.cursor_path = path.to_owned();
        self.cursor_name = name;
    }

    fn move_cursor(&mut self, path: &str, branch: Option<&str>) {
        let index = self.listing_at(path).and_then(|l| {
            branch
                .and_then(|name| l.entries.iter().position(|e| e.name == name))
                .or_else(|| (!l.entries.is_empty()).then_some(0))
        });
        self.set_cursor_index(path, index);
    }

    fn set_cursor_index(&mut self, path: &str, index: Option<usize>) {
        let name = index
            .and_then(|i| self.listing_at(path)?.entries.get(i))
            .map(|e| e.name.clone());
        self.set_cursor(path, name);
        if let Some(index) = index
            && let Some(scroll) = self.scroll.get(path)
        {
            scroll.scroll_to_item(index);
        }
    }
}

impl Desktop {
    pub(super) fn file_panel_folders(
        &self,
        id: FileId,
        width: f32,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let folders = &self.files[&id].folders;
        let count = column_count(width);
        let new_child = folders.pending_child(count);
        let mut paths = folders.recent_paths(count - usize::from(new_child.is_some()));
        paths.extend(new_child);
        let error_column = folders
            .attempt
            .as_ref()
            .and_then(|a| a.path.as_deref())
            .filter(|p| paths.contains(p))
            .unwrap_or(&folders.listing.path);
        let mut columns = div()
            .flex_1()
            .min_h_0()
            .flex()
            .border_1()
            .border_color(rgb(self.palette.line));
        for (index, path) in paths.iter().copied().enumerate() {
            let branch = paths
                .get(index + 1)
                .copied()
                .filter(|p| parent(p) == Some(path))
                .map(leaf);
            let listing = folders.listing_at(path);
            let attempt = if path == error_column && folders.attempt.is_some() {
                folders.attempt.as_ref()
            } else if let Some(ParentColumn::Pending(attempt)) = folders.parents.get(path) {
                Some(attempt)
            } else {
                None
            };
            let mut column = div()
                .debug_selector(move || format!("file-column-{index}"))
                .flex_1()
                .min_w_0()
                .min_h_0()
                .flex()
                .flex_col()
                .when(index > 0, |d| {
                    d.border_l_1().border_color(rgb(self.palette.line))
                });
            if !path.is_empty() {
                column = column.child(
                    div()
                        .px_2()
                        .py_1()
                        .flex_none()
                        .truncate()
                        .bg(rgb(self.palette.panel))
                        .text_xs()
                        .child(leaf(path).to_owned()),
                );
            }
            if let Some(attempt) = attempt {
                let label = match (&attempt.path, &attempt.error) {
                    (Some(target), Some(error)) => tr!("files.path_error", target, error),
                    (None, Some(error)) => error.clone(),
                    (Some(target), None) if target != path => tr!("files.loading_path", target),
                    _ => t("files.loading").to_owned(),
                };
                column = column.child(
                    status(label).debug_selector(move || format!("file-column-status-{index}")),
                );
            }
            if let Some(listing) = listing {
                if let Some(reason) = &listing.fallback {
                    column = column.child(status(tr!("files.fallback", reason)));
                }
                if !path.is_empty() && listing.entries.is_empty() && attempt.is_none() {
                    column = column.child(status(t("files.empty")));
                }
                let mut rows = div()
                    .id(SharedString::from(format!("file-rows-{id}-{path}")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when_some(folders.scroll.get(path), |d, scroll| d.track_scroll(scroll));
                for entry in &listing.entries {
                    rows = rows.child(self.file_row(id, path, entry, branch, focused, cx));
                }
                column = column.child(rows);
            }
            columns = columns.child(column);
        }
        columns
    }

    pub(super) fn file_breadcrumbs(&self, id: FileId, cx: &mut Context<Self>) -> Stateful<Div> {
        let folders = &self.files[&id].folders;
        let path = &folders.listing.path;
        let mut bar = div()
            .id(("file-path", id))
            .flex()
            .flex_none()
            .items_center()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&folders.breadcrumb_scroll)
            .gap_1();
        if !path.is_empty() {
            for (index, target) in breadcrumbs(path).enumerate() {
                let target_path = target.to_owned();
                bar = bar.child(
                    self.button(
                        format!("file-crumb-{index}"),
                        leaf(target).to_owned(),
                        cx,
                        move |s, _, cx| s.browse_files(id, Some(target_path.clone()), cx),
                    )
                    .flex_none(),
                );
                if target != path {
                    bar = bar.child(
                        div()
                            .flex_none()
                            .child(UiIcon::ChevronRight.render(self.palette.fg)),
                    );
                }
            }
        }
        bar
    }

    fn file_row(
        &self,
        id: FileId,
        path: &str,
        entry: &Entry,
        branch: Option<&str>,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let tab = &self.files[&id];
        let folders = &tab.folders;
        let selected = (folders.selection_path == path && folders.selected.contains(&entry.name))
            || branch == Some(entry.name.as_str());
        let cursor = focused
            && folders.cursor_path == path
            && folders.cursor_name.as_ref() == Some(&entry.name);
        let name = entry.name.clone();
        let path = path.to_owned();
        let directory = entry.directory;
        let regular = entry.regular;
        let p = self.palette;
        let selector = format!("file-row-{path}/{name}");
        let mut row = div()
            .id(SharedString::from(name.clone()))
            .debug_selector(move || selector.clone())
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .h(px(27.))
            .border_b_1()
            .border_color(rgb(if cursor { p.accent } else { p.line }))
            .when(selected, |d| d.bg(rgb(p.selection)))
            .when(directory || regular, |d| {
                d.cursor_pointer()
                    .hover(move |d| d.bg(rgb(if selected { p.selection } else { p.hover })))
            })
            .child(div().flex_1().min_w_0().truncate().child(name.clone()))
            .when(directory, |d| d.child(UiIcon::ChevronRight.render(p.fg)));
        if regular {
            let (path, name) = (path.clone(), name.clone());
            row = row.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, e: &MouseDownEvent, _, _| {
                    s.files.get_mut(&id).unwrap().pressed =
                        Some((path.clone(), name.clone(), e.position));
                }),
            );
        }
        row.on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
            let tab = s.files.get_mut(&id).unwrap();
            if tab.dragged {
                return;
            }
            let folders = &mut tab.folders;
            folders.set_cursor(&path, Some(name.clone()));
            if directory {
                s.browse_files(
                    id,
                    Some(sftp::child_path(&path, &name).expect("validated folder")),
                    cx,
                );
            } else if regular {
                folders.select(&path, &name, e.modifiers());
            }
            cx.notify();
        }))
    }

    pub(super) fn file_arrows(
        &mut self,
        id: FileId,
        width: f32,
        key: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        let folders = &mut self.files.get_mut(&id).unwrap().folders;
        let count = column_count(width);
        let paths =
            folders.recent_paths(count - usize::from(folders.pending_child(count).is_some()));
        let index = paths
            .iter()
            .position(|p| *p == folders.cursor_path)
            .unwrap_or(paths.len() - 1);
        let path = paths[index].to_owned();
        let cursor = (folders.cursor_path == path)
            .then_some(folders.cursor_name.as_deref())
            .flatten();
        match key.key.as_str() {
            "up" | "down" => {
                if let Some(listing) = folders.listing_at(&path)
                    && !listing.entries.is_empty()
                {
                    let entries = &listing.entries;
                    let last = entries.len() - 1;
                    let at = cursor.and_then(|name| entries.iter().position(|e| e.name == name));
                    let next = if key.key == "up" {
                        at.map_or(last, |i| i.saturating_sub(1))
                    } else {
                        at.map_or(0, |i| (i + 1).min(last))
                    };
                    let entry = &entries[next];
                    let name = entry.name.clone();
                    let regular = entry.regular;
                    folders.set_cursor_index(&path, Some(next));
                    if regular
                        && (key.modifiers.shift
                            || !(key.modifiers.control || key.modifiers.platform))
                    {
                        folders.select(&path, &name, key.modifiers);
                    }
                }
            }
            "left" => {
                if let Some(previous) = index.checked_sub(1).map(|i| paths[i].to_owned()) {
                    folders.move_cursor(&previous, Some(leaf(&path)));
                } else if let Some(parent) = parent(&path) {
                    self.browse_files(id, Some(parent.to_owned()), cx);
                }
            }
            "right" => {
                let next = paths.get(index + 1).copied();
                let folder = cursor
                    .filter(|name| {
                        folders.listing_at(&path).is_some_and(|l| {
                            l.entries.iter().any(|e| e.directory && e.name == *name)
                        })
                    })
                    .and_then(|name| sftp::child_path(&path, name).ok());
                if let Some(child) = folder.as_deref()
                    && Some(child) != next
                {
                    self.browse_files(id, Some(child.to_owned()), cx);
                } else if let Some(next) = next {
                    let next = next.to_owned();
                    let branch = paths.get(index + 2).map(|p| leaf(p).to_owned());
                    folders.move_cursor(&next, branch.as_deref());
                }
            }
            _ => return false,
        }
        true
    }
}

fn column_count(width: f32) -> usize {
    (((width - 16.) / COLUMN_WIDTH) as usize).clamp(1, MAX_COLUMNS)
}

fn status(label: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .py_1()
        .flex_none()
        .text_xs()
        .child(label.into())
}
