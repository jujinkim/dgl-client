//! Wheel and touch scrolling over an alternate screen that does not track the mouse.
//!
//! Terminals turn the wheel into Up/Down there (alternate scroll). Agent CLIs such as
//! Codex read those as prompt history, so the assist sends PageUp/PageDown instead,
//! which all supported CLIs scroll their transcript with. Mouse-tracking programs and
//! the normal screen's local scrollback are never affected.
use crate::agent::{Agent, Snapshot};
use crate::model::RemotePane;

pub const PAGE_UP: &[u8] = b"\x1b[5~";
pub const PAGE_DOWN: &[u8] = b"\x1b[6~";
/// A pause longer than this starts a new gesture, which pages on its first row.
const IDLE_MS: u64 = 400;

/// The `terminalScrollAssist` preference. Unknown stored values behave as [`Self::Auto`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAssist {
    /// Only panes running a supported agent CLI.
    #[default]
    Auto,
    /// Every alternate screen without mouse tracking.
    On,
    /// Always the terminal's Up/Down translation.
    Off,
}
impl ScrollAssist {
    pub fn from_preference(value: &str) -> Self {
        match value {
            "on" => Self::On,
            "off" => Self::Off,
            _ => Self::Auto,
        }
    }
    pub fn preference(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }
    pub fn applies(self, cli_in_pane: bool) -> bool {
        match self {
            Self::Auto => cli_in_pane,
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// A supported CLI in `pane`: observed by the agent helper, or tmux reports it as the
/// pane's current command when the observer is stale or unavailable.
pub fn cli_in_pane(agents: &Snapshot, panes: &[RemotePane], pane: &str, now: u64) -> bool {
    agents.agent_in_pane(pane, now).is_some()
        || panes
            .iter()
            .any(|p| p.id == pane && Agent::from_command(&p.command).is_some())
}

/// Rows scrolled before another page key. PageUp moves half to a whole screen.
pub fn page_rows(screen_rows: usize) -> i32 {
    (screen_rows as i32 / 2).max(3)
}

/// Turns scrolled rows (positive is up) into PageUp/PageDown presses. A new gesture
/// pages on its first row; the following ones every [`page_rows`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PageScroll {
    rows: i32,
    direction: i32,
    last: Option<u64>,
}
impl PageScroll {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Signed page count for `rows`, positive for PageUp.
    pub fn pages(&mut self, rows: i32, screen_rows: usize, now: u64) -> i32 {
        if rows == 0 {
            return 0;
        }
        let step = page_rows(screen_rows);
        let fresh = self
            .last
            .is_none_or(|last| now.saturating_sub(last) > IDLE_MS)
            || self.direction != rows.signum();
        if fresh {
            // Charged so the first row crosses a page boundary.
            self.rows = rows.signum() * (step - 1);
        }
        self.direction = rows.signum();
        self.last = Some(now);
        self.rows += rows;
        let pages = self.rows / step;
        self.rows -= pages * step;
        pages
    }
    /// Bytes for `pages`, capped to one input write.
    pub fn bytes(pages: i32) -> Vec<u8> {
        let key = if pages > 0 { PAGE_UP } else { PAGE_DOWN };
        key.repeat((pages.unsigned_abs() as usize).min(crate::input::MAX_INPUT / key.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_values_round_trip_and_unknown_is_auto() {
        for mode in [ScrollAssist::Auto, ScrollAssist::On, ScrollAssist::Off] {
            assert_eq!(ScrollAssist::from_preference(mode.preference()), mode);
        }
        assert_eq!(ScrollAssist::from_preference("future"), ScrollAssist::Auto);
        assert!(ScrollAssist::Auto.applies(true));
        assert!(!ScrollAssist::Auto.applies(false));
        assert!(ScrollAssist::On.applies(false));
        assert!(!ScrollAssist::Off.applies(true));
    }

    #[test]
    fn first_row_pages_then_every_half_screen() {
        let mut s = PageScroll::default();
        assert_eq!(page_rows(40), 20);
        assert_eq!(page_rows(4), 3);
        assert_eq!(s.pages(1, 40, 0), 1);
        assert_eq!(s.pages(18, 40, 10), 0);
        assert_eq!(s.pages(1, 40, 20), 0);
        assert_eq!(s.pages(1, 40, 30), 1);
        assert_eq!(s.pages(45, 40, 40), 2);
        // Reversing or pausing starts a new gesture, also right after a whole page.
        assert_eq!(s.pages(19, 40, 45), 1);
        assert_eq!(s.pages(-1, 40, 50), -1);
        assert_eq!(s.pages(-1, 40, 60), 0);
        assert_eq!(s.pages(-1, 40, 1000), -1);
        assert_eq!(s.pages(0, 40, 1010), 0);
        assert_eq!(PageScroll::bytes(2), b"\x1b[5~\x1b[5~");
        assert_eq!(PageScroll::bytes(-1), b"\x1b[6~");
    }

    #[test]
    fn cli_found_by_observer_or_current_command() {
        let agents = Snapshot::default();
        let mut panes = vec![RemotePane {
            id: "%1".into(),
            window: "@1".into(),
            width: 80,
            height: 24,
            active: true,
            mode: String::new(),
            command: "zsh".into(),
            title: String::new(),
            left: 0,
            top: 0,
        }];
        assert!(!cli_in_pane(&agents, &panes, "%1", 0));
        panes[0].command = "codex".into();
        assert!(cli_in_pane(&agents, &panes, "%1", 0));
        assert!(!cli_in_pane(&agents, &panes, "%2", 0));
        panes[0].command = "/usr/local/bin/grok".into();
        assert!(cli_in_pane(&agents, &panes, "%1", 0));
        panes[0].command = "less".into();
        assert!(!cli_in_pane(&agents, &panes, "%1", 0));
    }
}
