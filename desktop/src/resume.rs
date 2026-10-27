//! Cross-platform wake detection: wall time advances during sleep on both macOS and Windows.
use std::time::{Duration, SystemTime};
#[derive(Default)]
pub struct ResumeDetector {
    previous: Option<(bool, SystemTime)>,
}
impl ResumeDetector {
    pub fn update(&mut self, foreground: bool, now: SystemTime) -> bool {
        let previous = self.previous.replace((foreground, now));
        previous.is_some_and(|(active, at)| {
            (!active && foreground)
                || now
                    .duration_since(at)
                    .is_ok_and(|gap| gap >= Duration::from_secs(3))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_and_sleep_resume_once_without_initial_or_repeated_checks() {
        let mut detector = ResumeDetector::default();
        let now = SystemTime::UNIX_EPOCH;
        assert!(!detector.update(true, now));
        assert!(!detector.update(false, now));
        assert!(detector.update(true, now));
        assert!(!detector.update(true, now + Duration::from_millis(16)));
        let wake = now + Duration::from_secs(60);
        assert!(detector.update(true, wake));
        assert!(!detector.update(true, wake));
        assert!(
            !detector.update(true, now),
            "clock moving backwards is not a wake"
        );
    }
}
