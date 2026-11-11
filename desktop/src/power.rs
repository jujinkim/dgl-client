//! Power saving pacing. By default it follows the OS power saving mode the user turned on:
//! macOS Low Power Mode or Windows battery saver, read at most every 30 s. Plugging in or
//! unplugging alone does not change pacing. The device-local setting can override it.
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
        std::fs::read_to_string("/sys/firmware/acpi/platform_profile")
            .is_ok_and(|profile| profile.trim() == "low-power")
    }
}

const REFRESH: Duration = Duration::from_secs(30);

/// The `desktopPowerSaving` preference. Unknown stored values behave as [`Mode::System`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    System,
    Always,
    Off,
}
impl Mode {
    pub fn from_preference(value: &str) -> Self {
        match value {
            "always" => Self::Always,
            "off" => Self::Off,
            _ => Self::System,
        }
    }
    pub fn preference(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Always => "always",
            Self::Off => "off",
        }
    }
}

#[derive(Default)]
pub struct PowerSaving {
    checked: Option<(Instant, bool)>,
}
impl PowerSaving {
    /// Whether to pace for power saving; only [`Mode::System`] queries the OS.
    pub fn get(&mut self, mode: Mode, now: Instant) -> bool {
        match mode {
            Mode::Always => true,
            Mode::Off => false,
            Mode::System => self.get_with(now, native::saving),
        }
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

    #[test]
    fn overrides_skip_the_os_and_unknown_values_follow_it() {
        let now = Instant::now();
        let mut power = PowerSaving::default();
        assert!(power.get(Mode::Always, now));
        assert!(!power.get(Mode::Off, now));
        assert!(power.checked.is_none(), "overrides never query the OS");
        for mode in [Mode::System, Mode::Always, Mode::Off] {
            assert_eq!(Mode::from_preference(mode.preference()), mode);
        }
        assert_eq!(Mode::from_preference("future-mode"), Mode::System);
        assert_eq!(Mode::from_preference(""), Mode::System);
    }
}
