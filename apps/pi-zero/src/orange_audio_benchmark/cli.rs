use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_WARMUP_SECONDS: u64 = 5;
pub const DEFAULT_MEASURE_SECONDS: u64 = 30;
pub const DEFAULT_RESULT_PATH: &str = "/run/octessera/orange-audio-benchmark-result.json";
pub const DEFAULT_PROGRESS_PATH: &str = "/run/octessera/orange-audio-benchmark-progress.json";
pub const DEFAULT_READINESS_PATH: &str = "/run/octessera/orange-audio-benchmark-readiness.json";
pub const DEFAULT_RELEASE_TIMEOUT_SECONDS: u64 = 30;
#[cfg(not(feature = "routing-tree-benchmark"))]
pub(crate) const ROUTING_TREE_FEATURE_REQUIRED_ERROR: &str =
    "routing_tree_persistent executor requires a binary built with routing-tree-benchmark";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerTimingMode {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkExecutorMode {
    Inline,
    PersistentTwoWorkers,
    RoutingTreePersistent,
}

impl BenchmarkExecutorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::PersistentTwoWorkers => "persistent_two_workers",
            Self::RoutingTreePersistent => "routing_tree_persistent",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "inline" => Some(Self::Inline),
            "persistent_two_workers" => Some(Self::PersistentTwoWorkers),
            "routing_tree_persistent" => Some(Self::RoutingTreePersistent),
            _ => None,
        }
    }
}

impl WorkerTimingMode {
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "enabled" => Some(Self::Enabled),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioId {
    Synth16,
    Synth32,
    Synth64,
    Sample64,
    Mixed16,
    Mixed32,
    BusHeavy,
    Momentary,
    SynthSteal,
    SampleSteal,
    MixedSteal,
    SynthCrossSlot16,
    SampleCrossSlot64,
    Mixed16Synth32Sample,
    Fixed8Synth8Sample12Bus2Global2Momentary,
    SynthCrossSlot32NoSteal,
    MixedRamp16_48,
    DefaultEnvelope24Synth8Sample,
    DefaultHeadroom32Synth8Sample,
    DefaultHeadroom32Synth16Sample,
    DefaultHeadroom40Synth16Sample,
    DefaultHeadroom48Synth16Sample,
    DefaultCapacity64Synth16Sample,
    DefaultCapacity48Synth64Sample,
    DefaultCapacity64Synth64Sample,
}

impl ScenarioId {
    pub const ALL: [Self; 11] = [
        Self::Synth16,
        Self::Synth32,
        Self::Synth64,
        Self::Sample64,
        Self::Mixed16,
        Self::Mixed32,
        Self::BusHeavy,
        Self::Momentary,
        Self::SynthSteal,
        Self::SampleSteal,
        Self::MixedSteal,
    ];

