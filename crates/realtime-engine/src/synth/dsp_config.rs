use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
const PPM_PER_PERCENT: u32 = 10_000;
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
const CLEAR_HYSTERESIS_PERCENT: u32 = 5;
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
const CLEAR_DURATION_SECONDS: u64 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DspRuntimeConfig {
    #[serde(default = "default_worker_warning_threshold")]
    pub worker_warning_threshold: WorkerWarningThreshold,
    #[serde(default = "default_bus_idle_threshold")]
    pub bus_idle_threshold: BusIdleThreshold,
}

impl Default for DspRuntimeConfig {
    fn default() -> Self {
        Self {
            worker_warning_threshold: WorkerWarningThreshold::Percent85,
            bus_idle_threshold: BusIdleThreshold::Db120,
        }
    }
}

impl DspRuntimeConfig {
    pub fn from_value(value: &Value) -> Result<Self, String> {
        serde_json::from_value(value.clone())
            .map_err(|error| format!("invalid DSP runtime config: {error}"))
    }

    pub fn to_value(self) -> Result<Value, String> {
        serde_json::to_value(self)
            .map_err(|error| format!("DSP runtime config encode failed: {error}"))
    }
}

fn default_worker_warning_threshold() -> WorkerWarningThreshold {
    WorkerWarningThreshold::Percent85
}

