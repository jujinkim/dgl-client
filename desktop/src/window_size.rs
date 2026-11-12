//! Device-local windowed size, reused for the next launch. Position and fullscreen
//! are not restored: the window opens centered on the primary display.
use super::*;
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

pub const DEFAULT: (f32, f32) = (1240., 800.);
const MIN: (f32, f32) = (400., 300.);
const MAX: f32 = 20_000.;
const SAVE_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSize {
    width: f32,
    height: f32,
}

impl WindowSize {
    fn new(width: f32, height: f32) -> Option<Self> {
        let valid = |v: f32, min: f32| v.is_finite() && (min..=MAX).contains(&v);
        (valid(width, MIN.0) && valid(height, MIN.1)).then_some(Self { width, height })
    }

    pub fn load(path: &Path) -> Option<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .ok()?
            .take(65_537)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 65_536 {
            return None;
        }
        let saved: Self = serde_json::from_slice(&bytes).ok()?;
        Self::new(saved.width, saved.height)
    }

    pub fn save(&self, path: &Path) -> dgl_core::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|_| dgl_core::Error::Storage)?;
        dgl_core::sync::save_private(path, &bytes)
    }

    /// The windowed size to remember. A maximized window reports its restore size;
    /// fullscreen is skipped so the size from before it is kept.
    pub fn from_bounds(bounds: WindowBounds) -> Option<Self> {
        match bounds {
            WindowBounds::Windowed(b) | WindowBounds::Maximized(b) => {
                Self::new(f32::from(b.size.width), f32::from(b.size.height))
            }
            WindowBounds::Fullscreen(_) => None,
        }
    }

    /// Opening size: the saved one, shrunk to fit the display when it no longer does.
    pub fn fit(saved: Option<Self>, display: Option<Size<Pixels>>) -> Size<Pixels> {
        let (mut width, mut height) = saved.map_or(DEFAULT, |s| (s.width, s.height));
        if let Some(display) = display {
            width = width.min(f32::from(display.width).max(MIN.0));
            height = height.min(f32::from(display.height).max(MIN.1));
        }
        size(px(width), px(height))
    }
}

pub struct Tracker {
    path: PathBuf,
    saved: Option<WindowSize>,
    pending: Option<WindowSize>,
    save: Option<Task<()>>,
}

impl Tracker {
    pub fn new(path: PathBuf) -> Self {
        Self {
            saved: WindowSize::load(&path),
            path,
            pending: None,
            save: None,
        }
    }

    /// Records the size and reports whether a write is now due.
    fn record(&mut self, size: Option<WindowSize>) -> bool {
        let Some(size) = size else {
            return false;
        };
        if self.pending.or(self.saved) == Some(size) {
            return false;
        }
        self.pending = Some(size);
        true
    }

    fn flush(&mut self) {
        self.save = None;
        if let Some(size) = self.pending.take()
            && Some(size) != self.saved
            && size.save(&self.path).is_ok()
        {
            self.saved = Some(size);
        }
    }
}

impl Desktop {
    /// Saves the windowed size shortly after resizing settles, and on quit.
    pub(super) fn watch_window_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_bounds(window, |s, window, cx| {
            if !s
                .window_size
                .record(WindowSize::from_bounds(window.window_bounds()))
            {
                return;
            }
            s.window_size.save = Some(cx.spawn(async move |view, cx| {
                Timer::after(SAVE_DELAY).await;
                let _ = view.update(cx, |s, _| s.window_size.flush());
            }));
        })
        .detach();
        cx.on_app_quit(|s, _| {
            s.window_size.flush();
            async {}
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{Tracker, WindowSize};
    use gpui::{Bounds, WindowBounds, point, px, size};

    fn windowed(w: f32, h: f32) -> WindowBounds {
        WindowBounds::Windowed(Bounds::new(point(px(10.), px(20.)), size(px(w), px(h))))
    }

    #[test]
    fn saved_size_round_trips_and_invalid_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.json");
        assert_eq!(WindowSize::load(&path), None);
        let size = WindowSize::new(1500., 900.).unwrap();
        size.save(&path).unwrap();
        assert_eq!(WindowSize::load(&path), Some(size));
        for bad in [
            "not json",
            r#"{"width":1500}"#,
            r#"{"width":1500,"height":900,"x":1}"#,
            r#"{"width":100,"height":900}"#,
            r#"{"width":1500,"height":30000}"#,
            r#"{"width":-1500,"height":900}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            assert_eq!(WindowSize::load(&path), None, "{bad}");
        }
        std::fs::write(&path, vec![b' '; 70_000]).unwrap();
        assert_eq!(WindowSize::load(&path), None);
    }

    #[test]
    fn fullscreen_is_skipped_and_maximized_keeps_its_restore_size() {
        assert_eq!(
            WindowSize::from_bounds(windowed(1300., 850.)),
            WindowSize::new(1300., 850.)
        );
        let restore = Bounds::new(point(px(0.), px(0.)), size(px(1100.), px(700.)));
        assert_eq!(
            WindowSize::from_bounds(WindowBounds::Maximized(restore)),
            WindowSize::new(1100., 700.)
        );
        assert_eq!(
            WindowSize::from_bounds(WindowBounds::Fullscreen(restore)),
            None
        );
        assert_eq!(WindowSize::from_bounds(windowed(0., 0.)), None);
    }

    #[test]
    fn opening_size_defaults_and_fits_the_display() {
        assert_eq!(WindowSize::fit(None, None), size(px(1240.), px(800.)));
        let saved = WindowSize::new(2400., 1400.);
        assert_eq!(WindowSize::fit(saved, None), size(px(2400.), px(1400.)));
        assert_eq!(
            WindowSize::fit(saved, Some(size(px(1920.), px(1080.)))),
            size(px(1920.), px(1080.))
        );
        assert_eq!(
            WindowSize::fit(None, Some(size(px(1024.), px(768.)))),
            size(px(1024.), px(768.))
        );
        assert_eq!(
            WindowSize::fit(saved, Some(size(px(200.), px(100.)))),
            size(px(400.), px(300.))
        );
    }

    #[test]
    fn tracker_writes_only_changed_windowed_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.json");
        let mut tracker = Tracker::new(path.clone());
        assert!(!tracker.record(None));
        assert!(tracker.record(WindowSize::new(1300., 850.)));
        assert!(!tracker.record(WindowSize::new(1300., 850.)));
        tracker.flush();
        assert_eq!(WindowSize::load(&path), WindowSize::new(1300., 850.));
        assert!(!tracker.record(WindowSize::new(1300., 850.)));
        assert!(
            !tracker.record(WindowSize::from_bounds(WindowBounds::Fullscreen(
                Bounds::default()
            )))
        );
        tracker.flush();
        assert_eq!(WindowSize::load(&path), WindowSize::new(1300., 850.));
        let reopened = Tracker::new(path.clone());
        assert_eq!(reopened.saved, WindowSize::new(1300., 850.));
    }
}
