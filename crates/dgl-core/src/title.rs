//! OSC signals only schedule a query; tmux remains the source of displayed titles.
use crate::flow::PaneTarget;
use std::time::Duration;
use tokio::time::Instant;

const INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
enum Scan {
    #[default]
    Ground,
    Escape,
    Csi,
    Code,
    Separator,
    Title,
    TitleEscape,
    IgnoreOsc,
    IgnoreOscEscape,
    String,
    StringEscape,
}

/// Constant memory, including unterminated/large OSC and opaque control strings.
#[derive(Default)]
struct Detector(Scan);
impl Detector {
    fn feed(&mut self, bytes: &[u8]) -> bool {
        use Scan::*;
        let mut changed = false;
        for &b in bytes {
            self.0 = match (&self.0, b) {
                (_, 0x18 | 0x1a) => Ground, // CAN/SUB cancel a control string.
                (Ground, 0x1b) => Escape,
                (Escape, b']') => Code,
                (Escape, b'[') => Csi,
                (Escape, b'P' | b'X' | b'^' | b'_') => String,
                (Escape, 0x1b) => Escape,
                (Escape, _) => Ground,
                (Csi, 0x40..=0x7e) => Ground,
                (Csi, 0x1b) => Escape,
                (Csi, _) => Csi,
                (Code, b'0' | b'2') => Separator,
                (Code | Separator, 0x07) => Ground,
                (Code | Separator, 0x1b) => IgnoreOscEscape,
                (Separator, b';') => Title,
                (Code | Separator, _) => IgnoreOsc,
                (Title, 0x07) | (TitleEscape, b'\\') => {
                    changed = true;
                    Ground
                }
                (Title, 0x1b) => TitleEscape,
                (Title, 0x00..=0x1f | 0x7f) => IgnoreOsc,
                (Title, _) => Title,
                (TitleEscape, 0x07) => Ground,
                (TitleEscape, 0x1b) => IgnoreOscEscape,
                (TitleEscape, _) => IgnoreOsc,
                (IgnoreOsc | IgnoreOscEscape, 0x07) | (IgnoreOscEscape, b'\\') => Ground,
                (IgnoreOsc, 0x1b) => IgnoreOscEscape,
                (IgnoreOscEscape, 0x1b) => IgnoreOscEscape,
                (IgnoreOsc | IgnoreOscEscape, _) => IgnoreOsc,
                (String, 0x1b) => StringEscape,
                (StringEscape, b'\\') => Ground,
                (StringEscape, 0x1b) => StringEscape,
                (String | StringEscape, _) => String,
                (Ground, _) => Ground,
            };
        }
        changed
    }
}

#[derive(Clone, Debug)]
pub(super) struct Scope {
    pub target: PaneTarget,
    pub visibility: u64,
}
impl Scope {
    pub fn matches(&self, other: &Self) -> bool {
        self.visibility == other.visibility && self.target.same_input_destination(&other.target)
    }
}

