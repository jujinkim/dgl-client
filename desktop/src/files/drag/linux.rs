use guth_wayland_dnd::{DndEvent, DropAction, FileDnd};
use raw_window_handle::HasDisplayHandle;
use std::{path::PathBuf, sync::Arc, time::Duration};
pub(super) enum DragEvent {
    Drop(f32, f32, Vec<PathBuf>, Option<u64>),
    Error(String),
}
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use x11rb::{
    CURRENT_TIME, NONE,
    connection::Connection as XConnection,
    protocol::{Event, xproto::*},
    wrapper::ConnectionExt as _,
};

#[derive(Default)]
pub(crate) struct LinuxDrag {
    wayland: Option<FileDnd>,
    tried: bool,
    pending: Arc<AtomicBool>,
    sending: Option<Arc<tempfile::TempDir>>,
    x11: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}
impl LinuxDrag {
    pub(crate) fn pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }
    pub(super) fn events(
        &mut self,
        window: &(impl HasWindowHandle + HasDisplayHandle),
        enabled: bool,
    ) -> Vec<DragEvent> {
        let mut events = Vec::new();
        if !self.tried && enabled {
            self.tried = true;
            let pending = self.pending.clone();
            self.wayland = FileDnd::attach(window, move || {
                pending.store(true, Ordering::Release);
                crate::cadence::wake();
            });
            if let Some(bridge) = &self.wayland {
                bridge.set_outgoing_drag_action(Some(DropAction::Copy));
                bridge.set_incoming_drop_policy(false, DropAction::Copy);
            }
        }
        if let Some(bridge) = &self.wayland {
            for event in bridge.poll() {
                match event {
                    DndEvent::Dropped {
                        x,
                        y,
                        paths,
                        completion_id,
                        ..
                    } => {
                        events.push(DragEvent::Drop(x, y, paths, completion_id));
                    }
                    DndEvent::DragEnded { .. } => self.sending = None,
                    DndEvent::DragStartFailed { reason }
                    | DndEvent::WorkerUnavailable { reason }
                    | DndEvent::DropFailed { reason } => {
                        events.push(DragEvent::Error(reason));
                        self.sending = None;
                    }
                    _ => {}
                }
            }
        }
        if let Some(result) = self.x11.as_ref().and_then(|r| r.try_recv().ok()) {
            self.x11 = None;
            if let Err(e) = result {
                events.push(DragEvent::Error(e));
            }
        }
        events
    }
    pub(crate) fn handles_drops(&self) -> bool {
        self.wayland.is_some()
    }
    pub(super) fn complete(&self, id: u64, accepted: bool) {
        if let Some(bridge) = &self.wayland {
            bridge.complete_drop_with_action(
                id,
                accepted.then_some(guth_wayland_dnd::DropCompletion::Copy),
            );
        }
    }
    pub(super) fn start(
        &mut self,
        w: &impl HasWindowHandle,
        paths: Vec<PathBuf>,
        dir: Arc<tempfile::TempDir>,
    ) -> Result<bool, String> {
        if let Some(bridge) = &self.wayland {
            self.sending = Some(dir);
            bridge.start_drag(paths);
            return Ok(true);
        }
        let source = match HasWindowHandle::window_handle(w)
            .map_err(|e| e.to_string())?
            .as_raw()
        {
            RawWindowHandle::Xcb(h) => h.window.get(),
            RawWindowHandle::Xlib(h) => h.window as u32,
            _ => return Err("No file drag device".into()),
        };
        if self.x11.is_some() {
            return Ok(false);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.x11 = Some(rx);
        let pending = self.pending.clone();
        std::thread::spawn(move || {
            let _keep_files = dir;
            let _ = tx.send(xdnd(source, &paths).map_err(|e| e.to_string()));
            pending.store(true, Ordering::Release);
            crate::cadence::wake();
        });
        Ok(true)
    }
}
x11rb::atom_manager! {pub Atoms: AtomCookies {
    XdndAware, XdndProxy, XdndEnter, XdndLeave, XdndPosition, XdndStatus, XdndDrop, XdndFinished,
    XdndSelection, XdndActionCopy, TARGETS, uri_list:b"text/uri-list",
}}
/// Offer the prepared paths using XDND v5. Selection requests arrive on a
/// dedicated source window so GPUI continues to own the application's event queue.
fn xdnd(origin: u32, paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let (c, screen) = x11rb::connect(None)?;
    let root = c.setup().roots[screen].root;
    let a = Atoms::new(&c)?.reply()?;
    let source = c.generate_id()?;
    c.create_window(
        0,
        source,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new(),
    )?;
    c.set_selection_owner(source, a.XdndSelection, CURRENT_TIME)?;
    let uris = paths
        .iter()
        .map(|p| {
            url::Url::from_file_path(p)
                .map(|u| format!("{u}\r\n"))
                .map_err(|_| "Invalid file URL")
        })
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let send = |to: u32, kind: u32, data: [u32; 5]| -> Result<(), Box<dyn std::error::Error>> {
        let proxy = c
            .get_property(false, to, a.XdndProxy, AtomEnum::WINDOW, 0, 1)?
            .reply()?
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or(to);
        c.send_event(
            false,
            proxy,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, to, kind, data),
        )?;
        c.flush()?;
        Ok(())
    };
    let mut target = NONE;
    let mut accepted = false;
    let mut dropped: Option<std::time::Instant> = None;
    loop {
        while let Some(event) = c.poll_for_event()? {
            match event {
                Event::ClientMessage(m)
                    if m.type_ == a.XdndStatus && m.data.as_data32()[0] == target =>
                {
                    accepted = m.data.as_data32()[1] & 1 != 0
                }
                Event::ClientMessage(m)
                    if m.type_ == a.XdndFinished && m.data.as_data32()[0] == target =>
                {
                    return Ok(());
                }
                Event::SelectionRequest(r) if r.selection == a.XdndSelection => {
                    let prop = if r.property == NONE {
                        r.target
                    } else {
                        r.property
                    };
                    let result = if r.target == a.uri_list {
                        c.change_property8(
                            PropMode::REPLACE,
                            r.requestor,
                            prop,
                            a.uri_list,
                            uris.as_bytes(),
                        )?;
                        prop
                    } else if r.target == a.TARGETS {
                        c.change_property32(
                            PropMode::REPLACE,
                            r.requestor,
                            prop,
                            AtomEnum::ATOM,
                            &[a.TARGETS, a.uri_list],
                        )?;
                        prop
                    } else {
                        NONE
                    };
                    c.send_event(
                        false,
                        r.requestor,
                        EventMask::NO_EVENT,
                        SelectionNotifyEvent {
                            response_type: SELECTION_NOTIFY_EVENT,
                            sequence: 0,
                            time: r.time,
                            requestor: r.requestor,
                            selection: r.selection,
                            target: r.target,
                            property: result,
                        },
                    )?;
                    c.flush()?;
                }
                Event::SelectionClear(_) => return Ok(()),
                _ => {}
            }
        }
        if let Some(time) = dropped {
            if std::time::Instant::now() - time > Duration::from_secs(60) {
                return Ok(());
            }
        } else {
            let pointer = c.query_pointer(root)?.reply()?;
            if !pointer.mask.contains(KeyButMask::BUTTON1) {
                if target != NONE {
                    if accepted {
                        send(target, a.XdndDrop, [source, 0, CURRENT_TIME, 0, 0])?;
                        dropped = Some(std::time::Instant::now());
                    } else {
                        send(target, a.XdndLeave, [source, 0, 0, 0, 0])?;
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            } else {
                let mut under = pointer.child;
                let mut next = NONE;
                let mut version = 0;
                while under != NONE && under != origin {
                    if let Some(v) = c
                        .get_property(false, under, a.XdndAware, AtomEnum::ATOM, 0, 1)?
                        .reply()?
                        .value32()
                        .and_then(|mut v| v.next())
                    {
                        next = under;
                        version = v.min(5);
                        break;
                    }
                    under = c.query_pointer(under)?.reply()?.child;
                }
                if next != target {
                    if target != NONE {
                        send(target, a.XdndLeave, [source, 0, 0, 0, 0])?;
                    }
                    target = next;
                    accepted = false;
                    if target != NONE {
                        send(
                            target,
                            a.XdndEnter,
                            [source, version << 24, a.uri_list, 0, 0],
                        )?;
                    }
                }
                if target != NONE {
                    let xy =
                        (u32::from(pointer.root_x as u16) << 16) | u32::from(pointer.root_y as u16);
                    send(
                        target,
                        a.XdndPosition,
                        [source, 0, xy, CURRENT_TIME, a.XdndActionCopy],
                    )?;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