    pub const BASELINE_LIVE: [Self; 14] = [
        Self::SynthCrossSlot16,
        Self::SampleCrossSlot64,
        Self::Mixed16Synth32Sample,
        Self::Fixed8Synth8Sample12Bus2Global2Momentary,
        Self::SynthCrossSlot32NoSteal,
        Self::MixedRamp16_48,
        Self::DefaultEnvelope24Synth8Sample,
        Self::DefaultHeadroom32Synth8Sample,
        Self::DefaultHeadroom32Synth16Sample,
        Self::DefaultHeadroom40Synth16Sample,
        Self::DefaultHeadroom48Synth16Sample,
        Self::DefaultCapacity64Synth16Sample,
        Self::DefaultCapacity48Synth64Sample,
        Self::DefaultCapacity64Synth64Sample,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synth16 => "synth_ramp_16",
            Self::Synth32 => "synth_ramp_32",
            Self::Synth64 => "synth_ramp_64",
            Self::Sample64 => "sample_ramp_64",
            Self::Mixed16 => "mixed_ramp_16_16",
            Self::Mixed32 => "mixed_ramp_32_32",
            Self::BusHeavy => "bus_heavy_6_bus_fx_2_global",
            Self::Momentary => "momentary_combined",
            Self::SynthSteal => "synth_cross_slot_96_steal",
            Self::SampleSteal => "sample_cross_slot_96_steal",
            Self::MixedSteal => "mixed_cross_slot_48_48_steal",
            Self::SynthCrossSlot16 => "synth_cross_slot_16",
            Self::SampleCrossSlot64 => "sample_cross_slot_64",
            Self::Mixed16Synth32Sample => "mixed_16_synth_32_sample",
            Self::Fixed8Synth8Sample12Bus2Global2Momentary => {
                "fixed_8_synth_8_sample_12_bus_2_global_2_momentary"
            }
            Self::SynthCrossSlot32NoSteal => "synth_cross_slot_32_no_steal",
            Self::MixedRamp16_48 => "mixed_ramp_16_48",
            Self::DefaultEnvelope24Synth8Sample => "default_envelope_24_synth_8_sample",
            Self::DefaultHeadroom32Synth8Sample => "default_headroom_32_synth_8_sample",
            Self::DefaultHeadroom32Synth16Sample => "default_headroom_32_synth_16_sample",
            Self::DefaultHeadroom40Synth16Sample => "default_headroom_40_synth_16_sample",
            Self::DefaultHeadroom48Synth16Sample => "default_headroom_48_synth_16_sample",
            Self::DefaultCapacity64Synth16Sample => "default_capacity_64_synth_16_sample",
            Self::DefaultCapacity48Synth64Sample => "default_capacity_48_synth_64_sample",
            Self::DefaultCapacity64Synth64Sample => "default_capacity_64_synth_64_sample",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain(Self::BASELINE_LIVE)
            .find(|id| id.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkConfig {
    pub scenario: String,
    pub output_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub internal_frames: usize,
    pub executor_mode: BenchmarkExecutorMode,
    pub worker_timing_mode: WorkerTimingMode,
    pub warmup_seconds: u64,
    pub measure_seconds: u64,
    pub result_path: PathBuf,
    pub progress_path: PathBuf,
    pub readiness_path: PathBuf,
    pub release_gate_path: PathBuf,
    pub release_timeout_seconds: u64,
    pub artifact_sha256: String,
    pub continue_on_recovered_miss: bool,
}

pub fn requested() -> bool {
    requested_from_args(std::env::args().skip(1))
}

pub fn requested_from_args(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == "--benchmark-orange-audio")
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<BenchmarkConfig, String> {
    let mut benchmark = false;
    let mut scenario = None;
    let mut output_frames = None;
    let mut engine_block_frames = None;
    let mut executor_mode = BenchmarkExecutorMode::PersistentTwoWorkers;
    let mut worker_timing_mode = WorkerTimingMode::Enabled;
    let mut warmup_seconds = DEFAULT_WARMUP_SECONDS;
    let mut measure_seconds = DEFAULT_MEASURE_SECONDS;
    let mut result_path = PathBuf::from(DEFAULT_RESULT_PATH);
    let mut progress_path = PathBuf::from(DEFAULT_PROGRESS_PATH);
    let mut readiness_path = PathBuf::from(DEFAULT_READINESS_PATH);
    let mut release_gate_path = None;
    let mut release_timeout_seconds = DEFAULT_RELEASE_TIMEOUT_SECONDS;
    let mut artifact_sha256 = None;
    let mut continue_on_recovered_miss = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--benchmark-orange-audio" => benchmark = true,
            "--unmuted" | "--no-mute" | "--disable-mute" => {
                return Err("Orange audio benchmark is always post-DSP muted".into())
            }
            "--scenario" => scenario = Some(next_value(&mut iter, "scenario")?),
            "--output-frames" => output_frames = Some(parse_value(&mut iter, "output frames")?),
            "--engine-block-frames" => {
                engine_block_frames = Some(parse_value(&mut iter, "engine block frames")?)
            }
            "--executor" => {
                let value = next_value(&mut iter, "executor")?;
                executor_mode = BenchmarkExecutorMode::parse(&value).ok_or_else(|| {
                    "executor must be inline, persistent_two_workers, or routing_tree_persistent"
                        .to_string()
                })?;
            }
            "--worker-timing" => {
                let value = next_value(&mut iter, "worker timing")?;
                worker_timing_mode = WorkerTimingMode::parse(&value)
                    .ok_or_else(|| "worker timing must be enabled or disabled".to_string())?;
            }
            "--warmup-seconds" => warmup_seconds = parse_value(&mut iter, "warmup seconds")?,
            "--measure-seconds" => measure_seconds = parse_value(&mut iter, "measure seconds")?,
            "--result" => result_path = PathBuf::from(next_value(&mut iter, "result path")?),
            "--progress" => progress_path = PathBuf::from(next_value(&mut iter, "progress path")?),
            "--readiness" => {
                readiness_path = PathBuf::from(next_value(&mut iter, "readiness path")?)
            }
            "--release-gate" => {
                release_gate_path = Some(PathBuf::from(next_value(&mut iter, "release gate path")?))
            }
            "--release-timeout-seconds" => {
                release_timeout_seconds = parse_value(&mut iter, "release timeout seconds")?
            }
            "--artifact-sha256" => {
                artifact_sha256 = Some(next_value(&mut iter, "artifact SHA-256")?)
            }
            "--continue-on-recovered-miss" => continue_on_recovered_miss = true,
            value => return Err(format!("unknown Orange benchmark argument: {value}")),
        }
    }
    if !benchmark {
        return Err("--benchmark-orange-audio is required".into());
    }
    if executor_mode == BenchmarkExecutorMode::Inline
        && worker_timing_mode == WorkerTimingMode::Enabled
    {
        return Err("inline executor requires worker timing disabled".into());
    }
    let scenario = scenario.ok_or_else(|| "an exact --scenario is required".to_string())?;
    if ScenarioId::parse(&scenario).is_none()
        && !crate::dsp_scenarios::is_dynamic_live_scenario_name(&scenario)
    {
        return Err("an exact --scenario is required".into());
    }
    let output_frames = output_frames.ok_or_else(|| "--output-frames is required".to_string())?;
    if !matches!(output_frames, 128 | 256 | 512 | 1024) {
        return Err("output frames must be 128, 256, 512, or 1024".into());
    }
    let expected_alsa_period_frames = match output_frames {
        128 => 32,
        256 => 64,
        512 => 128,
        1024 => 256,
        _ => unreachable!(),
    };
    let internal_frames =
        engine_block_frames.ok_or_else(|| "--engine-block-frames is required".to_string())?;
    if !matches!(internal_frames, 32 | 64 | 128 | 256) {
        return Err("engine block frames must be 32, 64, 128, or 256".into());
    }
    validate_requested_geometry(&scenario, executor_mode, output_frames, internal_frames)?;
    if warmup_seconds != DEFAULT_WARMUP_SECONDS {
        return Err("warmup seconds must be 5".into());
    }
    if !matches!(measure_seconds, 30 | 120 | 180 | 300) {
        return Err("measure seconds must be 30, 120, 180, or 300".into());
    }
    if !(1..=120).contains(&release_timeout_seconds) {
        return Err("release timeout seconds must be between 1 and 120".into());
    }
    let release_gate_path = release_gate_path
        .ok_or_else(|| "an explicit --release-gate path is required".to_string())?;
    if result_path.as_os_str().is_empty()
        || progress_path.as_os_str().is_empty()
        || readiness_path.as_os_str().is_empty()
        || release_gate_path.as_os_str().is_empty()
    {
        return Err("result, progress, readiness, and release paths must not be empty".into());
    }
    if result_path == progress_path
        || result_path == readiness_path
        || progress_path == readiness_path
        || result_path == release_gate_path
        || progress_path == release_gate_path
        || readiness_path == release_gate_path
    {
        return Err("result, progress, readiness, and release paths must differ".into());
    }
    let artifact_sha256 = artifact_sha256
        .ok_or_else(|| "an explicit --artifact-sha256 value is required".to_string())?;
    if artifact_sha256.len() != 64
        || !artifact_sha256
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err("artifact SHA-256 must be 64 hexadecimal characters".into());
    }
    let config = BenchmarkConfig {
        scenario,
        output_frames,
        expected_alsa_period_frames,
        internal_frames,
        executor_mode,
        worker_timing_mode,
        warmup_seconds,
        measure_seconds,
        result_path,
        progress_path,
        readiness_path,
        release_gate_path,
        release_timeout_seconds,
        artifact_sha256,
        continue_on_recovered_miss,
    };
    validate_continue_on_recovered_miss(&config)?;
    Ok(config)
}

pub(crate) fn preflight(config: &BenchmarkConfig) -> Result<(), String> {
    if config.executor_mode == BenchmarkExecutorMode::Inline
        && config.worker_timing_mode == WorkerTimingMode::Enabled
    {
        return Err("inline executor requires worker timing disabled".into());
    }
    #[cfg(not(feature = "routing-tree-benchmark"))]
    if config.executor_mode == BenchmarkExecutorMode::RoutingTreePersistent {
        return Err(ROUTING_TREE_FEATURE_REQUIRED_ERROR.into());
    }
    validate_requested_geometry(
        &config.scenario,
        config.executor_mode,
        config.output_frames,
        config.internal_frames,
    )?;
    validate_continue_on_recovered_miss(config)
}

fn validate_continue_on_recovered_miss(config: &BenchmarkConfig) -> Result<(), String> {
    if !config.continue_on_recovered_miss {
        return Ok(());
    }
    if config.measure_seconds == 120
        && config.scenario == "capacity_analogue_32"
        && config.executor_mode == BenchmarkExecutorMode::RoutingTreePersistent
        && config.worker_timing_mode == WorkerTimingMode::Enabled
        && config.output_frames == 256
        && config.expected_alsa_period_frames == 64
        && config.internal_frames == 64
    {
        return Ok(());
    }
    Err("--continue-on-recovered-miss requires the capacity_analogue_32 routing_tree_persistent 120-second 256/64 observation cell with worker timing enabled".into())
}

pub(crate) fn expected_lookahead_frames(
    executor_mode: BenchmarkExecutorMode,
    internal_frames: usize,
) -> usize {
    match executor_mode {
        BenchmarkExecutorMode::RoutingTreePersistent => internal_frames,
        BenchmarkExecutorMode::Inline | BenchmarkExecutorMode::PersistentTwoWorkers => 0,
    }
}

pub(crate) fn validate_requested_geometry(
    scenario: &str,
    executor_mode: BenchmarkExecutorMode,
    output_frames: u32,
    internal_frames: usize,
) -> Result<(), String> {
    if executor_mode == BenchmarkExecutorMode::RoutingTreePersistent && output_frames > 256 {
        return Err("routing_tree_persistent executor requires output frames <= 256".into());
    }
    let approved = match (output_frames, internal_frames) {
        (128, 32) | (256, 64) | (256, 128) | (256, 256) | (512, 128) | (1024, 256) => true,
        (128, 64) => {
            executor_mode == BenchmarkExecutorMode::Inline
                && is_analogue_capacity_scenario(scenario)
        }
        _ => false,
    };
    if !approved {
        return Err(format!(
            "unsupported Orange benchmark geometry tuple: output={output_frames} internal={internal_frames}"
        ));
    }
    Ok(())
}

pub(crate) struct RecordedGeometry<'a> {
    pub(crate) scenario: &'a str,
    pub(crate) executor_mode: BenchmarkExecutorMode,
    pub(crate) requested_output_buffer_frames: u32,
    pub(crate) expected_alsa_buffer_frames: u32,
    pub(crate) expected_alsa_period_frames: u32,
    pub(crate) internal_block_frames: usize,
    pub(crate) lookahead_frames: usize,
    pub(crate) effective_output_latency_frames: Option<usize>,
}

