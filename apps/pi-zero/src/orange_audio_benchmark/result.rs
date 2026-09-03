use super::super::cli::{BenchmarkExecutorMode, WorkerTimingMode};
use super::{
    deserialize_result_schema_v9, BenchmarkProfileSnapshot, BenchmarkWorkerTiming,
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
    pub callback_scheduling_policy: Option<String>,
    pub callback_scheduling_priority: Option<i32>,
    pub callback_scheduling_cpu: Option<u32>,
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
    #[serde(deserialize_with = "deserialize_result_schema_v9")]
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
    callback_scheduling_policy: Option<String>,
    callback_scheduling_priority: Option<i32>,
    callback_scheduling_cpu: Option<u32>,
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
            callback_scheduling_policy,
            callback_scheduling_priority,
            callback_scheduling_cpu,
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
            callback_scheduling_policy,
            callback_scheduling_priority,
            callback_scheduling_cpu,
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
    if !matches!(result.status.as_str(), "pass" | "fail") {
        return Err("benchmark result status is invalid".into());
    }
    let executor_mode = BenchmarkExecutorMode::parse(&result.executor_mode)
        .ok_or_else(|| "benchmark executor mode is missing or invalid".to_string())?;
    let expected_priority = match executor_mode {
        BenchmarkExecutorMode::Inline | BenchmarkExecutorMode::PersistentTwoWorkers => 70,
    };
    let expected_cpu = match executor_mode {
        BenchmarkExecutorMode::Inline => None,
        BenchmarkExecutorMode::PersistentTwoWorkers => Some(1),
    };
    let scheduling_is_valid = result.callback_scheduling_policy.as_deref() == Some("SCHED_FIFO")
        && result.callback_scheduling_priority == Some(expected_priority)
        && result.callback_scheduling_cpu == expected_cpu;
    if result.callback_scheduling_policy.is_some() != result.callback_scheduling_priority.is_some()
        || result.callback_scheduling_cpu.is_some()
            != (executor_mode == BenchmarkExecutorMode::PersistentTwoWorkers
                && result.callback_scheduling_policy.is_some())
        || (result.scheduler_qualified && !scheduling_is_valid)
        || (result.callback_scheduling_policy.is_some() && !scheduling_is_valid)
    {
        return Err("effective callback scheduling policy, priority, or CPU is invalid".into());
    }
    if result.status == "pass"
        && (!result.scheduler_qualified
            || !result.measurement_stop_acknowledged
            || !result.stream_stopped
            || !result.final_progress_write_succeeded
            || result.retirement_error.is_some()
            || result.terminal_error.is_some())
    {
        return Err("benchmark lifecycle evidence is incomplete".into());
    }
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
        }
    }
    match executor_mode {
        BenchmarkExecutorMode::Inline => {
            if result.worker_timing_mode != WorkerTimingMode::Disabled {
                return Err("inline executor requires disabled worker timing".into());
            }
            if !result.worker_thread_name_0.is_empty() || !result.worker_thread_name_1.is_empty() {
                return Err("inline executor must not report worker thread names".into());
            }
            if result.worker_health != "disabled" || result.joined_workers != 0 {
                return Err("inline executor has invalid worker lifecycle evidence".into());
            }
        }
        BenchmarkExecutorMode::PersistentTwoWorkers => {
            let pre_stream_failure = result.status == "fail"
                && result.terminal_error.is_some()
                && result.worker_health == "disabled"
                && result.worker_thread_name_0.is_empty()
                && result.worker_thread_name_1.is_empty()
                && result.joined_workers == 0;
            let persistent_health = matches!(
                result.worker_health.as_str(),
                "healthy"
                    | "deadline_miss"
                    | "dispatch_failed"
                    | "completion_failed"
                    | "worker_exited"
                    | "invalid_block"
            );
            if !pre_stream_failure
                && (result.worker_thread_name_0 != "oct-dsp-src-0"
                    || result.worker_thread_name_1 != "oct-dsp-src-1"
                    || !persistent_health
                    || result.joined_workers != 2)
            {
                return Err("persistent executor has invalid worker lifecycle evidence".into());
            }
            if result.status == "pass" && result.worker_health != "healthy" {
                return Err("a passing persistent executor must report healthy workers".into());
            }
            if result.status == "fail"
                && !pre_stream_failure
                && result.worker_health != "healthy"
                && result.terminal_error.is_none()
            {
                return Err("terminal persistent worker health lacks failure evidence".into());
            }
        }
    }
    Ok(())
}
