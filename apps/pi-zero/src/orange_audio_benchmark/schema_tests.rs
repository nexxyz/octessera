use super::*;
use crate::orange_audio_benchmark::cli::parse;
use crate::orange_audio_benchmark::stream;

fn config() -> BenchmarkConfig {
    parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_ramp_16".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "256".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap()
}

#[test]
fn schema4_artifacts_round_trip_and_schema1_is_rejected() {
    let config = config();
    let metrics = CallbackMetricsSnapshot::default();
    let progress = BenchmarkProgress::new(
        &config,
        "warmup",
        2,
        5,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
    assert_eq!(progress.requested_output_buffer_frames, 256);
    assert_eq!(progress.expected_alsa_period_frames, 64);
    assert_eq!(progress.internal_block_frames, 256);
    let encoded = serde_json::to_string(&progress).unwrap();
    assert_eq!(
        serde_json::from_str::<BenchmarkProgress>(&encoded).unwrap(),
        progress
    );
    let schema1 = encoded.replacen("\"schema_version\":4", "\"schema_version\":1", 1);
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
    let artifact = readiness(
        &config,
        "invocation",
        "F32",
        2,
        44_100,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
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
    let schema1 = encoded.replacen("\"schema_version\":4", "\"schema_version\":1", 1);
    assert!(serde_json::from_str::<BenchmarkReadiness>(&schema1).is_err());
}

#[test]
fn result_schema5_round_trips_without_removed_parallel_evidence() {
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
        warmup_seconds: 5,
        measure_seconds: 30,
        scheduler_qualified: false,
        post_dsp_zero: false,
        measurement_stop_acknowledged: false,
        stream_stopped: false,
        final_progress_write_succeeded: false,
        pid: 1,
        systemd_invocation_id: None,
        artifact_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        callback: CallbackMetricsSnapshot::default(),
        profile_start: BenchmarkProfileSnapshot::default(),
        profile_end: BenchmarkProfileSnapshot::default(),
        recovered_alsa_epipe_count: None,
        recovered_alsa_epipe_observable: false,
        terminal_error: Some("benchmark profile evidence is missing".into()),
        executor_mode: stream::EXECUTOR_MODE.into(),
        worker_health: "healthy".into(),
        worker_thread_name_0: stream::expected_worker_thread_names()[0].clone(),
        worker_thread_name_1: stream::expected_worker_thread_names()[1].clone(),
        joined_workers: 2,
        retirement_error: None,
    };
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["schema_version"], 5);
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
        result
    );
    let schema4 = encoded.replacen("\"schema_version\":5", "\"schema_version\":4", 1);
    assert!(serde_json::from_str::<BenchmarkResult>(&schema4).is_err());
}

#[test]
fn profile_snapshot_preserves_admission_drop_evidence() {
    let snapshot = SynthProfileSnapshot {
        cumulative_voice_admission_drops: 3,
        ..SynthProfileSnapshot::default()
    };

    let profile = BenchmarkProfileSnapshot::from(snapshot);

    assert_eq!(profile.cumulative_voice_admission_drops, 3);
}

#[test]
fn schema5_requires_numeric_admission_drop_evidence() {
    let config = config();
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
        warmup_seconds: 5,
        measure_seconds: 30,
        scheduler_qualified: false,
        post_dsp_zero: false,
        measurement_stop_acknowledged: false,
        stream_stopped: false,
        final_progress_write_succeeded: false,
        pid: 1,
        systemd_invocation_id: None,
        artifact_sha256: config.artifact_sha256,
        callback: CallbackMetricsSnapshot::default(),
        profile_start: BenchmarkProfileSnapshot::default(),
        profile_end: BenchmarkProfileSnapshot::default(),
        recovered_alsa_epipe_count: None,
        recovered_alsa_epipe_observable: false,
        terminal_error: None,
        executor_mode: stream::EXECUTOR_MODE.into(),
        worker_health: "healthy".into(),
        worker_thread_name_0: stream::expected_worker_thread_names()[0].clone(),
        worker_thread_name_1: stream::expected_worker_thread_names()[1].clone(),
        joined_workers: 2,
        retirement_error: None,
    };
    let encoded = serde_json::to_value(result).unwrap();
    let mut missing = encoded.clone();
    missing["profile_start"]
        .as_object_mut()
        .unwrap()
        .remove("cumulative_voice_admission_drops");
    assert!(serde_json::from_value::<BenchmarkResult>(missing).is_err());
    let mut malformed = encoded;
    malformed["profile_end"]["cumulative_voice_admission_drops"] = "one".into();
    assert!(serde_json::from_value::<BenchmarkResult>(malformed).is_err());
}
