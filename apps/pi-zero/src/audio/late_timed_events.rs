//! Timestamped events that reached the engine after their block had started; they played at once.
//! Every engine source the board builds counts into one process-wide counter, which the runtime
//! loop reads in both Perf. Modes.

use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio_engine_source::LateTimedEvents;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

const LOG_INTERVAL: Duration = Duration::from_secs(1);

static LATE_TIMED_EVENTS: LazyLock<Arc<LateTimedEvents>> = LazyLock::new(Arc::default);

pub(crate) fn late_timed_events_counter() -> Arc<LateTimedEvents> {
    Arc::clone(&LATE_TIMED_EVENTS)
}

pub(crate) struct LateTimedEventLog {
    logged: u64,
    next_check: Instant,
}

impl LateTimedEventLog {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            logged: LATE_TIMED_EVENTS.count(),
            next_check: now,
        }
    }

    pub(crate) fn log_increase(&mut self, now: Instant) {
        if now < self.next_check {
            return;
        }
        self.next_check = now + LOG_INTERVAL;
        if let Some(line) = self.increase_line(
            LATE_TIMED_EVENTS.count(),
            LATE_TIMED_EVENTS.take_worst_frames(),
        ) {
            eprintln!("{line}");
        }
    }

    fn increase_line(&mut self, total: u64, worst_frames: u64) -> Option<String> {
        if total <= self.logged {
            return None;
        }
        let worst_us = worst_frames * 1_000_000 / u64::from(DEFAULT_AUDIO_SAMPLE_RATE);
        let line = format!(
            "late timed events: {} (total {total}, worst {worst_frames} frames = {worst_us} us)",
            total - self.logged
        );
        self.logged = total;
        Some(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_increase_is_logged_with_its_delta_and_total() {
        let mut log = LateTimedEventLog {
            logged: 0,
            next_check: Instant::now(),
        };
        assert_eq!(log.increase_line(0, 0), None);
        assert_eq!(
            log.increase_line(3, 441).as_deref(),
            Some("late timed events: 3 (total 3, worst 441 frames = 10000 us)")
        );
        assert_eq!(log.increase_line(3, 0), None);
        assert_eq!(
            log.increase_line(5, 44).as_deref(),
            Some("late timed events: 2 (total 5, worst 44 frames = 997 us)")
        );
    }
}