pub(crate) fn validate_recorded_geometry(geometry: RecordedGeometry<'_>) -> Result<(), String> {
    let RecordedGeometry {
        scenario,
        executor_mode,
        requested_output_buffer_frames,
        expected_alsa_buffer_frames,
        expected_alsa_period_frames,
        internal_block_frames,
        lookahead_frames,
        effective_output_latency_frames,
    } = geometry;
    validate_requested_geometry(
        scenario,
        executor_mode,
        requested_output_buffer_frames,
        internal_block_frames,
    )?;
    let expected_period_frames = match requested_output_buffer_frames {
        128 => 32,
        256 => 64,
        512 => 128,
        1024 => 256,
        _ => return Err("benchmark output frames are invalid".into()),
    };
    if expected_alsa_buffer_frames != requested_output_buffer_frames
        || expected_alsa_period_frames != expected_period_frames
    {
        return Err("benchmark ALSA geometry does not match requested output geometry".into());
    }
    let expected_lookahead = expected_lookahead_frames(executor_mode, internal_block_frames);
    if lookahead_frames != expected_lookahead {
        return Err("benchmark executor lookahead does not match executor geometry".into());
    }
    if let Some(effective) = effective_output_latency_frames {
        if effective != requested_output_buffer_frames as usize + lookahead_frames {
            return Err(
                "benchmark effective output latency does not match benchmark geometry".into(),
            );
        }
    }
    Ok(())
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
fn is_analogue_capacity_scenario(scenario: &str) -> bool {
    crate::dsp_profile::analogue_capacity_scenario::parse(scenario).is_some()
}

#[cfg(not(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
)))]
fn is_analogue_capacity_scenario(scenario: &str) -> bool {
    let _ = scenario;
    false
}

fn next_value(iter: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    iter.next().ok_or_else(|| format!("missing {name} value"))
}

fn parse_value<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    next_value(iter, name)?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
