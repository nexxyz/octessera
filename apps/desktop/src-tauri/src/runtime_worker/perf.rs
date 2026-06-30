use std::time::{Duration, Instant};

const PERF_LOG_INTERVAL: Duration = Duration::from_secs(2);

pub(super) struct RuntimePerfCounters {
    last_log_at: Instant,
    max_command_ms: u128,
    max_advance_ms: u128,
    max_emit_ms: u128,
}

impl RuntimePerfCounters {
    pub(super) fn new() -> Self {
        Self {
            last_log_at: Instant::now(),
            max_command_ms: 0,
            max_advance_ms: 0,
            max_emit_ms: 0,
        }
    }

    pub(super) fn record_command(&mut self, elapsed: Duration) {
        self.max_command_ms = self.max_command_ms.max(elapsed.as_millis());
        self.maybe_log();
    }

    pub(super) fn record_advance(&mut self, elapsed: Duration) {
        self.max_advance_ms = self.max_advance_ms.max(elapsed.as_millis());
        self.maybe_log();
    }

    pub(super) fn record_emit(&mut self, elapsed: Duration) {
        self.max_emit_ms = self.max_emit_ms.max(elapsed.as_millis());
        self.maybe_log();
    }

    fn maybe_log(&mut self) {
        if self.last_log_at.elapsed() < PERF_LOG_INTERVAL {
            return;
        }
        if self.max_command_ms > 0 || self.max_advance_ms > 0 || self.max_emit_ms > 0 {
            eprintln!(
                "[runtime-perf] max command={}ms advance={}ms emit={}ms",
                self.max_command_ms, self.max_advance_ms, self.max_emit_ms
            );
        }
        self.last_log_at = Instant::now();
        self.max_command_ms = 0;
        self.max_advance_ms = 0;
        self.max_emit_ms = 0;
    }
}
