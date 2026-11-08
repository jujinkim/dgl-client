//! The OS power saving mode the user turned on: macOS Low Power Mode or Windows battery
//! saver. Plugging in or unplugging alone does not change pacing. Read at most every 30 s.
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use self::macos as native;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows as native;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod native {
    pub fn saving() -> bool {
        false
    }
}

const REFRESH: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct PowerSaving {
    checked: Option<(Instant, bool)>,
}
impl PowerSaving {
    pub fn get(&mut self, now: Instant) -> bool {
        self.get_with(now, native::saving)
    }
    fn get_with(&mut self, now: Instant, read: impl FnOnce() -> bool) -> bool {
        match self.checked {
            Some((at, saving)) if now.saturating_duration_since(at) < REFRESH => saving,
            _ => {
                let saving = read();
                self.checked = Some((now, saving));
                saving
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_once_per_refresh_interval() {
        let start = Instant::now();
        let mut power = PowerSaving::default();
        assert!(power.get_with(start, || true));
        assert!(
            power.get_with(start + REFRESH - Duration::from_millis(1), || panic!(
                "cached"
            ))
        );
        assert!(!power.get_with(start + REFRESH, || false));
        // The native query runs on this platform without failing.
        let _ = native::saving();
    }
}
