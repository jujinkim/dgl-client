//! Main loop pacing. The loop runs at the 16 ms frame rate while anything moves and slows
//! down when idle so the CPU can reach deep idle states on laptops. User input and remote
//! input sends wake it at once, so typing and echoes never wait for an idle interval.
use std::future::Future;
use std::sync::OnceLock;
use std::task::Poll;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

pub const ACTIVE: Duration = Duration::from_millis(16);
/// Foreground idle: remote output that arrives unprompted shows within this delay.
pub const IDLE: Duration = Duration::from_millis(100);
/// No active window. Stays well under the 3 s sleep-resume gap of `ResumeDetector`.
pub const BACKGROUND: Duration = Duration::from_millis(250);
/// Keep the active rate this long after the last activity, for echoes and output bursts.
pub const LINGER: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct Cadence {
    last_active: Option<Instant>,
}
impl Cadence {
    pub fn mark(&mut self, now: Instant) {
        self.last_active = Some(now);
    }
    pub fn interval(&self, now: Instant, foreground: bool) -> Duration {
        if self
            .last_active
            .is_some_and(|at| now.saturating_duration_since(at) < LINGER)
        {
            ACTIVE
        } else if foreground {
            IDLE
        } else {
            BACKGROUND
        }
    }
}

fn signal() -> &'static Notify {
    static SIGNAL: OnceLock<Notify> = OnceLock::new();
    SIGNAL.get_or_init(Notify::new)
}
/// Run the next tick now and return to the active rate. A wake while a tick runs is kept
/// for the next wait.
pub fn wake() {
    signal().notify_one();
}
/// Waits for `duration` or a [`wake`]; true when woken.
pub async fn sleep(duration: Duration) -> bool {
    let mut timer = std::pin::pin!(gpui::Timer::after(duration));
    let mut woken = std::pin::pin!(signal().notified());
    std::future::poll_fn(|cx| {
        if woken.as_mut().poll(cx).is_ready() {
            Poll::Ready(true)
        } else if timer.as_mut().poll(cx).is_ready() {
            Poll::Ready(false)
        } else {
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_keeps_frame_rate_then_idles_by_window_state() {
        let start = Instant::now();
        let mut cadence = Cadence::default();
        assert_eq!(cadence.interval(start, true), IDLE);
        assert_eq!(cadence.interval(start, false), BACKGROUND);
        cadence.mark(start);
        assert_eq!(cadence.interval(start, false), ACTIVE);
        let lingering = start + LINGER - Duration::from_millis(1);
        assert_eq!(cadence.interval(lingering, true), ACTIVE);
        assert_eq!(cadence.interval(start + LINGER, true), IDLE);
        assert_eq!(cadence.interval(start + LINGER, false), BACKGROUND);
        assert!(BACKGROUND < Duration::from_secs(3) / 4);
    }

    #[test]
    fn a_wake_before_the_wait_ends_it_immediately() {
        wake();
        let woken = block_on(sleep(Duration::from_secs(60)));
        assert!(woken);
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }
}
