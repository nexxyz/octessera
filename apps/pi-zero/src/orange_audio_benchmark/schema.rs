use super::cli::BenchmarkConfig;
use super::metrics::CallbackMetricsSnapshot;
use realtime_engine::synth::SynthProfileSnapshot;
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const BENCHMARK_SCHEMA_VERSION: u8 = 2;
const BENCHMARK_RESULT_SCHEMA_VERSION: u8 = 3;

fn deserialize_schema_v2<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version != BENCHMARK_SCHEMA_VERSION {
        return Err(D::Error::custom(format!(
            "benchmark schema version {version} is not supported"
        )));
    }
    Ok(version)
}

fn deserialize_schema_v3<'de, D>(deserializer: D) -> Result<u8, D::Error>
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkProgress {
    #[serde(deserialize_with = "deserialize_schema_v2")]
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
    pub workers_effective: Option<bool>,
    pub cpal_device_error_count: u64,
    pub cpal_stream_error_count: u64,
    pub terminal_error: bool,
    pub post_dsp_zero: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReadiness {
    #[serde(deserialize_with = "deserialize_schema_v2")]
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
    pub callback_frames_min: u32,
    pub callback_frames_max: u32,
    pub callback_frame_sample_count: u64,
    pub callback_frame_size_change_count: u64,
    pub invalid_callback_frame_count: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub workers_requested: usize,
    pub workers_effective: bool,
    pub scheduler_qualified: bool,
    pub post_dsp_zero: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkResult {
    #[serde(deserialize_with = "deserialize_schema_v3")]
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
    pub workers_requested: usize,
    pub workers_effective: bool,
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
    pub worker_delta: Option<BenchmarkWorkerDelta>,
    pub worker_policy_error: Option<String>,
    pub recovered_alsa_epipe_count: Option<u64>,
    pub recovered_alsa_epipe_observable: bool,
    pub terminal_error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BenchmarkProfileSnapshot {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_preview_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub cumulative_voice_steals: u64,
    pub synth_parallel_dispatches: u64,
    pub synth_parallel_light_skips: u64,
    pub synth_parallel_backoff_skips: u64,
    pub synth_parallel_timing_backoffs: u64,
    pub synth_parallel_failures: u64,
    pub synth_parallel_unhealthy: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BenchmarkWorkerDelta {
    pub synth_parallel_dispatches: u64,
    pub synth_parallel_light_skips: u64,
    pub synth_parallel_backoff_skips: u64,
    pub synth_parallel_timing_backoffs: u64,
    pub synth_parallel_failures: u64,
    pub synth_parallel_unhealthy: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReleaseGate {
    #[serde(deserialize_with = "deserialize_schema_v2")]
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
            cumulative_voice_steals: snapshot.cumulative_voice_steals,
            synth_parallel_dispatches: snapshot.synth_parallel_dispatches,
            synth_parallel_light_skips: snapshot.synth_parallel_light_skips,
            synth_parallel_backoff_skips: snapshot.synth_parallel_backoff_skips,
            synth_parallel_timing_backoffs: snapshot.synth_parallel_timing_backoffs,
            synth_parallel_failures: snapshot.synth_parallel_failures,
            synth_parallel_unhealthy: snapshot.synth_parallel_unhealthy,
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
        workers_effective: Option<bool>,
    ) -> Self {
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
            workers_effective,
            cpal_device_error_count: metrics.cpal_device_error_count,
            cpal_stream_error_count: metrics.cpal_stream_error_count,
            terminal_error: metrics.terminal_error,
            post_dsp_zero: metrics.lifetime_callback_count > 0
                && metrics.post_mute_nonzero_samples == 0,
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
    workers_effective: bool,
) -> BenchmarkReadiness {
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
        callback_frames_min: metrics.lifetime_callback_frames_min,
        callback_frames_max: metrics.lifetime_callback_frames_max,
        callback_frame_sample_count: metrics.lifetime_callback_frame_sample_count,
        callback_frame_size_change_count: metrics.lifetime_callback_frame_size_change_count,
        invalid_callback_frame_count: metrics.lifetime_invalid_callback_frame_count,
        sample_rate,
        channels,
        sample_format: sample_format.into(),
        workers_requested: config.workers,
        workers_effective,
        scheduler_qualified: true,
        post_dsp_zero: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orange_audio_benchmark::cli::parse;

    fn config() -> BenchmarkConfig {
        parse(vec![
            "--benchmark-orange-audio".into(),
            "--scenario".into(),
            "synth_ramp_16".into(),
            "--output-frames".into(),
            "256".into(),
            "--engine-block-frames".into(),
            "256".into(),
            "--workers".into(),
            "2".into(),
            "--release-gate".into(),
            "release.json".into(),
            "--artifact-sha256".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        ])
        .unwrap()
    }

    #[test]
    fn schema2_artifacts_round_trip_and_schema1_is_rejected() {
        let config = config();
        let metrics = CallbackMetricsSnapshot::default();
        let progress = BenchmarkProgress::new(&config, "warmup", 2, 5, &metrics, Some(true));
        assert_eq!(progress.requested_output_buffer_frames, 256);
        assert_eq!(progress.expected_alsa_period_frames, 64);
        assert_eq!(progress.internal_block_frames, 256);
        let encoded = serde_json::to_string(&progress).unwrap();
        assert_eq!(
            serde_json::from_str::<BenchmarkProgress>(&encoded).unwrap(),
            progress
        );
        let schema1 = encoded.replacen("\"schema_version\":2", "\"schema_version\":1", 1);
        assert!(serde_json::from_str::<BenchmarkProgress>(&schema1).is_err());
    }

    #[test]
    fn readiness_uses_lifetime_variable_batch_geometry() {
        let config = config();
        let metrics = CallbackMetricsSnapshot {
            lifetime_callback_frames_min: 64,
            lifetime_callback_frames_max: 256,
            lifetime_callback_frame_sample_count: 5,
            lifetime_callback_frame_size_change_count: 4,
            ..Default::default()
        };
        let artifact = readiness(&config, "invocation", "F32", 2, 44_100, &metrics, false);
        assert_eq!(artifact.schema_version, BENCHMARK_SCHEMA_VERSION);
        assert_eq!(artifact.requested_output_buffer_frames, 256);
        assert_eq!(artifact.expected_alsa_period_frames, 64);
        assert_eq!(artifact.internal_block_frames, 256);
        assert_eq!(artifact.callback_frames_min, 64);
        assert_eq!(artifact.callback_frames_max, 256);
        let encoded = serde_json::to_string(&artifact).unwrap();
        assert_eq!(
            serde_json::from_str::<BenchmarkReadiness>(&encoded).unwrap(),
            artifact
        );
        let schema1 = encoded.replacen("\"schema_version\":2", "\"schema_version\":1", 1);
        assert!(serde_json::from_str::<BenchmarkReadiness>(&schema1).is_err());
    }

    #[test]
    fn result_schema3_round_trips_and_serializes_incomplete_worker_evidence_as_null() {
        let result = BenchmarkResult {
            schema_version: BENCHMARK_RESULT_SCHEMA_VERSION,
            kind: "orange_audio_benchmark_result".into(),
            status: "fail".into(),
            board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
            scenario: "synth_ramp_16".into(),
            requested_output_buffer_frames: 256,
            expected_alsa_buffer_frames: 256,
            expected_alsa_period_frames: 64,
            internal_block_frames: 256,
            sample_format: "F32".into(),
            channels: 2,
            sample_rate: 44_100,
            workers_requested: 2,
            workers_effective: true,
            warmup_seconds: 5,
            measure_seconds: 30,
            scheduler_qualified: false,
            post_dsp_zero: false,
            measurement_stop_acknowledged: false,
            stream_stopped: false,
            final_progress_write_succeeded: false,
            pid: 1,
            systemd_invocation_id: None,
            artifact_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            callback: CallbackMetricsSnapshot::default(),
            profile_start: BenchmarkProfileSnapshot::default(),
            profile_end: BenchmarkProfileSnapshot::default(),
            worker_delta: None,
            worker_policy_error: None,
            recovered_alsa_epipe_count: None,
            recovered_alsa_epipe_observable: false,
            terminal_error: Some("worker profile evidence is missing".into()),
        };
        let encoded = serde_json::to_string(&result).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["schema_version"], 3);
        assert!(value["worker_delta"].is_null());
        assert!(value["worker_policy_error"].is_null());
        assert_eq!(
            serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
            result
        );
        let schema2 = encoded.replacen("\"schema_version\":3", "\"schema_version\":2", 1);
        assert!(serde_json::from_str::<BenchmarkResult>(&schema2).is_err());
    }
}