fn default_bus_idle_threshold() -> BusIdleThreshold {
    BusIdleThreshold::Db120
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerWarningThreshold {
    #[serde(rename = "70")]
    Percent70,
    #[serde(rename = "75")]
    Percent75,
    #[serde(rename = "80")]
    Percent80,
    #[serde(rename = "85")]
    Percent85,
    #[serde(rename = "90")]
    Percent90,
    #[serde(rename = "95")]
    Percent95,
}

impl WorkerWarningThreshold {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Percent70 => "70",
            Self::Percent75 => "75",
            Self::Percent80 => "80",
            Self::Percent85 => "85",
            Self::Percent90 => "90",
            Self::Percent95 => "95",
        }
    }

    pub const fn percent(self) -> u8 {
        match self {
            Self::Percent70 => 70,
            Self::Percent75 => 75,
            Self::Percent80 => 80,
            Self::Percent85 => 85,
            Self::Percent90 => 90,
            Self::Percent95 => 95,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "70" => Some(Self::Percent70),
            "75" => Some(Self::Percent75),
            "80" => Some(Self::Percent80),
            "85" => Some(Self::Percent85),
            "90" => Some(Self::Percent90),
            "95" => Some(Self::Percent95),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusIdleThreshold {
    #[serde(rename = "exact")]
    Exact,
    #[serde(rename = "-140")]
    Db140,
    #[serde(rename = "-120")]
    Db120,
    #[serde(rename = "-100")]
    Db100,
    #[serde(rename = "-80")]
    Db80,
}

impl BusIdleThreshold {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Db140 => "-140",
            Self::Db120 => "-120",
            Self::Db100 => "-100",
            Self::Db80 => "-80",
        }
    }

    pub const fn amplitude(self) -> f32 {
        match self {
            Self::Exact => 0.0,
            Self::Db140 => 0.0000001,
            Self::Db120 => 0.000001,
            Self::Db100 => 0.00001,
            Self::Db80 => 0.0001,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "exact" => Some(Self::Exact),
            "-140" => Some(Self::Db140),
            "-120" => Some(Self::Db120),
            "-100" => Some(Self::Db100),
            "-80" => Some(Self::Db80),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WorkerLoadWarningState {
    threshold: WorkerWarningThreshold,
    high_cpu_steady: bool,
    clear_frames: u64,
}

impl Default for WorkerLoadWarningState {
    fn default() -> Self {
        Self {
            threshold: WorkerWarningThreshold::Percent85,
            high_cpu_steady: false,
            clear_frames: 0,
        }
    }
}

impl WorkerLoadWarningState {
    pub(super) fn set_threshold(&mut self, threshold: WorkerWarningThreshold) {
        if self.threshold != threshold {
            self.threshold = threshold;
            self.clear_frames = 0;
        }
    }

    #[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
    pub(super) fn observe(
        &mut self,
        utilization_ppm: u32,
        rendered_frames: usize,
        sample_rate: u32,
    ) {
        let threshold_ppm = u32::from(self.threshold.percent()) * PPM_PER_PERCENT;
        let clear_threshold_ppm =
            threshold_ppm.saturating_sub(CLEAR_HYSTERESIS_PERCENT * PPM_PER_PERCENT);
        if utilization_ppm >= threshold_ppm {
            self.high_cpu_steady = true;
            self.clear_frames = 0;
            return;
        }
        if utilization_ppm >= clear_threshold_ppm || !self.high_cpu_steady {
            self.clear_frames = 0;
            return;
        }
        if sample_rate == 0 {
            return;
        }
        let required_frames = u64::from(sample_rate).saturating_mul(CLEAR_DURATION_SECONDS);
        let rendered_frames = u64::try_from(rendered_frames).unwrap_or(u64::MAX);
        self.clear_frames = self
            .clear_frames
            .saturating_add(rendered_frames)
            .min(required_frames);
        if self.clear_frames >= required_frames {
            self.high_cpu_steady = false;
            self.clear_frames = 0;
        }
    }

    pub(super) const fn high_cpu_steady(self) -> bool {
        self.high_cpu_steady
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const WORKER_THRESHOLDS: [WorkerWarningThreshold; 6] = [
        WorkerWarningThreshold::Percent70,
        WorkerWarningThreshold::Percent75,
        WorkerWarningThreshold::Percent80,
        WorkerWarningThreshold::Percent85,
        WorkerWarningThreshold::Percent90,
        WorkerWarningThreshold::Percent95,
    ];

    #[test]
    fn dsp_runtime_config_round_trips_and_defaults() {
        assert_eq!(
            DspRuntimeConfig::from_value(&json!({})).unwrap(),
            DspRuntimeConfig::default()
        );
        assert_eq!(
            DspRuntimeConfig::default().to_value().unwrap(),
            json!({ "busIdleThreshold": "-120", "workerWarningThreshold": "85" })
        );
    }

    #[test]
    fn dsp_runtime_config_rejects_unknown_or_invalid_values() {
        assert!(DspRuntimeConfig::from_value(&json!({
            "workerWarningThreshold": "86"
        }))
        .is_err());
        assert!(DspRuntimeConfig::from_value(&json!({ "unknown": "value" })).is_err());
    }

    #[test]
    fn bus_idle_threshold_maps_to_linear_amplitude() {
        assert_eq!(BusIdleThreshold::Exact.amplitude(), 0.0);
        assert_eq!(BusIdleThreshold::Db140.amplitude(), 0.0000001);
        assert_eq!(BusIdleThreshold::Db120.amplitude(), 0.000001);
        assert_eq!(BusIdleThreshold::Db100.amplitude(), 0.00001);
        assert_eq!(BusIdleThreshold::Db80.amplitude(), 0.0001);
    }

    #[test]
    fn worker_warning_threshold_boundaries_cover_every_option() {
        for threshold in WORKER_THRESHOLDS {
            let mut state = WorkerLoadWarningState::default();
            state.set_threshold(threshold);
            let threshold_ppm = u32::from(threshold.percent()) * PPM_PER_PERCENT;
            state.observe(threshold_ppm - 1, 128, 48_000);
            assert!(!state.high_cpu_steady());
            state.observe(threshold_ppm, 128, 48_000);
            assert!(state.high_cpu_steady());
        }
    }

    #[test]
    fn worker_warning_clears_at_frame_count_edges_for_supported_rates_and_quanta() {
        for sample_rate in [44_100, 48_000] {
            for quantum in [32, 64, 128, 256, 2_048] {
                let mut state = WorkerLoadWarningState::default();
                state.observe(850_000, quantum, sample_rate);
                let required_frames = u64::from(sample_rate) * CLEAR_DURATION_SECONDS;
                let full_blocks = required_frames / quantum as u64;
                for _ in 0..full_blocks.saturating_sub(1) {
                    state.observe(799_999, quantum, sample_rate);
                }
                assert!(state.high_cpu_steady());
                state.observe(799_999, quantum, sample_rate);
                if required_frames.is_multiple_of(quantum as u64) {
                    assert!(!state.high_cpu_steady());
                } else {
                    assert!(state.high_cpu_steady());
                    state.observe(799_999, quantum, sample_rate);
                    assert!(!state.high_cpu_steady());
                }
            }
        }
    }

    #[test]
    fn worker_warning_middle_band_holds_and_resets_clear_accumulation() {
        let mut state = WorkerLoadWarningState::default();
        state.observe(850_000, 128, 48_000);
        for _ in 0..1_000 {
            state.observe(799_999, 128, 48_000);
        }
        state.observe(800_000, 128, 48_000);
        assert!(state.high_cpu_steady());
        for _ in 0..1_875 - 1 {
            state.observe(799_999, 128, 48_000);
        }
        assert!(state.high_cpu_steady());
        state.observe(799_999, 128, 48_000);
        assert!(!state.high_cpu_steady());
    }

    #[test]
    fn worker_warning_clear_boundary_equality_holds_and_resets_accumulation() {
        let mut state = WorkerLoadWarningState::default();
        state.observe(850_000, 128, 48_000);
        state.observe(799_999, 128, 48_000);
        assert_eq!(state.clear_frames, 128);

        state.observe(800_000, 128, 48_000);
        assert!(state.high_cpu_steady());
        assert_eq!(state.clear_frames, 0);
    }

    #[test]
    fn worker_warning_threshold_changes_apply_on_the_next_observation() {
        let mut state = WorkerLoadWarningState::default();
        state.set_threshold(WorkerWarningThreshold::Percent95);
        state.observe(900_000, 128, 48_000);
        assert!(!state.high_cpu_steady());
        state.set_threshold(WorkerWarningThreshold::Percent85);
        assert!(!state.high_cpu_steady());
        state.observe(900_000, 128, 48_000);
        assert!(state.high_cpu_steady());
    }

    #[test]
    fn worker_warning_evaluation_does_not_allocate() {
        let mut state = WorkerLoadWarningState::default();
        let (_, allocations, deallocations) =
            crate::synth::test_allocator::count_allocations_and_deallocations(|| {
                state.observe(850_000, 128, 48_000);
                state.observe(799_999, 128, 48_000);
            });
        assert_eq!((allocations, deallocations), (0, 0));
    }
}
