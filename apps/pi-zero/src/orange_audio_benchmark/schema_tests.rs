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

fn benchmark_result(worker_timing: BenchmarkWorkerTiming) -> BenchmarkResult {
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
        worker_timing,
    }
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
fn result_schema6_requires_worker_timing_and_rejects_unknown_fields() {
    let result = benchmark_result(worker_timing());
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["schema_version"], 6);
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
    let schema5 = encoded.replacen("\"schema_version\":6", "\"schema_version\":5", 1);
    assert!(serde_json::from_str::<BenchmarkResult>(&schema5).is_err());
    let missing_timing = value_without_worker_timing(&result);
    assert!(serde_json::from_value::<BenchmarkResult>(missing_timing).is_err());
    let mut null_timing = serde_json::to_value(&result).unwrap();
    null_timing["worker_timing"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BenchmarkResult>(null_timing).is_err());
}

fn value_without_worker_timing(result: &BenchmarkResult) -> serde_json::Value {
    let mut value = serde_json::to_value(result).unwrap();
    value.as_object_mut().unwrap().remove("worker_timing");
    value
}

type SemanticCase = (&'static str, fn(&mut serde_json::Value));

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
fn schema6_requires_numeric_admission_drop_evidence() {
    let config = config();
    let mut result = benchmark_result(worker_timing());
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
fn schema6_accepts_healthy_and_deadline_worker_timing() {
    for timing in [worker_timing(), deadline_worker_timing()] {
        let encoded = serde_json::to_string(&benchmark_result(timing)).unwrap();
        assert!(serde_json::from_str::<BenchmarkResult>(&encoded).is_ok());
    }
}

#[test]
fn schema6_rejects_impossible_worker_timing_relationships() {
    let cases: [SemanticCase; 37] = [
        ("missing deadline", |value| {
            value["worker_timing"]["coordinator"]["deadline_ns"] = serde_json::Value::Null;
        }),
        ("missing dispatch-to-deadline start", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_start_ns"] =
                serde_json::Value::Null;
        }),
        ("missing in-flight mask", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = serde_json::Value::Null;
        }),
        ("missing completed mask", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = serde_json::Value::Null;
        }),
        ("missing engine total", |value| {
            value["worker_timing"]["coordinator"]["engine_block_total_ns"] =
                serde_json::Value::Null;
        }),
        ("missing callback total", |value| {
            value["worker_timing"]["coordinator"]["callback_total_ns"] = serde_json::Value::Null;
        }),
        ("unknown mask bit", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 4.into();
        }),
        ("overlapping masks", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 1.into();
        }),
        ("mask union gap", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 0.into();
        }),
        ("engine total exceeds callback", |value| {
            value["worker_timing"]["coordinator"]["engine_block_total_ns"] = 51.into();
        }),
        ("unexecuted coordinator has measurements", |value| {
            value["worker_timing"]["coordinator"]["sequence"] = serde_json::Value::Null;
        }),
        ("unexecuted coordinator has finished worker", |value| {
            let coordinator = &mut value["worker_timing"]["coordinator"];
            for name in [
                "sequence",
                "deadline_ns",
                "dispatch_to_deadline_start_ns",
                "dispatch_to_deadline_elapsed_ns",
                "in_flight_mask",
                "completed_mask",
                "first_parity",
                "dispatch_to_first_ns",
                "dispatch_to_both_ns",
                "reduction_ns",
                "coordinator_remainder_ns",
                "engine_block_total_ns",
                "callback_total_ns",
            ] {
                coordinator[name] = serde_json::Value::Null;
            }
        }),
        ("zero completed has first evidence", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 0.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 3.into();
        }),
        ("completed has no first evidence", |value| {
            value["worker_timing"]["coordinator"]["first_parity"] = serde_json::Value::Null;
        }),
        ("first parity is not completed", |value| {
            value["worker_timing"]["coordinator"]["first_parity"] = 1.into();
        }),
        ("both completion is missing", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = serde_json::Value::Null;
        }),
        ("both completion is premature", |value| {
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
        }),
        ("first follows both", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 30.into();
        }),
        ("first precedes worker finish", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 19.into();
        }),
        ("both precedes worker finish", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = 24.into();
        }),
        ("completion is after deadline", |value| {
            value["worker_timing"]["workers"][0]["dispatch_to_finish_ns"] = 111.into();
            value["worker_timing"]["coordinator"]["dispatch_to_first_ns"] = 111.into();
        }),
        ("healthy masks are incomplete", |value| {
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 2.into();
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
        }),
        ("healthy reduction is missing", |value| {
            value["worker_timing"]["coordinator"]["reduction_ns"] = serde_json::Value::Null;
        }),
        ("healthy remainder is missing", |value| {
            value["worker_timing"]["coordinator"]["coordinator_remainder_ns"] =
                serde_json::Value::Null;
        }),
        ("healthy deadline elapsed is present", |value| {
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] = 110.into();
        }),
        ("failed deadline elapsed precedes deadline", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] = 109.into();
        }),
        (
            "failed deadline elapsed precedes dispatch boundary",
            |value| {
                value["worker_timing"]["coordinator"]["failed"] = true.into();
                value["worker_timing"]["coordinator"]["dispatch_to_deadline_start_ns"] = 50.into();
                value["worker_timing"]["coordinator"]["dispatch_to_deadline_elapsed_ns"] =
                    149.into();
            },
        ),
        ("failed incomplete timing has reduction", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["in_flight_mask"] = 2.into();
            value["worker_timing"]["coordinator"]["completed_mask"] = 1.into();
            value["worker_timing"]["coordinator"]["dispatch_to_both_ns"] = serde_json::Value::Null;
        }),
        ("remainder has no reduction", |value| {
            value["worker_timing"]["coordinator"]["failed"] = true.into();
            value["worker_timing"]["coordinator"]["reduction_ns"] = serde_json::Value::Null;
        }),
        ("finished worker sequence is missing", |value| {
            value["worker_timing"]["workers"][0]["sequence"] = serde_json::Value::Null;
        }),
        ("finished worker sequence disagrees", |value| {
            value["worker_timing"]["workers"][1]["sequence"] = 8.into();
        }),
        ("worker dispatch precedes render", |value| {
            value["worker_timing"]["workers"][0]["dispatch_to_finish_ns"] = 9.into();
        }),
        ("unfinished worker has evidence", |value| {
            value["worker_timing"]["workers"][0]["finished"] = false.into();
        }),
        ("worker CPU pair is partial", |value| {
            value["worker_timing"]["workers"][0]["cpu_start"] = serde_json::Value::Null;
        }),
        ("worker CPU availability disagrees", |value| {
            value["worker_timing"]["workers"][1]["cpu_start"] = serde_json::Value::Null;
            value["worker_timing"]["workers"][1]["cpu_end"] = serde_json::Value::Null;
        }),
        ("CPU endpoint-change summary disagrees", |value| {
            value["worker_timing"]["cpu_endpoint_changed"] = false.into();
        }),
        ("late summary disagrees", |value| {
            value["worker_timing"]["late_after_deadline_ns"] = 1.into();
        }),
    ];
    for (name, mutate) in cases {
        let mut value = serde_json::to_value(benchmark_result(worker_timing())).unwrap();
        mutate(&mut value);
        assert!(
            serde_json::from_value::<BenchmarkResult>(value).is_err(),
            "case should be rejected: {name}"
        );
    }
}
