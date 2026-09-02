use super::*;
use crate::orange_audio_benchmark::cli::{parse, WorkerTimingMode};
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

fn worker_timing() -> BenchmarkWorkerTiming {
    BenchmarkWorkerTiming {
        workers: [
            BenchmarkWorkerTimingWorker {
                sequence: Some(7),
                render_ns: Some(10),
                dispatch_to_finish_ns: Some(20),
                cpu_start: Some(2),
                cpu_end: Some(3),
                finished: true,
            },
            BenchmarkWorkerTimingWorker {
                sequence: Some(7),
                render_ns: Some(11),
                dispatch_to_finish_ns: Some(25),
                cpu_start: Some(2),
                cpu_end: Some(2),
                finished: true,
            },
        ],
        coordinator: BenchmarkCoordinatorTiming {
            sequence: Some(7),
            deadline_ns: Some(100),
            dispatch_to_deadline_start_ns: Some(10),
            dispatch_to_deadline_elapsed_ns: None,
            in_flight_mask: Some(0),
            completed_mask: Some(3),
            first_parity: Some(0),
            dispatch_to_first_ns: Some(20),
            dispatch_to_both_ns: Some(25),
            reduction_ns: Some(4),
            coordinator_remainder_ns: Some(5),
            engine_block_total_ns: Some(40),
            callback_total_ns: Some(50),
            failed: false,
            frozen: true,
        },
        late_after_deadline_ns: None,
        cpu_endpoint_changed: true,
    }
}

fn deadline_worker_timing() -> BenchmarkWorkerTiming {
    let mut timing = worker_timing();
    timing.workers[1].dispatch_to_finish_ns = Some(125);
    timing.coordinator.dispatch_to_deadline_elapsed_ns = Some(110);
    timing.coordinator.in_flight_mask = Some(2);
    timing.coordinator.completed_mask = Some(1);
    timing.coordinator.dispatch_to_both_ns = None;
    timing.coordinator.reduction_ns = None;
    timing.coordinator.coordinator_remainder_ns = None;
    timing.coordinator.failed = true;
    timing.late_after_deadline_ns = Some(15);
    timing
}

fn benchmark_result(
    worker_timing_mode: WorkerTimingMode,
    worker_timing: Option<BenchmarkWorkerTiming>,
) -> BenchmarkResult {
    BenchmarkResult {
        schema_version: BENCHMARK_RESULT_SCHEMA_VERSION,
        kind: "orange_audio_benchmark_result".into(),
        status: "pass".into(),
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
        scheduler_qualified: true,
        callback_scheduling_policy: Some("SCHED_FIFO".into()),
        callback_scheduling_priority: Some(69),
        post_dsp_zero: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        pid: 1,
        systemd_invocation_id: None,
        artifact_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
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
        worker_timing_mode,
        worker_timing,
    }
}

fn inline_benchmark_result() -> BenchmarkResult {
    let mut result = benchmark_result(WorkerTimingMode::Disabled, None);
    result.executor_mode = "inline".into();
    result.callback_scheduling_priority = Some(70);
    result.worker_health = "disabled".into();
    result.worker_thread_name_0.clear();
    result.worker_thread_name_1.clear();
    result.joined_workers = 0;
    result
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
fn result_schema8_requires_worker_timing_and_rejects_unknown_fields() {
    let result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["schema_version"], 8);
    assert_eq!(value["worker_timing_mode"], "enabled");
    assert_eq!(value["worker_timing"]["workers"][1]["render_ns"], 11);
    assert_eq!(value["worker_timing"]["coordinator"]["reduction_ns"], 4);
    let mut unknown = serde_json::to_value(&result).unwrap();
    unknown["worker_timing"]["unknown"] = true.into();
    assert!(serde_json::from_value::<BenchmarkResult>(unknown).is_err());
    let mut unknown_worker = serde_json::to_value(&result).unwrap();
    unknown_worker["worker_timing"]["workers"][0]["unknown"] = true.into();
    assert!(serde_json::from_value::<BenchmarkResult>(unknown_worker).is_err());
    let mut unknown_coordinator = serde_json::to_value(&result).unwrap();
    unknown_coordinator["worker_timing"]["coordinator"]["unknown"] = true.into();
    assert!(serde_json::from_value::<BenchmarkResult>(unknown_coordinator).is_err());
    let mut unknown_result = serde_json::to_value(&result).unwrap();
    unknown_result["unknown"] = true.into();
    assert!(serde_json::from_value::<BenchmarkResult>(unknown_result).is_err());
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
        result
    );
    let schema7 = encoded.replacen("\"schema_version\":8", "\"schema_version\":7", 1);
    assert!(serde_json::from_str::<BenchmarkResult>(&schema7).is_err());
    let missing_timing = value_without_worker_timing(&result);
    assert!(serde_json::from_value::<BenchmarkResult>(missing_timing).is_err());
    let mut null_timing = serde_json::to_value(&result).unwrap();
    null_timing["worker_timing"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BenchmarkResult>(null_timing).is_err());
}

