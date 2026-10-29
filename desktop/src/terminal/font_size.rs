/// Per-connection, memory-only font size. Stored preferences remain the default.
#[derive(Debug, Clone)]
pub struct TerminalFontSize {
    default: i32,
    temporary: Option<i32>,
}

impl TerminalFontSize {
    pub fn new(default: i32) -> Self {
        Self {
            default: default.clamp(10, 28),
            temporary: None,
        }
    }
    pub fn effective(&self) -> i32 {
        self.temporary.unwrap_or(self.default)
    }
    pub fn override_size(&self) -> Option<i32> {
        self.temporary
    }
    pub fn configure(&mut self, default: i32) {
        self.default = default.clamp(10, 28);
    }
    pub fn adjust(&mut self, delta: i32) {
        self.temporary = Some(self.effective().saturating_add(delta).clamp(10, 28));
    }
    pub fn reset(&mut self) {
        self.temporary = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_overrides_are_isolated_clamped_and_reset_to_latest_default() {
        let mut first = TerminalFontSize::new(14);
        let second = TerminalFontSize::new(14);
        first.adjust(1);
        first.configure(15);
        assert_eq!(first.override_size(), Some(15));
        assert_eq!(second.override_size(), None);
        first.adjust(100);
        assert_eq!(first.effective(), 28);
        first.adjust(-1);
        assert_eq!(first.effective(), 27);
        first.adjust(-100);
        assert_eq!(first.effective(), 10);
        first.configure(18);
        first.reset();
        assert_eq!(first.override_size(), None);
        assert_eq!(first.effective(), 18);
        assert_eq!(second.effective(), 14);
    }
}
