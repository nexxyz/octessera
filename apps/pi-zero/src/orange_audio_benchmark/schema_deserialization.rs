use super::super::cli::BenchmarkExecutorMode;
use super::{BenchmarkProgress, BenchmarkReadiness};
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkProgressUnchecked {
    schema_version: u8,
    kind: String,
    status: String,
    phase: String,
    elapsed_seconds: u64,
    target_seconds: u64,
    board_profile: String,
    pid: u32,
    systemd_invocation_id: Option<String>,
    artifact_sha256: String,
    scenario: String,
    requested_output_buffer_frames: u32,
    expected_alsa_buffer_frames: u32,
    expected_alsa_period_frames: u32,
    internal_block_frames: usize,
    lookahead_frames: usize,
    callback_frames_min: u32,
    callback_frames_max: u32,
    callback_frame_sample_count: u64,
    callback_frame_size_change_count: u64,
    invalid_callback_frame_count: u64,
    lifetime_callback_frames_min: u32,
    lifetime_callback_frames_max: u32,
    lifetime_callback_frame_sample_count: u64,
    lifetime_callback_frame_size_change_count: u64,
    lifetime_invalid_callback_frame_count: u64,
    lifetime_callback_count: u64,
    measured_callback_count: u64,
    cpal_device_error_count: u64,
    cpal_stream_error_count: u64,
    terminal_error: bool,
    post_dsp_zero: bool,
    executor_mode: String,
    worker_health: String,
    worker_thread_name_0: String,
    worker_thread_name_1: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkReadinessUnchecked {
    schema_version: u8,
    kind: String,
    status: String,
    board_profile: String,
    pid: u32,
    systemd_invocation_id: String,
    artifact_sha256: String,
    scenario: String,
    requested_output_buffer_frames: u32,
    expected_alsa_buffer_frames: u32,
    expected_alsa_period_frames: u32,
    internal_block_frames: usize,
    lookahead_frames: usize,
    callback_frames_min: u32,
    callback_frames_max: u32,
    callback_frame_sample_count: u64,
    callback_frame_size_change_count: u64,
    invalid_callback_frame_count: u64,
    sample_rate: u32,
    channels: u16,
    sample_format: String,
    scheduler_qualified: bool,
    post_dsp_zero: bool,
    executor_mode: String,
    worker_health: String,
    worker_thread_name_0: String,
    worker_thread_name_1: String,
}

impl<'de> Deserialize<'de> for BenchmarkProgress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = BenchmarkProgressUnchecked::deserialize(deserializer)?;
        validate_schema::<D::Error>(value.schema_version, "benchmark")?;
        let executor_mode = parse_executor_mode(&value.executor_mode)?;
        if value.kind != "orange_audio_benchmark_progress" {
            return Err(D::Error::custom("benchmark progress kind is invalid"));
        }
        validate_geometry(
            executor_mode,
            value.requested_output_buffer_frames,
            value.expected_alsa_buffer_frames,
            value.expected_alsa_period_frames,
            value.internal_block_frames,
            value.lookahead_frames,
            None,
        )?;
        validate_worker_names(
            executor_mode,
            &value.worker_thread_name_0,
            &value.worker_thread_name_1,
        )?;
        Ok(Self {
            schema_version: value.schema_version,
            kind: value.kind,
            status: value.status,
            phase: value.phase,
            elapsed_seconds: value.elapsed_seconds,
            target_seconds: value.target_seconds,
            board_profile: value.board_profile,
            pid: value.pid,
            systemd_invocation_id: value.systemd_invocation_id,
            artifact_sha256: value.artifact_sha256,
            scenario: value.scenario,
            requested_output_buffer_frames: value.requested_output_buffer_frames,
            expected_alsa_buffer_frames: value.expected_alsa_buffer_frames,
            expected_alsa_period_frames: value.expected_alsa_period_frames,
            internal_block_frames: value.internal_block_frames,
            lookahead_frames: value.lookahead_frames,
            callback_frames_min: value.callback_frames_min,
            callback_frames_max: value.callback_frames_max,
            callback_frame_sample_count: value.callback_frame_sample_count,
            callback_frame_size_change_count: value.callback_frame_size_change_count,
            invalid_callback_frame_count: value.invalid_callback_frame_count,
            lifetime_callback_frames_min: value.lifetime_callback_frames_min,
            lifetime_callback_frames_max: value.lifetime_callback_frames_max,
            lifetime_callback_frame_sample_count: value.lifetime_callback_frame_sample_count,
            lifetime_callback_frame_size_change_count: value
                .lifetime_callback_frame_size_change_count,
            lifetime_invalid_callback_frame_count: value.lifetime_invalid_callback_frame_count,
            lifetime_callback_count: value.lifetime_callback_count,
            measured_callback_count: value.measured_callback_count,
            cpal_device_error_count: value.cpal_device_error_count,
            cpal_stream_error_count: value.cpal_stream_error_count,
            terminal_error: value.terminal_error,
            post_dsp_zero: value.post_dsp_zero,
            executor_mode: value.executor_mode,
            worker_health: value.worker_health,
            worker_thread_name_0: value.worker_thread_name_0,
            worker_thread_name_1: value.worker_thread_name_1,
        })
    }
}

