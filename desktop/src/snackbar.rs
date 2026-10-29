//! One replaceable snackbar. Expiry and the gauge share the same monotonic clock.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnackbarDuration {
    Short,
    #[default]
    Normal,
    Long,
    Indefinite,
    /// Zero expires immediately; indefinite display always uses the explicit preset.
    Millis(u64),
}

impl SnackbarDuration {
    pub fn timeout(self) -> Option<Duration> {
        Some(Duration::from_millis(match self {
            Self::Short => 3_000,
            Self::Normal => 5_000,
            Self::Long => 10_000,
            Self::Indefinite => return None,
            Self::Millis(ms) => ms,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct SnackbarRequest {
    pub message: String,
    pub duration: SnackbarDuration,
}

impl SnackbarRequest {
    pub fn new(message: impl Into<String>, duration: SnackbarDuration) -> Self {
        Self {
            message: message.into(),
            duration,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Snackbar {
    pub id: u64,
    pub request: SnackbarRequest,
    pub started_at: Instant,
}

/// Every gauge step repaints the whole window, terminals included, so it moves in
/// steps of this length rather than every frame.
const GAUGE_STEP: Duration = Duration::from_millis(100);

impl Snackbar {
    fn step(&self, now: Instant) -> u128 {
        now.saturating_duration_since(self.started_at).as_millis() / GAUGE_STEP.as_millis()
    }
    /// The gauge as drawn: the remaining fraction at the start of the current step.
    pub fn gauge_fraction(&self, now: Instant) -> Option<f32> {
        let step = GAUGE_STEP * self.step(now) as u32;
        self.remaining_fraction(self.started_at + step)
    }
    pub fn remaining_fraction(&self, now: Instant) -> Option<f32> {
        self.request.duration.timeout().map(|timeout| {
            if timeout.is_zero() {
                0.
            } else {
                (timeout
                    .saturating_sub(now.saturating_duration_since(self.started_at))
                    .as_secs_f64()
                    / timeout.as_secs_f64()) as f32
            }
        })
    }
}

#[derive(Default)]
pub struct SnackbarState {
    pub current: Option<Snackbar>,
    next_id: u64,
    /// The gauge step last reported by `tick`.
    drawn: Option<u128>,
}

impl SnackbarState {
    pub fn show_snackbar(&mut self, message: impl Into<String>) {
        self.show_snackbar_with_duration(message, SnackbarDuration::default());
    }

    pub fn show_snackbar_with_duration(
        &mut self,
        message: impl Into<String>,
        duration: SnackbarDuration,
    ) {
        self.show_at(SnackbarRequest::new(message, duration), Instant::now());
    }

    fn show_at(&mut self, request: SnackbarRequest, now: Instant) {
        self.next_id += 1;
        self.drawn = None;
        self.current = Some(Snackbar {
            id: self.next_id,
            request,
            started_at: now,
        });
    }

    pub fn is_some(&self) -> bool {
        self.current.is_some()
    }
    pub fn is_none(&self) -> bool {
        self.current.is_none()
    }

    pub fn dismiss(&mut self, id: u64) {
        if self
            .current
            .as_ref()
            .is_some_and(|message| message.id == id)
        {
            self.current = None;
        }
    }

    /// True when a timed gauge moved to a new step, or on the frame that removes it.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(current) = &self.current else {
            return false;
        };
        let Some(remaining) = current.remaining_fraction(now) else {
            return false;
        };
        if remaining <= 0. {
            self.current = None;
            return true;
        }
        let step = Some(current.step(now));
        step != std::mem::replace(&mut self.drawn, step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_custom_and_default_expire_at_their_exact_boundary() {
        let now = Instant::now();
        let mut state = SnackbarState::default();
        state.show_snackbar("default");
        assert_eq!(
            state.current.as_ref().unwrap().request.duration,
            SnackbarDuration::Normal
        );
        for (duration, ms) in [
            (SnackbarDuration::Short, 3_000),
            (SnackbarDuration::Normal, 5_000),
            (SnackbarDuration::Long, 10_000),
            (SnackbarDuration::Millis(1_234), 1_234),
        ] {
            state.show_at(SnackbarRequest::new("timed", duration), now);
            let message = state.current.as_ref().unwrap();
            assert_eq!(message.remaining_fraction(now), Some(1.));
            assert_eq!(
                message.remaining_fraction(now + Duration::from_millis(ms / 2)),
                Some(0.5)
            );
            assert!(state.tick(now + Duration::from_millis(ms - 1)));
            assert!(state.is_some());
            assert!(state.tick(now + Duration::from_millis(ms)));
            assert!(state.is_none());
        }
        state.show_at(
            SnackbarRequest::new("zero", SnackbarDuration::Millis(0)),
            now,
        );
        assert!(state.tick(now));
        assert!(state.is_none());
    }

    #[test]
    fn gauge_repaints_once_per_step_not_every_frame() {
        let now = Instant::now();
        let mut state = SnackbarState::default();
        state.show_at(SnackbarRequest::new("timed", SnackbarDuration::Normal), now);
        let frames = (0..5_000)
            .step_by(16)
            .map(|ms| now + Duration::from_millis(ms));
        let repaints = frames.filter(|at| state.tick(*at)).count();
        assert_eq!(repaints, 50, "one per 100 ms step over 5 s at 60 Hz");
        let message = state.current.as_ref().unwrap();
        assert_eq!(
            message.gauge_fraction(now + Duration::from_millis(2_599)),
            Some(0.5)
        );
        assert!(state.tick(now + Duration::from_secs(5)));
        assert!(state.is_none());
    }

    #[test]
    fn replacement_restarts_same_text_and_stale_dismiss_cannot_remove_it() {
        let now = Instant::now();
        let mut state = SnackbarState::default();
        state.show_at(SnackbarRequest::new("same", SnackbarDuration::Normal), now);
        let old = state.current.as_ref().unwrap().id;
        let later = now + Duration::from_secs(4);
        state.show_at(
            SnackbarRequest::new("same", SnackbarDuration::Normal),
            later,
        );
        state.dismiss(old);
        assert!(state.tick(now + Duration::from_secs(5)));
        assert_eq!(
            state.current.as_ref().unwrap().remaining_fraction(later),
            Some(1.)
        );
        state.dismiss(state.current.as_ref().unwrap().id);
        assert!(state.is_none());
    }

    #[test]
    fn indefinite_has_no_gauge_or_expiry_and_can_be_replaced_or_closed() {
        let now = Instant::now();
        let mut state = SnackbarState::default();
        state.show_at(
            SnackbarRequest::new("permanent", SnackbarDuration::Indefinite),
            now,
        );
        assert_eq!(
            state.current.as_ref().unwrap().remaining_fraction(now),
            None
        );
        assert!(!state.tick(now + Duration::from_secs(1_000_000)));
        assert!(state.is_some());
        state.show_snackbar("replacement");
        state.dismiss(state.current.as_ref().unwrap().id);
        assert!(state.is_none());
    }
}
