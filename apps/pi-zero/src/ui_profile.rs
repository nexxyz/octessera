use crate::render::RenderProfileMetrics;
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
    render: DurationStats,
    render_overrun: DurationStats,
    snapshot_clone: DurationStats,
    config_sync: DurationStats,
    grid_poll: DurationStats,
    neokey_poll: DurationStats,
    led_extract: DurationStats,
    led_write: DurationStats,
    render_neokey_build: DurationStats,
    render_neokey_write: DurationStats,
    oled_signature: DurationStats,
    oled_frame_build: DurationStats,
    oled_write: DurationStats,
    oled_rendered: u64,
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
            render: DurationStats::default(),
            render_overrun: DurationStats::default(),
            snapshot_clone: DurationStats::default(),
            config_sync: DurationStats::default(),
            grid_poll: DurationStats::default(),
            neokey_poll: DurationStats::default(),
            led_extract: DurationStats::default(),
            led_write: DurationStats::default(),
            render_neokey_build: DurationStats::default(),
            render_neokey_write: DurationStats::default(),
            oled_signature: DurationStats::default(),
            oled_frame_build: DurationStats::default(),
            oled_write: DurationStats::default(),
            oled_rendered: 0,
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

    pub fn record_poll(&mut self, grid: Duration, neokey: Duration) {
        if self.enabled {
            self.grid_poll.record(grid);
            self.neokey_poll.record(neokey);
        }
    }

    pub fn record_render(
        &mut self,
        total: Duration,
        interval: Duration,
        snapshot_clone: Duration,
        config_sync: Duration,
        metrics: &RenderProfileMetrics,
    ) {
        if !self.enabled {
            return;
        }
        self.render.record(total);
        self.snapshot_clone.record(snapshot_clone);
        self.config_sync.record(config_sync);
        if total > interval {
            self.render_overrun.record(total - interval);
        }
        self.led_extract.record(metrics.led_extract);
        self.led_write.record(metrics.led_write);
        self.render_neokey_build.record(metrics.neokey_build);
        self.render_neokey_write.record(metrics.neokey_write);
        self.oled_signature.record(metrics.oled_signature);
        if metrics.oled_rendered {
            self.oled_rendered += 1;
            self.oled_frame_build.record(metrics.oled_frame_build);
            self.oled_write.record(metrics.oled_write);
        }
    }

    pub fn maybe_report(&mut self) {
        if !self.enabled || self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        eprintln!(
            "pi-ui-profile loop={} gap={} runtime_late={} runtime_advance={} render={} render_overrun={} snapshot_clone={} config_sync={} grid_poll={} neokey_poll={} led_extract={} led_write={} render_neokey_build={} render_neokey_write={} oled_signature={} oled_rendered={} oled_frame_build={} oled_write={}",
            self.loop_iteration.summary(),
            self.loop_gap.summary(),
            self.runtime_late.summary(),
            self.runtime_advance.summary(),
            self.render.summary(),
            self.render_overrun.summary(),
            self.snapshot_clone.summary(),
            self.config_sync.summary(),
            self.grid_poll.summary(),
            self.neokey_poll.summary(),
            self.led_extract.summary(),
            self.led_write.summary(),
            self.render_neokey_build.summary(),
            self.render_neokey_write.summary(),
            self.oled_signature.summary(),
            self.oled_rendered,
            self.oled_frame_build.summary(),
            self.oled_write.summary(),
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