#[derive(Default)]
pub(super) struct Updates {
    scope: Option<Scope>,
    detector: Detector,
    pending: bool,
    in_flight: bool,
    last_start: Option<Instant>,
}
impl Updates {
    pub fn set_last_start(&mut self, at: Option<Instant>) {
        self.last_start = at;
    }
    pub fn reconcile(&mut self, scope: Option<Scope>) {
        let same = match (&self.scope, &scope) {
            (Some(a), Some(b)) => a.matches(b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.detector = Detector::default();
            self.pending = false;
            self.scope = scope;
        }
    }
    pub fn feed(&mut self, pane: &str, bytes: &[u8]) {
        if self
            .scope
            .as_ref()
            .is_some_and(|s| s.target.pane_id == pane)
            && self.detector.feed(bytes)
        {
            self.pending = true;
        }
    }
    /// Changes preceding the full pane query are already covered by that query.
    pub fn covered_by_refresh(&mut self) {
        self.pending = false;
    }
    pub fn deadline(&self) -> Option<Instant> {
        (self.pending && !self.in_flight).then(|| {
            self.last_start
                .map_or_else(Instant::now, |at| at + INTERVAL)
        })
    }
    pub fn begin(&mut self) -> Option<Scope> {
        if self.deadline().is_none_or(|at| at > Instant::now()) {
            return None;
        }
        self.last_start = Some(Instant::now());
        self.pending = false;
        self.in_flight = true;
        self.scope.clone()
    }
    pub fn finish(&mut self) {
        self.in_flight = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::Generation;
    fn scope() -> Scope {
        Scope {
            target: PaneTarget {
                connection: 1,
                pane_id: "%1".into(),
                generation: Generation {
                    connection: 1,
                    pane: 1,
                    size: 1,
                },
            },
            visibility: 1,
        }
    }
    #[test]
    fn fragmented_titles_and_opaque_strings_preserve_all_bytes() {
        for title in ["", "한글 🚀", "quotes ' | #{} \\", "plain"] {
            for code in [0, 2] {
                for end in ["\x07", "\x1b\\"] {
                    let bytes = format!("before\x1b]{code};{title}{end}after").into_bytes();
                    for split in 0..=bytes.len() {
                        let mut detector = Detector::default();
                        let chunks = [&bytes[..split], &bytes[split..]];
                        assert_eq!(chunks.iter().filter(|b| detector.feed(b)).count(), 1);
                        assert_eq!(chunks.concat(), bytes);
                    }
                }
            }
        }
        for bytes in [
            &b"ordinary\x1b[31moutput\x07"[..],
            b"\x1b]1;icon\x07",
            b"\x1b]20;other\x07",
            b"\x1b]02;bad\x07",
            b"\x1b]2bad\x07",
            b"\x1b]2;unterminated",
            b"\x1b]2;cancel\x18\x07",
            b"\x1b]0;bad\n\x07",
            b"\x1b]2;bad\x1bX\x07",
            b"\x1b]52;\x1b]2;nested\x07",
            b"\x1bP\x1b]2;opaque\x07\x1b\\",
            b"\x1b_\x1b]2;opaque\x07\x1b\\",
            b"\x1b^\x1b]0;opaque\x07\x1b\\",
            b"\x1bX\x1b]0;opaque\x07\x1b\\",
        ] {
            let mut detector = Detector::default();
            assert!(!bytes.iter().any(|b| detector.feed(&[*b])), "{bytes:?}");
        }
        for bytes in [
            b"\x1b]2;bad\x1b\x07".as_slice(),
            b"\x1b]1;other\x1b\x07",
            b"\x1b]2;bad\x1b\x1b\\",
        ] {
            let mut detector = Detector::default();
            assert!(!detector.feed(bytes));
            assert!(detector.feed(b"\x1b]2;valid again\x07"));
        }
    }
    #[test]
    fn title_query_decodes_tmux_quoting_and_preserves_empty_and_unicode() {
        use crate::{
            command::{Command, Operation},
            model::pane_title,
        };
        assert_eq!(pane_title(&[Vec::new()]).unwrap(), "");
        assert_eq!(
            pane_title(&["한글 🚀 \\| \\' \\#{}".as_bytes().to_vec()]).unwrap(),
            "한글 🚀 | ' #{}"
        );
        assert!(pane_title(&[]).is_err());
        assert!(pane_title(&[vec![0xff]]).is_err());
        let command = Command::new(&Operation::PaneTitle {
            target: "%42".into(),
        })
        .unwrap();
        assert_eq!(
            command.wire,
            "display-message -p -t '%42' '#{q;s/[[#:cntrl#:]]/ /:pane_title}'"
        );
        assert!(
            Command::new(&Operation::PaneTitle {
                target: "%1; kill-server".into()
            })
            .is_err()
        );
    }
    #[tokio::test(start_paused = true)]
    async fn immediate_throttled_serial_and_trailing_queries_without_idle_timers() {
        let mut u = Updates::default();
        u.feed("%1", b"\x1b]2;hidden\x07");
        assert!(u.deadline().is_none());
        u.reconcile(Some(scope()));
        u.feed("%2", b"\x1b]2;inactive\x07");
        u.feed("%1", b"normal output");
        assert!(u.deadline().is_none());
        u.feed("%1", b"\x1b]2;first\x07");
        assert!(u.begin().is_some());
        for _ in 0..20 {
            u.feed("%1", b"\x1b]0;burst\x07");
        }
        assert!(u.begin().is_none()); // still in flight
        u.finish();
        tokio::time::advance(Duration::from_millis(499)).await;
        assert!(u.begin().is_none());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(u.begin().is_some());
        // Output arriving while a slow response is read must get a trailing query.
        u.feed("%1", b"\x1b]2;last\x07");
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(u.begin().is_none());
        u.finish();
        assert!(u.begin().is_some());
        u.finish();
        assert!(u.deadline().is_none());
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!(u.begin().is_none());
    }
    #[tokio::test(start_paused = true)]
    async fn refresh_and_scope_changes_clear_work_and_fragment_state() {
        let mut u = Updates::default();
        u.reconcile(Some(scope()));
        u.feed("%1", b"\x1b]2;covered\x07");
        u.covered_by_refresh();
        assert!(u.deadline().is_none());
        u.feed("%1", b"\x1b]2;");
        let mut changed = scope();
        changed.target.generation.pane += 1;
        u.reconcile(Some(changed.clone()));
        u.feed("%1", b"stale fragment\x07");
        assert!(u.deadline().is_none());
        u.feed("%1", b"\x1b]2;pending\x07");
        changed.target.generation.connection += 1;
        u.reconcile(Some(changed));
        assert!(u.deadline().is_none());
        u.feed("%1", b"\x1b]2;pending\x07");
        u.reconcile(None);
        assert!(u.deadline().is_none());
        u.reconcile(Some(scope()));
        u.feed("%1", b"\x1b]2;new\x07");
        assert!(u.begin().is_some());
        u.finish();
        // Hiding/returning does not bypass the per-connection rate limit.
        u.reconcile(None);
        u.reconcile(Some(scope()));
        u.feed("%1", b"\x1b]2;new\x07");
        assert!(u.begin().is_none());
    }
}