#[test]
fn schema8_worker_timing_modes_require_exact_consistent_evidence() {
    let enabled = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    let enabled_encoded = serde_json::to_string(&enabled).unwrap();
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&enabled_encoded).unwrap(),
        enabled
    );

    let disabled = benchmark_result(WorkerTimingMode::Disabled, None);
    let disabled_encoded = serde_json::to_string(&disabled).unwrap();
    let disabled_value: serde_json::Value = serde_json::from_str(&disabled_encoded).unwrap();
    assert_eq!(disabled_value["worker_timing_mode"], "disabled");
    assert!(disabled_value["worker_timing"].is_null());
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&disabled_encoded).unwrap(),
        disabled
    );

    let invalid_cases: [fn(&mut serde_json::Value); 8] = [
        |value| value["worker_timing_mode"] = "invalid".into(),
        |value| value["worker_timing_mode"] = 1.into(),
        |value| {
            value.as_object_mut().unwrap().remove("worker_timing_mode");
        },
        |value| {
            value["worker_timing_mode"] = "enabled".into();
            value["worker_timing"] = serde_json::Value::Null;
        },
        |value| {
            value["worker_timing_mode"] = "disabled".into();
            value["worker_health"] = "disabled".into();
        },
        |value| {
            value["worker_timing_mode"] = "disabled".into();
            value["worker_timing"] = serde_json::to_value(worker_timing()).unwrap();
        },
        |value| {
            value["worker_timing_mode"] = "disabled".into();
            value["executor_mode"] = "inline".into();
        },
        |value| {
            value["worker_timing_mode"] = "disabled".into();
            value["joined_workers"] = 1.into();
        },
    ];
    for mutate in invalid_cases {
        let mut value = serde_json::to_value(&disabled).unwrap();
        mutate(&mut value);
        assert!(
            serde_json::from_value::<BenchmarkResult>(value).is_err(),
            "inconsistent worker timing evidence should be rejected"
        );
    }
}

#[test]
fn schema8_executor_modes_require_exact_runtime_evidence() {
    let inline = inline_benchmark_result();
    let encoded = serde_json::to_string(&inline).unwrap();
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
        inline
    );

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["worker_timing_mode"] = "enabled".into();
    invalid["worker_timing"] = serde_json::to_value(worker_timing()).unwrap();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["worker_thread_name_0"] = "oct-dsp-src-0".into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["callback_scheduling_priority"] = 69.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["callback_scheduling_policy"] = "SCHED_RR".into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["executor_mode"] = "unknown".into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let persistent = benchmark_result(WorkerTimingMode::Disabled, None);
    let mut invalid = serde_json::to_value(&persistent).unwrap();
    invalid["callback_scheduling_policy"] = 1.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());
}

#[test]
fn schema8_accepts_pre_stream_failures_for_both_executors() {
    for executor_mode in [
        crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::Inline,
        crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::PersistentTwoWorkers,
    ] {
        let mut result = inline_benchmark_result();
        result.executor_mode = executor_mode.as_str().into();
        result.status = "fail".into();
        result.scheduler_qualified = false;
        result.callback_scheduling_policy = None;
        result.callback_scheduling_priority = None;
        result.measurement_stop_acknowledged = false;
        result.stream_stopped = false;
        result.final_progress_write_succeeded = false;
        result.terminal_error = Some("stream build failed".into());
        if executor_mode
            == crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::PersistentTwoWorkers
        {
            result.worker_health = "disabled".into();
            result.worker_thread_name_0.clear();
            result.worker_thread_name_1.clear();
        }
        assert!(
            serde_json::from_value::<BenchmarkResult>(serde_json::to_value(result).unwrap())
                .is_ok()
        );
    }
}

fn value_without_worker_timing(result: &BenchmarkResult) -> serde_json::Value {
    let mut value = serde_json::to_value(result).unwrap();
    value.as_object_mut().unwrap().remove("worker_timing");
    value
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
fn schema8_requires_numeric_admission_drop_evidence() {
    let config = config();
    let mut result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    result.artifact_sha256 = config.artifact_sha256;
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

#[test]
fn schema8_accepts_healthy_and_deadline_worker_timing() {
    for timing in [worker_timing(), deadline_worker_timing()] {
        let encoded =
            serde_json::to_string(&benchmark_result(WorkerTimingMode::Enabled, Some(timing)))
                .unwrap();
        assert!(serde_json::from_str::<BenchmarkResult>(&encoded).is_ok());
    }
}

#[test]
fn progress_and_readiness_report_the_selected_executor() {
    let mut config = config();
    config.executor_mode = crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::Inline;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    let metrics = CallbackMetricsSnapshot::default();
    let progress = BenchmarkProgress::new(
        &config,
        "prepared",
        0,
        0,
        &metrics,
        SourceWorkerHealth::Disabled,
    );
    assert_eq!(progress.executor_mode, "inline");
    assert_eq!(progress.worker_health, "disabled");
    assert_eq!(progress.worker_thread_name_0, "");
    assert_eq!(progress.worker_thread_name_1, "");
    let readiness = readiness(
        &config,
        "invocation",
        "F32",
        2,
        44_100,
        &metrics,
        SourceWorkerHealth::Disabled,
    );
    assert_eq!(readiness.executor_mode, "inline");
    assert_eq!(readiness.worker_health, "disabled");
    assert_eq!(readiness.worker_thread_name_0, "");
    assert_eq!(readiness.worker_thread_name_1, "");
}

#[path = "schema_timing_tests.rs"]
mod timing_tests;