impl<'de> Deserialize<'de> for BenchmarkReadiness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = BenchmarkReadinessUnchecked::deserialize(deserializer)?;
        validate_schema::<D::Error>(value.schema_version, "benchmark")?;
        let executor_mode = parse_executor_mode(&value.executor_mode)?;
        if value.kind != "orange_audio_benchmark_readiness" || value.status != "ready" {
            return Err(D::Error::custom(
                "benchmark readiness kind or status is invalid",
            ));
        }
        validate_geometry(
            executor_mode,
            value.requested_output_buffer_frames,
            value.expected_alsa_buffer_frames,
            value.expected_alsa_period_frames,
            value.internal_block_frames,
            value.lookahead_frames,
            None,
        )?;
        validate_worker_names(
            executor_mode,
            &value.worker_thread_name_0,
            &value.worker_thread_name_1,
        )?;
        let expected_health = match executor_mode {
            BenchmarkExecutorMode::Inline => "disabled",
            BenchmarkExecutorMode::PersistentTwoWorkers
            | BenchmarkExecutorMode::RoutingTreePersistent => "healthy",
        };
        if value.worker_health != expected_health {
            return Err(D::Error::custom(
                "benchmark readiness worker health does not match executor",
            ));
        }
        Ok(Self {
            schema_version: value.schema_version,
            kind: value.kind,
            status: value.status,
            board_profile: value.board_profile,
            pid: value.pid,
            systemd_invocation_id: value.systemd_invocation_id,
            artifact_sha256: value.artifact_sha256,
            scenario: value.scenario,
            requested_output_buffer_frames: value.requested_output_buffer_frames,
            expected_alsa_buffer_frames: value.expected_alsa_buffer_frames,
            expected_alsa_period_frames: value.expected_alsa_period_frames,
            internal_block_frames: value.internal_block_frames,
            lookahead_frames: value.lookahead_frames,
            callback_frames_min: value.callback_frames_min,
            callback_frames_max: value.callback_frames_max,
            callback_frame_size_change_count: value.callback_frame_size_change_count,
            callback_frame_sample_count: value.callback_frame_sample_count,
            invalid_callback_frame_count: value.invalid_callback_frame_count,
            sample_rate: value.sample_rate,
            channels: value.channels,
            sample_format: value.sample_format,
            scheduler_qualified: value.scheduler_qualified,
            post_dsp_zero: value.post_dsp_zero,
            executor_mode: value.executor_mode,
            worker_health: value.worker_health,
            worker_thread_name_0: value.worker_thread_name_0,
            worker_thread_name_1: value.worker_thread_name_1,
        })
    }
}

fn parse_executor_mode<E: DeserializeError>(value: &str) -> Result<BenchmarkExecutorMode, E> {
    BenchmarkExecutorMode::parse(value).ok_or_else(|| {
        E::custom(format!(
            "benchmark executor mode is missing or invalid: {value}"
        ))
    })
}

fn validate_schema<E: DeserializeError>(version: u8, name: &str) -> Result<(), E> {
    if version != super::BENCHMARK_SCHEMA_VERSION {
        return Err(E::custom(format!(
            "{name} schema version {version} is not supported"
        )));
    }
    Ok(())
}

fn validate_geometry<E: DeserializeError>(
    executor_mode: BenchmarkExecutorMode,
    requested_output_buffer_frames: u32,
    expected_alsa_buffer_frames: u32,
    expected_alsa_period_frames: u32,
    internal_block_frames: usize,
    lookahead_frames: usize,
    effective_output_latency_frames: Option<usize>,
) -> Result<(), E> {
    super::super::cli::validate_recorded_geometry(
        executor_mode,
        requested_output_buffer_frames,
        expected_alsa_buffer_frames,
        expected_alsa_period_frames,
        internal_block_frames,
        lookahead_frames,
        effective_output_latency_frames,
    )
    .map_err(E::custom)
}

fn validate_worker_names<E: DeserializeError>(
    executor_mode: BenchmarkExecutorMode,
    first: &str,
    second: &str,
) -> Result<(), E> {
    let expected = super::super::stream::worker_thread_names_for_executor(executor_mode);
    if first != expected[0] || second != expected[1] {
        return Err(E::custom(
            "benchmark worker thread names do not match executor",
        ));
    }
    Ok(())
}
