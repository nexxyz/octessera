use super::*;
use crate::live_audio_benchmark::cli::{parse, BenchmarkExecutorMode, WorkerTimingMode};
use crate::live_audio_benchmark::stream;

fn config() -> BenchmarkConfig {
    let raspberry = super::super::geometry::is_raspberry_diagnostic();
    let mut config = parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_ramp_16".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        if raspberry { "64" } else { "256" }.into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    config.executor_mode = if raspberry {
        BenchmarkExecutorMode::RoutingTreePersistent
    } else {
        BenchmarkExecutorMode::PersistentTwoWorkers
    };
    config.worker_timing_mode = WorkerTimingMode::Enabled;
    config
}

fn worker_timing() -> BenchmarkWorkerTiming {
    BenchmarkWorkerTiming {
        workers: [
            BenchmarkWorkerTimingWorker {
                sequence: Some(7),
                render_ns: Some(10),
                dispatch_to_finish_ns: Some(20),
                cpu_start: Some(2),
                cpu_end: Some(2),
                finished: true,
            },
            BenchmarkWorkerTimingWorker {
                sequence: Some(7),
                render_ns: Some(11),
                dispatch_to_finish_ns: Some(25),
                cpu_start: Some(3),
                cpu_end: Some(3),
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
        cpu_endpoint_changed: false,
    }
}

fn routing_worker_timing() -> BenchmarkWorkerTiming {
    let mut timing = worker_timing();
    timing.workers[0].render_ns = Some(80_000);
    timing.workers[0].dispatch_to_finish_ns = Some(95_000);
    timing.workers[1].render_ns = Some(300_000);
    timing.workers[1].dispatch_to_finish_ns = Some(356_000);
    timing.coordinator.deadline_ns = Some(0);
    timing.coordinator.dispatch_to_deadline_start_ns = Some(1_289_000);
    timing.coordinator.dispatch_to_first_ns = Some(1_300_000);
    timing.coordinator.dispatch_to_both_ns = Some(1_317_000);
    timing
}

fn benchmark_result(
    worker_timing_mode: WorkerTimingMode,
    worker_timing: Option<BenchmarkWorkerTiming>,
) -> BenchmarkResult {
    let raspberry = super::super::geometry::is_raspberry_diagnostic();
    BenchmarkResult {
        schema_version: BENCHMARK_RESULT_SCHEMA_VERSION,
        kind: super::super::platform::BENCHMARK_RESULT_KIND.into(),
        status: "pass".into(),
        board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
        scenario: "synth_ramp_16".into(),
        requested_output_buffer_frames: 256,
        expected_alsa_buffer_frames: 256,
        expected_alsa_period_frames: 64,
        internal_block_frames: if raspberry { 64 } else { 256 },
        lookahead_frames: if raspberry { 64 } else { 256 },
        effective_output_latency_frames: if raspberry { 320 } else { 512 },
        sample_format: "F32".into(),
        channels: 2,
        sample_rate: 44_100,
        warmup_seconds: 5,
        measure_seconds: 30,
        scheduler_qualified: true,
        callback_scheduling_policy: Some("SCHED_FIFO".into()),
        callback_scheduling_priority: Some(70),
        callback_scheduling_cpu: Some(1),
        post_dsp_zero: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        pid: 1,
        systemd_invocation_id: None,
        artifact_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        callback: CallbackMetricsSnapshot::default(),
        persistent_output_counters: PersistentOutputCountersEvidence {
            observable: true,
            ..PersistentOutputCountersEvidence::default()
        },
        persistent_output_provenance: PersistentOutputProvenanceEvidence {
            observable: cfg!(feature = "routing-tree-benchmark"),
            ..PersistentOutputProvenanceEvidence::default()
        },
        detected_continuity_events: 0,
        profile_start: BenchmarkProfileSnapshot::default(),
        profile_end: BenchmarkProfileSnapshot::default(),
        recovered_alsa_epipe_count: None,
        recovered_alsa_epipe_observable: false,
        terminal_error: None,
        executor_mode: stream::EXECUTOR_MODE.into(),
        continue_on_recovered_miss: false,
        worker_health: "healthy".into(),
        worker_thread_name_0: stream::expected_routing_worker_thread_names()[0].clone(),
        worker_thread_name_1: stream::expected_routing_worker_thread_names()[1].clone(),
        joined_workers: 2,
        retirement_error: None,
        worker_timing_mode,
        worker_timing,
    }
}

fn inline_benchmark_result() -> BenchmarkResult {
    let mut result = benchmark_result(WorkerTimingMode::Disabled, None);
    result.executor_mode = "inline".into();
    if super::super::geometry::is_raspberry_diagnostic() {
        result.requested_output_buffer_frames = 128;
        result.expected_alsa_buffer_frames = 128;
        result.expected_alsa_period_frames = 32;
        result.internal_block_frames = 32;
        result.effective_output_latency_frames = 128;
    }
    result.lookahead_frames = 0;
    result.effective_output_latency_frames = if super::super::geometry::is_raspberry_diagnostic() {
        128
    } else {
        256
    };
    result.callback_scheduling_priority = Some(70);
    result.callback_scheduling_cpu = Some(1);
    result.persistent_output_counters = PersistentOutputCountersEvidence::default();
    result.worker_health = "disabled".into();
    result.worker_thread_name_0.clear();
    result.worker_thread_name_1.clear();
    result.joined_workers = 0;
    result.persistent_output_provenance = PersistentOutputProvenanceEvidence::default();
    result
}

#[test]
fn schema5_artifacts_round_trip_and_schema1_is_rejected() {
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
    assert_eq!(progress.lookahead_frames, 0);
    let encoded = serde_json::to_string(&progress).unwrap();
    assert_eq!(
        serde_json::from_str::<BenchmarkProgress>(&encoded).unwrap(),
        progress
    );
    let schema1 = encoded.replacen("\"schema_version\":5", "\"schema_version\":1", 1);
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
    assert_eq!(artifact.lookahead_frames, 0);
    assert_eq!(artifact.callback_frames_min, 64);
    assert_eq!(artifact.callback_frames_max, 256);
    let encoded = serde_json::to_string(&artifact).unwrap();
    assert_eq!(
        serde_json::from_str::<BenchmarkReadiness>(&encoded).unwrap(),
        artifact
    );
    let schema1 = encoded.replacen("\"schema_version\":5", "\"schema_version\":1", 1);
    assert!(serde_json::from_str::<BenchmarkReadiness>(&schema1).is_err());
}

#[path = "schema_executor_tests.rs"]
mod executor_tests;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[path = "schema_geometry_tests.rs"]
mod geometry_tests;
#[path = "schema_profile_tests.rs"]
mod profile_tests;
#[path = "schema_result_tests.rs"]
mod result_tests;
#[path = "schema_timing_tests.rs"]
mod timing_tests;
