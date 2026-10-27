use super::I2C_ERROR_BACKOFF;
use std::time::{Duration, Instant};

/// Both boards scan on the shared Seesaw INT line; this slow poll only catches a missed edge.
pub(super) const INPUT_SAFETY_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct ScanSchedule {
    next_scan_at: Instant,
    backing_off: bool,
}

impl ScanSchedule {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_scan_at: now,
            backing_off: false,
        }
    }

    pub(super) fn scan_due(&self, now: Instant, interrupt_pending: bool) -> bool {
        now >= self.next_scan_at || (interrupt_pending && !self.backing_off)
    }

    pub(super) fn record_scan(&mut self, scanned: bool, now: Instant) {
        self.backing_off = !scanned;
        self.next_scan_at = now
            + if scanned {
                INPUT_SAFETY_POLL_INTERVAL
            } else {
                I2C_ERROR_BACKOFF
            };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asserted_interrupt_scans_before_the_safety_poll() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        schedule.record_scan(true, start);
        let soon = start + Duration::from_millis(3);
        assert!(schedule.scan_due(soon, true));
    }

    #[test]
    fn idle_line_waits_for_the_safety_poll() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        assert!(schedule.scan_due(start, false));
        schedule.record_scan(true, start);
        assert!(!schedule.scan_due(start + Duration::from_millis(99), false));
        assert!(schedule.scan_due(start + Duration::from_millis(100), false));
    }

    #[test]
    fn failed_scan_ignores_the_interrupt_until_back_off_ends() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        schedule.record_scan(false, start);
        assert!(!schedule.scan_due(start + Duration::from_millis(49), true));
        assert!(schedule.scan_due(start + Duration::from_millis(50), true));
        schedule.record_scan(true, start + Duration::from_millis(50));
        assert!(schedule.scan_due(start + Duration::from_millis(51), true));
    }
}
