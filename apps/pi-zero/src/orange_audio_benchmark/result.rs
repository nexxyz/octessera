use super::super::cli::WorkerTimingMode;
use super::{
    deserialize_result_schema_v4, BenchmarkProfileSnapshot, BenchmarkWorkerTiming,
    CallbackMetricsSnapshot,
};
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, PartialEq)]
pub struct BenchmarkResult {
    pub schema_version: u8,
    pub kind: String,
    pub status: String,
    pub board_profile: String,
    pub scenario: String,
    pub requested_output_buffer_frames: u32,
    pub expected_alsa_buffer_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub internal_block_frames: usize,
    pub sample_format: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub warmup_seconds: u64,
    pub measure_seconds: u64,
    pub scheduler_qualified: bool,
    pub post_dsp_zero: bool,
    pub measurement_stop_acknowledged: bool,
    pub stream_stopped: bool,
    pub final_progress_write_succeeded: bool,
    pub pid: u32,
    pub systemd_invocation_id: Option<String>,
    pub artifact_sha256: String,
    pub callback: CallbackMetricsSnapshot,
    pub profile_start: BenchmarkProfileSnapshot,
    pub profile_end: BenchmarkProfileSnapshot,
    pub recovered_alsa_epipe_count: Option<u64>,
    pub recovered_alsa_epipe_observable: bool,
    pub terminal_error: Option<String>,
    pub executor_mode: String,
    pub worker_health: String,
    pub worker_thread_name_0: String,
    pub worker_thread_name_1: String,
    pub joined_workers: usize,
    pub retirement_error: Option<String>,
    pub worker_timing_mode: WorkerTimingMode,
    pub worker_timing: Option<BenchmarkWorkerTiming>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkResultUnchecked {
    #[serde(deserialize_with = "deserialize_result_schema_v4")]
    schema_version: u8,
    kind: String,
    status: String,
    board_profile: String,
    scenario: String,
    requested_output_buffer_frames: u32,
    expected_alsa_buffer_frames: u32,
    expected_alsa_period_frames: u32,
    internal_block_frames: usize,
    sample_format: String,
    channels: u16,
    sample_rate: u32,
    warmup_seconds: u64,
    measure_seconds: u64,
    scheduler_qualified: bool,
    post_dsp_zero: bool,
    measurement_stop_acknowledged: bool,
    stream_stopped: bool,
    final_progress_write_succeeded: bool,
    pid: u32,
    systemd_invocation_id: Option<String>,
    artifact_sha256: String,
    callback: CallbackMetricsSnapshot,
    profile_start: BenchmarkProfileSnapshot,
    profile_end: BenchmarkProfileSnapshot,
    recovered_alsa_epipe_count: Option<u64>,
    recovered_alsa_epipe_observable: bool,
    terminal_error: Option<String>,
    executor_mode: String,
    worker_health: String,
    worker_thread_name_0: String,
    worker_thread_name_1: String,
    joined_workers: usize,
    retirement_error: Option<String>,
    worker_timing_mode: WorkerTimingMode,
    worker_timing: Option<BenchmarkWorkerTiming>,
}

impl<'de> Deserialize<'de> for BenchmarkResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = BenchmarkResultUnchecked::deserialize(deserializer)?;
        validate_result_evidence(&unchecked).map_err(D::Error::custom)?;
        let BenchmarkResultUnchecked {
            schema_version,
            kind,
            status,
            board_profile,
            scenario,
            requested_output_buffer_frames,
            expected_alsa_buffer_frames,
            expected_alsa_period_frames,
            internal_block_frames,
            sample_format,
            channels,
            sample_rate,
            warmup_seconds,
            measure_seconds,
            scheduler_qualified,
            post_dsp_zero,
            measurement_stop_acknowledged,
            stream_stopped,
            final_progress_write_succeeded,
            pid,
            systemd_invocation_id,
            artifact_sha256,
            callback,
            profile_start,
            profile_end,
            recovered_alsa_epipe_count,
            recovered_alsa_epipe_observable,
            terminal_error,
            executor_mode,
            worker_health,
            worker_thread_name_0,
            worker_thread_name_1,
            joined_workers,
            retirement_error,
            worker_timing_mode,
            worker_timing,
        } = unchecked;
        Ok(Self {
            schema_version,
            kind,
            status,
            board_profile,
            scenario,
            requested_output_buffer_frames,
            expected_alsa_buffer_frames,
            expected_alsa_period_frames,
            internal_block_frames,
            sample_format,
            channels,
            sample_rate,
            warmup_seconds,
            measure_seconds,
            scheduler_qualified,
            post_dsp_zero,
            measurement_stop_acknowledged,
            stream_stopped,
            final_progress_write_succeeded,
            pid,
            systemd_invocation_id,
            artifact_sha256,
            callback,
            profile_start,
            profile_end,
            recovered_alsa_epipe_count,
            recovered_alsa_epipe_observable,
            terminal_error,
            executor_mode,
            worker_health,
            worker_thread_name_0,
            worker_thread_name_1,
            joined_workers,
            retirement_error,
            worker_timing_mode,
            worker_timing,
        })
    }
}

fn validate_result_evidence(result: &BenchmarkResultUnchecked) -> Result<(), String> {
    match result.worker_timing_mode {
        WorkerTimingMode::Enabled => {
            if result.worker_timing.is_none() {
                return Err("enabled worker timing mode requires worker timing evidence".into());
            }
        }
        WorkerTimingMode::Disabled => {
            if result.worker_timing.is_some() {
                return Err(
                    "disabled worker timing mode must not contain worker timing evidence".into(),
                );
            }
            if result.executor_mode != super::super::stream::EXECUTOR_MODE {
                return Err("disabled worker timing mode requires persistent workers".into());
            }
            if result.worker_health != "healthy" {
                return Err("disabled worker timing mode requires healthy workers".into());
            }
            if result.joined_workers != 2 {
                return Err("disabled worker timing mode requires both workers to join".into());
            }
            if !result.scheduler_qualified
                || !result.measurement_stop_acknowledged
                || !result.stream_stopped
                || !result.final_progress_write_succeeded
                || result.retirement_error.is_some()
            {
                return Err(
                    "disabled worker timing mode requires complete lifecycle evidence".into(),
                );
            }
        }
    }
    Ok(())
}
