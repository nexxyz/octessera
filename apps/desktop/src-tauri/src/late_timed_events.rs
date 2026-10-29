//! Timestamped events that reached the engine after their frame had been rendered; they played
//! at the start of the block instead. Every engine source the app starts counts into one
//! process-wide counter, which the runtime worker logs at most once a second.

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
        let total = LATE_TIMED_EVENTS.count();
        if total <= self.logged {
            return;
        }
        let worst_frames = LATE_TIMED_EVENTS.take_worst_frames();
        eprintln!(
            "{}",
            increase_line(total - self.logged, total, worst_frames)
        );
        self.logged = total;
    }
}

fn increase_line(new: u64, total: u64, worst_frames: u64) -> String {
    let worst_us = worst_frames * 1_000_000 / u64::from(DEFAULT_AUDIO_SAMPLE_RATE);
    format!(
        "[runtime-perf] late timed events: {new} (total {total}, worst {worst_frames} frames = {worst_us} us)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_increase_total_and_worst_lateness() {
        assert_eq!(
            increase_line(3, 5, 441),
            "[runtime-perf] late timed events: 3 (total 5, worst 441 frames = 10000 us)"
        );
    }
}
