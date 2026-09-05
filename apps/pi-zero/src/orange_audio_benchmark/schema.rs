use super::cli::BenchmarkConfig;
use super::metrics::CallbackMetricsSnapshot;
use realtime_engine::synth::{
    SourceWorkerCoordinatorTimingSnapshot, SourceWorkerHealth, SourceWorkerTimingSnapshot,
    SourceWorkerWorkerTimingSnapshot, SynthProfileSnapshot,
};
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "schema_deserialization.rs"]
mod deserialization;
#[path = "result.rs"]
mod result;
#[path = "worker_timing_validation.rs"]
mod worker_timing_validation;
pub use super::output_counters::PersistentOutputCountersEvidence;
pub use result::BenchmarkResult;

const BENCHMARK_SCHEMA_VERSION: u8 = 5;
const BENCHMARK_RESULT_SCHEMA_VERSION: u8 = 12;
const BENCHMARK_RELEASE_SCHEMA_VERSION: u8 = 2;

fn deserialize_result_schema_v12<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version != BENCHMARK_RESULT_SCHEMA_VERSION {
        return Err(D::Error::custom(format!(
            "benchmark result schema version {version} is not supported"
        )));
    }
    Ok(version)
}

fn deserialize_release_schema_v2<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version != BENCHMARK_RELEASE_SCHEMA_VERSION {
        return Err(D::Error::custom(format!(
            "benchmark release schema version {version} is not supported"
        )));
    }
    Ok(version)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct BenchmarkProgress {
    pub schema_version: u8,
    pub kind: String,
    pub status: String,
    pub phase: String,
    pub elapsed_seconds: u64,
    pub target_seconds: u64,
    pub board_profile: String,
    pub pid: u32,
    pub systemd_invocation_id: Option<String>,
    pub artifact_sha256: String,
    pub scenario: String,
    pub requested_output_buffer_frames: u32,
    pub expected_alsa_buffer_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub internal_block_frames: usize,
    pub lookahead_frames: usize,
    pub callback_frames_min: u32,
    pub callback_frames_max: u32,
    pub callback_frame_sample_count: u64,
    pub callback_frame_size_change_count: u64,
    pub invalid_callback_frame_count: u64,
    pub lifetime_callback_frames_min: u32,
    pub lifetime_callback_frames_max: u32,
    pub lifetime_callback_frame_sample_count: u64,
    pub lifetime_callback_frame_size_change_count: u64,
    pub lifetime_invalid_callback_frame_count: u64,
    pub lifetime_callback_count: u64,
    pub measured_callback_count: u64,
    pub cpal_device_error_count: u64,
    pub cpal_stream_error_count: u64,
    pub terminal_error: bool,
    pub post_dsp_zero: bool,
    pub executor_mode: String,
    pub worker_health: String,
    pub worker_thread_name_0: String,
    pub worker_thread_name_1: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct BenchmarkReadiness {
    pub schema_version: u8,
    pub kind: String,
    pub status: String,
    pub board_profile: String,
    pub pid: u32,
    pub systemd_invocation_id: String,
    pub artifact_sha256: String,
    pub scenario: String,
    pub requested_output_buffer_frames: u32,
    pub expected_alsa_buffer_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub internal_block_frames: usize,
    pub lookahead_frames: usize,
    pub callback_frames_min: u32,
    pub callback_frames_max: u32,
    pub callback_frame_sample_count: u64,
    pub callback_frame_size_change_count: u64,
    pub invalid_callback_frame_count: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub scheduler_qualified: bool,
    pub post_dsp_zero: bool,
    pub executor_mode: String,
    pub worker_health: String,
    pub worker_thread_name_0: String,
    pub worker_thread_name_1: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkWorkerTiming {
    pub workers: [BenchmarkWorkerTimingWorker; 2],
    pub coordinator: BenchmarkCoordinatorTiming,
    pub late_after_deadline_ns: Option<u64>,
    pub cpu_endpoint_changed: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkWorkerTimingWorker {
    pub sequence: Option<u64>,
    pub render_ns: Option<u64>,
    pub dispatch_to_finish_ns: Option<u64>,
    pub cpu_start: Option<u32>,
    pub cpu_end: Option<u32>,
    pub finished: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkCoordinatorTiming {
    pub sequence: Option<u64>,
    pub deadline_ns: Option<u64>,
    pub dispatch_to_deadline_start_ns: Option<u64>,
    pub dispatch_to_deadline_elapsed_ns: Option<u64>,
    pub in_flight_mask: Option<u8>,
    pub completed_mask: Option<u8>,
    pub first_parity: Option<usize>,
    pub dispatch_to_first_ns: Option<u64>,
    pub dispatch_to_both_ns: Option<u64>,
    pub reduction_ns: Option<u64>,
    pub coordinator_remainder_ns: Option<u64>,
    pub engine_block_total_ns: Option<u64>,
    pub callback_total_ns: Option<u64>,
    pub failed: bool,
    pub frozen: bool,
}

impl From<SourceWorkerTimingSnapshot> for BenchmarkWorkerTiming {
    fn from(snapshot: SourceWorkerTimingSnapshot) -> Self {
        Self {
            workers: snapshot.workers.map(BenchmarkWorkerTimingWorker::from),
            coordinator: BenchmarkCoordinatorTiming::from(snapshot.coordinator),
            late_after_deadline_ns: snapshot.late_after_deadline_ns,
            cpu_endpoint_changed: snapshot.cpu_endpoint_changed,
        }
    }
}

impl From<SourceWorkerWorkerTimingSnapshot> for BenchmarkWorkerTimingWorker {
    fn from(snapshot: SourceWorkerWorkerTimingSnapshot) -> Self {
        Self {
            sequence: snapshot.sequence,
            render_ns: snapshot.render_ns,
            dispatch_to_finish_ns: snapshot.dispatch_to_finish_ns,
            cpu_start: snapshot.cpu_start,
            cpu_end: snapshot.cpu_end,
            finished: snapshot.finished,
        }
    }
}

impl From<SourceWorkerCoordinatorTimingSnapshot> for BenchmarkCoordinatorTiming {
    fn from(snapshot: SourceWorkerCoordinatorTimingSnapshot) -> Self {
        Self {
            sequence: snapshot.sequence,
            deadline_ns: snapshot.deadline_ns,
            dispatch_to_deadline_start_ns: snapshot.dispatch_to_deadline_start_ns,
            dispatch_to_deadline_elapsed_ns: snapshot.dispatch_to_deadline_elapsed_ns,
            in_flight_mask: snapshot.in_flight_mask,
            completed_mask: snapshot.completed_mask,
            first_parity: snapshot.first_parity,
            dispatch_to_first_ns: snapshot.dispatch_to_first_ns,
            dispatch_to_both_ns: snapshot.dispatch_to_both_ns,
            reduction_ns: snapshot.reduction_ns,
            coordinator_remainder_ns: snapshot.coordinator_remainder_ns,
            engine_block_total_ns: snapshot.engine_block_total_ns,
            callback_total_ns: snapshot.callback_total_ns,
            failed: snapshot.failed,
            frozen: snapshot.frozen,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkProfileSnapshot {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_preview_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub active_bus_fx_slots: usize,
    pub active_global_fx_slots: usize,
    pub cumulative_voice_steals: u64,
    pub cumulative_voice_admission_drops: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReleaseGate {
    #[serde(deserialize_with = "deserialize_release_schema_v2")]
    pub schema_version: u8,
    pub kind: String,
    pub status: String,
    pub board_profile: String,
    pub pid: u32,
    pub systemd_invocation_id: String,
    pub artifact_sha256: String,
    pub scenario: String,
    pub expected_alsa_buffer_frames: u32,
    pub observed_alsa_buffer_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub observed_alsa_period_frames: u32,
}

impl From<SynthProfileSnapshot> for BenchmarkProfileSnapshot {
    fn from(snapshot: SynthProfileSnapshot) -> Self {
        Self {
            active_synth_voices: snapshot.active_synth_voices,
            active_sample_voices: snapshot.active_sample_voices,
            active_preview_sample_voices: snapshot.active_preview_sample_voices,
            active_momentary_fx: snapshot.active_momentary_fx,
            active_bus_fx_slots: snapshot.active_bus_fx_slots,
            active_global_fx_slots: snapshot.active_global_fx_slots,
            cumulative_voice_steals: snapshot.cumulative_voice_steals,
            cumulative_voice_admission_drops: snapshot.cumulative_voice_admission_drops,
        }
    }
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}-{timestamp}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("benchmark"),
        std::process::id()
    ));
    let content = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&content)
            .map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

impl BenchmarkProgress {
    pub fn new(
        config: &BenchmarkConfig,
        phase: &str,
        elapsed_seconds: u64,
        target_seconds: u64,
        metrics: &CallbackMetricsSnapshot,
        worker_health: SourceWorkerHealth,
    ) -> Self {
        let worker_thread_names =
            super::stream::worker_thread_names_for_executor(config.executor_mode);
        Self {
            schema_version: BENCHMARK_SCHEMA_VERSION,
            kind: "orange_audio_benchmark_progress".into(),
            status: phase.into(),
            phase: phase.into(),
            elapsed_seconds,
            target_seconds,
            board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
            pid: std::process::id(),
            systemd_invocation_id: std::env::var("INVOCATION_ID").ok(),
            artifact_sha256: config.artifact_sha256.clone(),
            scenario: config.scenario.as_str().into(),
            requested_output_buffer_frames: config.output_frames,
            expected_alsa_buffer_frames: config.output_frames,
            expected_alsa_period_frames: config.expected_alsa_period_frames,
            internal_block_frames: config.internal_frames,
            lookahead_frames: super::cli::expected_lookahead_frames(
                config.executor_mode,
                config.internal_frames,
            ),
            callback_frames_min: metrics.callback_frames_min,
            callback_frames_max: metrics.callback_frames_max,
            callback_frame_sample_count: metrics.callback_frame_sample_count,
            callback_frame_size_change_count: metrics.callback_frame_size_change_count,
            invalid_callback_frame_count: metrics.invalid_callback_frame_count,
            lifetime_callback_frames_min: metrics.lifetime_callback_frames_min,
            lifetime_callback_frames_max: metrics.lifetime_callback_frames_max,
            lifetime_callback_frame_sample_count: metrics.lifetime_callback_frame_sample_count,
            lifetime_callback_frame_size_change_count: metrics
                .lifetime_callback_frame_size_change_count,
            lifetime_invalid_callback_frame_count: metrics.lifetime_invalid_callback_frame_count,
            lifetime_callback_count: metrics.lifetime_callback_count,
            measured_callback_count: metrics.callback_count,
            cpal_device_error_count: metrics.cpal_device_error_count,
            cpal_stream_error_count: metrics.cpal_stream_error_count,
            terminal_error: metrics.terminal_error,
            post_dsp_zero: metrics.lifetime_callback_count > 0
                && metrics.post_mute_nonzero_samples == 0,
            executor_mode: config.executor_mode.as_str().into(),
            worker_health: worker_health.name().into(),
            worker_thread_name_0: worker_thread_names[0].clone(),
            worker_thread_name_1: worker_thread_names[1].clone(),
        }
    }
}

pub fn readiness(
    config: &BenchmarkConfig,
    invocation_id: &str,
    sample_format: &str,
    channels: u16,
    sample_rate: u32,
    metrics: &CallbackMetricsSnapshot,
    worker_health: SourceWorkerHealth,
) -> BenchmarkReadiness {
    let worker_thread_names = super::stream::worker_thread_names_for_executor(config.executor_mode);
    BenchmarkReadiness {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        kind: "orange_audio_benchmark_readiness".into(),
        status: "ready".into(),
        board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
        pid: std::process::id(),
        systemd_invocation_id: invocation_id.into(),
        artifact_sha256: config.artifact_sha256.clone(),
        scenario: config.scenario.as_str().into(),
        requested_output_buffer_frames: config.output_frames,
        expected_alsa_buffer_frames: config.output_frames,
        expected_alsa_period_frames: config.expected_alsa_period_frames,
        internal_block_frames: config.internal_frames,
        lookahead_frames: super::cli::expected_lookahead_frames(
            config.executor_mode,
            config.internal_frames,
        ),
        callback_frames_min: metrics.lifetime_callback_frames_min,
        callback_frames_max: metrics.lifetime_callback_frames_max,
        callback_frame_sample_count: metrics.lifetime_callback_frame_sample_count,
        callback_frame_size_change_count: metrics.lifetime_callback_frame_size_change_count,
        invalid_callback_frame_count: metrics.lifetime_invalid_callback_frame_count,
        sample_rate,
        channels,
        sample_format: sample_format.into(),
        scheduler_qualified: true,
        post_dsp_zero: true,
        executor_mode: config.executor_mode.as_str().into(),
        worker_health: worker_health.name().into(),
        worker_thread_name_0: worker_thread_names[0].clone(),
        worker_thread_name_1: worker_thread_names[1].clone(),
    }
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
