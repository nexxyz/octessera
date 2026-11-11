use super::I2C_ERROR_BACKOFF;
use std::time::{Duration, Instant};

/// Both boards scan on the shared Seesaw INT line; this slow poll only catches a missed edge.
pub(super) const INPUT_SAFETY_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// The NeoKey is read as a level snapshot, so a key that is mid-debounce needs prompt rescans.
pub(super) const NEOKEY_SETTLE_SCAN_INTERVAL: Duration = Duration::from_millis(4);
/// A line still low after a scan that found no input is held, not signalling; rescanning it on
/// every worker pass would keep the bus busy, so its level only rescans this often.
pub(super) const HELD_LINE_RESCAN_INTERVAL: Duration = Duration::from_millis(10);

pub(super) struct ScanSchedule {
    next_scan_at: Instant,
    next_level_scan_at: Instant,
    backing_off: bool,
}

impl ScanSchedule {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_scan_at: now,
            next_level_scan_at: now,
            backing_off: false,
        }
    }

    pub(super) fn scan_due(&self, now: Instant, edge: bool, line_low: bool) -> bool {
        now >= self.next_scan_at
            || (!self.backing_off && (edge || (line_low && now >= self.next_level_scan_at)))
    }

    pub(super) fn backing_off(&self) -> bool {
        self.backing_off
    }

    pub(super) fn record_scan(
        &mut self,
        scanned: bool,
        found_input: bool,
        neokey_settling: bool,
        now: Instant,
    ) {
        self.backing_off = !scanned;
        self.next_level_scan_at = if scanned && !found_input {
            now + HELD_LINE_RESCAN_INTERVAL
        } else {
            now
        };
        self.next_scan_at = now
            + if !scanned {
                I2C_ERROR_BACKOFF
            } else if neokey_settling {
                NEOKEY_SETTLE_SCAN_INTERVAL
            } else {
                INPUT_SAFETY_POLL_INTERVAL
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
        schedule.record_scan(true, true, false, start);
        let soon = start + Duration::from_millis(3);
        assert!(schedule.scan_due(soon, false, true));
        assert!(schedule.scan_due(soon, true, false));
    }

    #[test]
    fn idle_line_waits_for_the_safety_poll() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        assert!(schedule.scan_due(start, false, false));
        schedule.record_scan(true, false, false, start);
        assert!(!schedule.scan_due(start + Duration::from_millis(99), false, false));
        assert!(schedule.scan_due(start + Duration::from_millis(100), false, false));
    }

    #[test]
    fn failed_scan_ignores_the_interrupt_until_back_off_ends() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        schedule.record_scan(false, false, false, start);
        assert!(schedule.backing_off());
        assert!(!schedule.scan_due(start + Duration::from_millis(49), true, true));
        assert!(schedule.scan_due(start + Duration::from_millis(50), true, true));
        schedule.record_scan(true, true, false, start + Duration::from_millis(50));
        assert!(schedule.scan_due(start + Duration::from_millis(51), false, true));
    }

    #[test]
    fn settling_neokey_rescans_before_the_safety_poll() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        schedule.record_scan(true, false, true, start);
        assert!(!schedule.scan_due(start + Duration::from_millis(3), false, false));
        assert!(schedule.scan_due(start + NEOKEY_SETTLE_SCAN_INTERVAL, false, false));
    }

    #[test]
    fn a_line_held_low_after_an_empty_scan_rescans_at_a_bounded_rate() {
        let start = Instant::now();
        let mut schedule = ScanSchedule::new(start);
        schedule.record_scan(true, false, false, start);

        assert!(!schedule.scan_due(start + Duration::from_millis(2), false, true));
        assert!(!schedule.scan_due(
            start + HELD_LINE_RESCAN_INTERVAL - Duration::from_millis(1),
            false,
            true
        ));
        assert!(schedule.scan_due(start + HELD_LINE_RESCAN_INTERVAL, false, true));
        assert!(schedule.scan_due(start + Duration::from_millis(2), true, true));
    }
}
