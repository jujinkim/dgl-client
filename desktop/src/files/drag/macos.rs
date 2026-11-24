use super::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::mpsc,
};

unsafe extern "C" {
    fn dgl_file_drag(
        view: *mut c_void,
        names: *const *const c_char,
        dirs: *const bool,
        count: usize,
        context: *const c_void,
    ) -> bool;
    fn dgl_file_written(completion: usize, error: *const c_char);
}
struct Promise {
    sources: Vec<String>,
    browser: Arc<Browser>,
    runtime: Arc<tokio::runtime::Runtime>,
    sender: mpsc::Sender<Job>,
}
pub(in crate::files) struct Outgoing {
    sender: mpsc::Sender<Job>,
    receiver: mpsc::Receiver<Job>,
}
impl Default for Outgoing {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }
}
impl Outgoing {
    pub(in crate::files) fn poll(&mut self, jobs: &mut Vec<Job>) -> bool {
        let len = jobs.len();
        jobs.extend(self.receiver.try_iter());
        jobs.len() != len
    }
    pub(in crate::files) fn begin(
        &mut self,
        sources: Vec<String>,
        browser: Arc<Browser>,
        runtime: Arc<tokio::runtime::Runtime>,
        _: &mut Vec<Job>,
        window: &mut Window,
        dirs: Vec<bool>,
    ) -> Result<(), String> {
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)
            .map_err(|e| e.to_string())?
            .as_raw()
        else {
            return Err("AppKit window unavailable".into());
        };
        let names = sources
            .iter()
            .map(|p| CString::new(p.rsplit('/').next().unwrap_or("")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let pointers = names.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        let context = Arc::new(Promise {
            sources,
            browser,
            runtime,
            sender: self.sender.clone(),
        });
        let ok = unsafe {
            dgl_file_drag(
                handle.ns_view.as_ptr(),
                pointers.as_ptr(),
                dirs.as_ptr(),
                names.len(),
                Arc::as_ptr(&context).cast(),
            )
        };
        if ok {
            Ok(())
        } else {
            Err("Finder drag unavailable".into())
        }
    }
}
// Each provider retains one reference until AppKit releases its promised item.
#[unsafe(no_mangle)]
unsafe extern "C" fn dgl_file_retain(ctx: *const Promise) {
    unsafe { Arc::increment_strong_count(ctx) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn dgl_file_release(ctx: *const Promise) {
    unsafe { Arc::decrement_strong_count(ctx) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn dgl_file_write(
    ctx: *const Promise,
    index: usize,
    path: *const c_char,
    completion: usize,
) {
    let ctx = unsafe { &*ctx };
    let path = unsafe { CStr::from_ptr(path) }
        .to_str()
        .ok()
        .map(std::path::Path::new);
    let Some((dest, name)) =
        path.and_then(|p| Some((p.parent()?.to_str()?, p.file_name()?.to_str()?)))
    else {
        unsafe { dgl_file_written(completion, c"Invalid destination".as_ptr()) };
        return;
    };
    let request = CopyRequest {
        upload: false,
        sources: vec![ctx.sources[index].clone()],
        destination: dest.into(),
        name: Some(name.into()),
    };
    let _runtime = ctx.runtime.enter();
    let transfer = ctx.browser.start(Arc::new(LocalPaths), request.clone());
    if ctx
        .sender
        .send(Job {
            request,
            transfer: transfer.clone(),
            progress: Default::default(),
        })
        .is_err()
    {
        transfer.cancel();
    }
    crate::cadence::wake();
    ctx.runtime.spawn(async move {
        loop {
            let p = transfer.progress();
            if p.done {
                let error = p.error.map(|s| CString::new(s.replace('\0', " ")).unwrap());
                unsafe {
                    dgl_file_written(
                        completion,
                        error.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                    )
                };
                break;
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    });
}
