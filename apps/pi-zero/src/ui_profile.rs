use std::time::{Duration, Instant};

const REPORT_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Default)]
struct DurationStats {
    count: u64,
    total: Duration,
    max: Duration,
}

impl DurationStats {
    fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.total += duration;
        self.max = self.max.max(duration);
    }

    fn summary(&self) -> String {
        if self.count == 0 {
            return "n=0".into();
        }
        let avg = self.total.as_micros() / u128::from(self.count);
        format!(
            "n={} avg={}us max={}us",
            self.count,
            avg,
            self.max.as_micros()
        )
    }
}

pub struct UiProfiler {
    enabled: bool,
    last_report: Instant,
    loop_iteration: DurationStats,
    loop_gap: DurationStats,
    runtime_late: DurationStats,
    runtime_advance: DurationStats,
    host_input: DurationStats,
}

impl UiProfiler {
    pub fn from_process() -> Self {
        let enabled = std::env::var("CELLSYMPHONY_PI_UI_PROFILE")
            .map(|value| Self::truthy(&value))
            .unwrap_or(false)
            || std::env::args().any(|arg| arg == "--profile-ui");
        Self::new(enabled)
    }

    #[cfg(test)]
    pub fn from_controls(env_value: Option<&str>, profile_arg: bool) -> Self {
        Self::new(env_value.map(Self::truthy).unwrap_or(false) || profile_arg)
    }

    fn truthy(value: &str) -> bool {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "profile" | "ui" | "yes" | "on"
        )
    }

    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_report: Instant::now(),
            loop_iteration: DurationStats::default(),
            loop_gap: DurationStats::default(),
            runtime_late: DurationStats::default(),
            runtime_advance: DurationStats::default(),
            host_input: DurationStats::default(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn record_loop(&mut self, gap: Duration, iteration: Duration) {
        if self.enabled {
            self.loop_gap.record(gap);
            self.loop_iteration.record(iteration);
        }
    }

    pub fn record_runtime(&mut self, late: Duration, advance: Duration) {
        if self.enabled {
            self.runtime_late.record(late);
            self.runtime_advance.record(advance);
        }
    }

    pub fn record_host_input(&mut self, duration: Duration) {
        if self.enabled {
            self.host_input.record(duration);
        }
    }

    pub fn maybe_report(&mut self) {
        if !self.enabled || self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        eprintln!(
            "pi-ui-profile loop={} gap={} runtime_late={} runtime_advance={} host_input={}",
            self.loop_iteration.summary(),
            self.loop_gap.summary(),
            self.runtime_late.summary(),
            self.runtime_advance.summary(),
            self.host_input.summary(),
        );
        *self = Self::new(true);
    }
}

#[cfg(test)]
mod tests {
    use super::UiProfiler;

    #[test]
    fn env_profile_values_require_truthy_text() {
        for value in ["1", "true", "profile", "ui", "yes", "on"] {
            assert!(UiProfiler::from_controls(Some(value), false).enabled());
        }
        for value in ["0", "false", "", "off", "no"] {
            assert!(!UiProfiler::from_controls(Some(value), false).enabled());
        }
        assert!(UiProfiler::from_controls(Some("0"), true).enabled());
    }
}
