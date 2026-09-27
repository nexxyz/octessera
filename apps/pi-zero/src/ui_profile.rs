#[cfg(feature = "hardware-orange-pi-zero-2w")]
use playback_runtime::{RunnerMessage, RuntimePlatformEffect};
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
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    scene_capture: DurationStats,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    save_payload: DurationStats,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    save_payload_total: u64,
}

impl UiProfiler {
    pub fn from_process() -> Self {
        let enabled = std::env::var("OCTESSERA_PI_UI_PROFILE")
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
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            scene_capture: DurationStats::default(),
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            save_payload: DurationStats::default(),
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            save_payload_total: 0,
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

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub fn record_scene_capture(&mut self, duration: Duration) {
        if self.enabled {
            self.scene_capture.record(duration);
        }
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub fn record_save_payload(&mut self, duration: Duration, messages: &[RunnerMessage]) {
        if self.enabled
            && messages.iter().any(|message| match message {
                RunnerMessage::PlatformEffects { effects } => effects.iter().any(|effect| {
                    matches!(
                        effect,
                        RuntimePlatformEffect::StoreSaveDefault { .. }
                            | RuntimePlatformEffect::StoreSaveBackup { .. }
                    )
                }),
                _ => false,
            })
        {
            self.save_payload.record(duration);
            self.save_payload_total = self.save_payload_total.saturating_add(1);
        }
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn save_payload_count(&self) -> u64 {
        self.save_payload_total
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn scene_capture_count_for_test(&self) -> u64 {
        self.scene_capture.count
    }

    pub fn maybe_report(&mut self) {
        if !self.enabled || self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        eprintln!(
            "pi-ui-profile loop={} gap={} runtime_late={} runtime_advance={} host_input={} scene_capture={} save_payload={}",
            self.loop_iteration.summary(),
            self.loop_gap.summary(),
            self.runtime_late.summary(),
            self.runtime_advance.summary(),
            self.host_input.summary(),
            self.scene_capture.summary(),
            self.save_payload.summary(),
        );
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        eprintln!(
            "pi-ui-profile loop={} gap={} runtime_late={} runtime_advance={} host_input={}",
            self.loop_iteration.summary(),
            self.loop_gap.summary(),
            self.runtime_late.summary(),
            self.runtime_advance.summary(),
            self.host_input.summary(),
        );
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let save_payload_total = self.save_payload_total;
        *self = Self::new(true);
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        {
            self.save_payload_total = save_payload_total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UiProfiler;
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    use super::REPORT_INTERVAL;
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    use playback_runtime::{NativeRunner, NativeRunnerConfig};
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    use playback_runtime::{RunnerMessage, RuntimePlatformEffect};
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    use serde_json::json;
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    use std::time::{Duration, Instant};

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

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn enabled_profile_aggregates_stages_and_resets_after_reporting() {
        let mut profiler = UiProfiler::from_controls(Some("1"), false);
        profiler.record_scene_capture(Duration::from_micros(10));
        profiler.record_scene_capture(Duration::from_micros(20));
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let no_due_save = runner.flush_due_persistence_music_first().unwrap();
        assert!(no_due_save.is_empty());
        profiler.record_save_payload(Duration::from_micros(50), &no_due_save);
        assert_eq!(profiler.save_payload.summary(), "n=0");
        profiler.record_save_payload(
            Duration::from_micros(5),
            &[RunnerMessage::PlatformEffects {
                effects: vec![RuntimePlatformEffect::StoreSaveDefault {
                    payload: json!({"one": 1}),
                    mode: Some("deferred".into()),
                }],
            }],
        );
        profiler.record_save_payload(
            Duration::from_micros(9),
            &[RunnerMessage::PlatformEffects {
                effects: vec![RuntimePlatformEffect::StoreSaveBackup {
                    payload: json!({"two": 2}),
                }],
            }],
        );

        assert_eq!(profiler.scene_capture.summary(), "n=2 avg=15us max=20us");
        assert_eq!(profiler.save_payload.summary(), "n=2 avg=7us max=9us");

        profiler.last_report = Instant::now()
            .checked_sub(REPORT_INTERVAL)
            .expect("test clock should accommodate one profile interval");
        profiler.maybe_report();

        assert_eq!(profiler.scene_capture.summary(), "n=0");
        assert_eq!(profiler.save_payload.summary(), "n=0");
        assert_eq!(profiler.save_payload_count(), 2);
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn disabled_profile_keeps_stage_statistics_empty() {
        let mut profiler = UiProfiler::from_controls(Some("0"), false);
        profiler.record_scene_capture(Duration::from_micros(10));
        profiler.record_save_payload(
            Duration::from_micros(10),
            &[RunnerMessage::PlatformEffects {
                effects: vec![RuntimePlatformEffect::StoreSaveDefault {
                    payload: json!({}),
                    mode: None,
                }],
            }],
        );

        assert_eq!(profiler.scene_capture.summary(), "n=0");
        assert_eq!(profiler.save_payload.summary(), "n=0");
        assert_eq!(profiler.save_payload_count(), 0);
    }
}
