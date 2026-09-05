use super::*;
use crate::orange_audio_benchmark::cli::{parse, BenchmarkExecutorMode, WorkerTimingMode};
use rodio_engine_source::PersistentOutputCounters;
use std::time::Duration;

fn config() -> BenchmarkConfig {
    parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "1024".into(),
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
fn profile_validation_proves_max_fx_state() {
    let expected = crate::dsp_scenarios::expected_live_state(
        "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
    )
    .unwrap();
    let snapshot = SynthProfileSnapshot {
        active_synth_voices: 8,
        active_sample_voices: 8,
        active_momentary_fx: 2,
        active_bus_fx_slots: 12,
        active_global_fx_slots: 2,
        ..SynthProfileSnapshot::default()
    };
    validate_profile_state(
        &snapshot,
        expected,
        expected.expected_voice_admission_drops_start,
    )
    .unwrap();
    let mut invalid = snapshot;
    invalid.active_bus_fx_slots = 11;
    assert!(validate_profile_state(
        &invalid,
        expected,
        expected.expected_voice_admission_drops_start
    )
    .is_err());

    let mut preview = snapshot;
    preview.active_preview_sample_voices = 1;
    assert!(validate_profile_state(
        &preview,
        expected,
        expected.expected_voice_admission_drops_start
    )
    .is_err());

    let mut admission_mismatch = snapshot;
    admission_mismatch.cumulative_voice_admission_drops = 1;
    let error = validate_profile_state(
        &admission_mismatch,
        expected,
        expected.expected_voice_admission_drops_start,
    )
    .unwrap_err();
    assert!(error.contains("voice admission drops"));
    assert!(!error.contains("voice steals"));

    let mut declared = expected;
    declared.expected_voice_admission_drops_start = 1;
    let mut admitted = snapshot;
    admitted.cumulative_voice_admission_drops = 1;
    validate_profile_state(
        &admitted,
        declared,
        declared.expected_voice_admission_drops_start,
    )
    .unwrap();
}

#[test]
fn candidate_spacing_uses_the_alsa_period_not_the_engine_block() {
    let config = crate::orange_audio_benchmark::cli::parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "256".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    let state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            active_bus_fx_slots: 0,
            active_global_fx_slots: 0,
            expected_voice_steals: 0,
            expected_voice_admission_drops_start: 0,
            expected_voice_admission_drops_end: 0,
        },
        44_100,
        config.expected_alsa_period_frames,
        config.output_frames,
        config.executor_mode,
        WorkerTimingMode::Enabled,
    );
    state.metrics.enable_measurement();
    state
        .metrics
        .record_callback(256, Duration::from_nanos(1), 0, 0.0, 0, Some(2_000_000));
    assert_eq!(
        state.metrics.snapshot().callback_lateness_max_ns,
        2_000_000 - (64_u64 * 1_000_000_000 / 44_100)
    );
}

#[test]
fn run_state_captures_mirrored_cumulative_output_counters_at_each_boundary() {
    let config = config();
    let mut state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            active_bus_fx_slots: 0,
            active_global_fx_slots: 0,
            expected_voice_steals: 0,
            expected_voice_admission_drops_start: 0,
            expected_voice_admission_drops_end: 0,
        },
        44_100,
        config.expected_alsa_period_frames,
        config.output_frames,
        BenchmarkExecutorMode::PersistentTwoWorkers,
        WorkerTimingMode::Disabled,
    );
    let warmup_generation = state.phase_control.request(MeasurementPhase::Disabled);
    let warmup_capture = state.phase_control.capture_at_callback_entry();
    assert!(state
        .phase_control
        .acknowledgement(warmup_generation, MeasurementPhase::Disabled)
        .is_none());
    state.metrics.publish_phase_boundary(
        warmup_generation,
        PersistentOutputCounters {
            rendered_quantums: 2,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(warmup_capture);
    state.persistent_output_counters.warmup =
        state.phase_boundary_counters(warmup_generation).unwrap();
    let start_generation = state.phase_control.request(MeasurementPhase::Measuring);
    let start_capture = state.phase_control.capture_at_callback_entry();
    state.metrics.publish_phase_boundary(
        start_generation,
        PersistentOutputCounters {
            rendered_quantums: 3,
            dropped_quantums: 1,
            deadline_misses: 1,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(start_capture);
    state.persistent_output_counters.start =
        state.phase_boundary_counters(start_generation).unwrap();
    let end_generation = state.phase_control.request(MeasurementPhase::Disabled);
    let end_capture = state.phase_control.capture_at_callback_entry();
    state.metrics.publish_phase_boundary(
        end_generation,
        PersistentOutputCounters {
            rendered_quantums: 4,
            dropped_quantums: 2,
            deadline_misses: 1,
            ..Default::default()
        },
    );
    state.phase_control.acknowledge(end_capture);
    state.persistent_output_counters.end = state.phase_boundary_counters(end_generation).unwrap();
    state.persistent_output_counters.calculate_delta().unwrap();

    assert_eq!(state.persistent_output_counters.warmup.rendered_quantums, 2);
    assert_eq!(state.persistent_output_counters.start.dropped_quantums, 1);
    assert_eq!(state.persistent_output_counters.delta.rendered_quantums, 1);
    assert_eq!(state.persistent_output_counters.delta.dropped_quantums, 1);
}

#[test]
fn result_status_requires_clean_runtime_evidence() {
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let gates = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        worker_health: realtime_engine::synth::SourceWorkerHealth::Healthy,
        worker_thread_names: super::super::stream::expected_worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
    };
    for (measure_seconds, allowed, rejected) in [(30, 0, 1), (120, 0, 1), (180, 0, 1), (300, 5, 6)]
    {
        let mut config = config();
        config.measure_seconds = measure_seconds;
        assert_eq!(result_status(&config, &metrics, 0, gates.clone()), "pass");

        let allowed_metrics = CallbackMetricsSnapshot {
            over_audio_duration_budget_count: allowed,
            ..metrics
        };
        assert_eq!(
            result_status(&config, &allowed_metrics, 0, gates.clone()),
            "pass"
        );

        let rejected_metrics = CallbackMetricsSnapshot {
            over_audio_duration_budget_count: rejected,
            ..metrics
        };
        assert_eq!(
            result_status(&config, &rejected_metrics, 0, gates.clone()),
            "fail"
        );
    }
}

#[test]
fn one_eighty_second_result_requires_zero_detected_continuity_events() {
    let mut config = config();
    config.measure_seconds = 180;
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let gates = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        worker_health: SourceWorkerHealth::Healthy,
        worker_thread_names: super::super::stream::expected_worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
    };
    assert_eq!(result_status(&config, &metrics, 0, gates.clone()), "pass");
    assert_eq!(result_status(&config, &metrics, 1, gates), "fail");
}

#[test]
fn inline_result_status_requires_inline_worker_lifecycle_and_timing() {
    let mut config = config();
    config.executor_mode = BenchmarkExecutorMode::Inline;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let clean = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        worker_health: SourceWorkerHealth::Disabled,
        worker_thread_names: [String::new(), String::new()],
        joined_workers: 0,
        retirement_error: true,
        worker_timing_consistent: true,
    };
    assert_eq!(result_status(&config, &metrics, 0, clean.clone()), "pass");

    let mut invalid = clean.clone();
    invalid.worker_health = SourceWorkerHealth::Healthy;
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");
    invalid = clean.clone();
    invalid.worker_health = SourceWorkerHealth::WorkerExited;
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");
    invalid = clean.clone();
    invalid.joined_workers = 1;
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");
    invalid = clean.clone();
    invalid.worker_thread_names[0] = "oct-dsp-src-0".into();
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");

    config.worker_timing_mode = WorkerTimingMode::Enabled;
    assert_eq!(result_status(&config, &metrics, 0, clean), "fail");
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_result_status_requires_routing_worker_lifecycle() {
    let mut config = config();
    config.executor_mode = BenchmarkExecutorMode::RoutingTreePersistent;
    config.output_frames = 256;
    config.expected_alsa_period_frames = 64;
    config.internal_frames = 128;
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        ..CallbackMetricsSnapshot::default()
    };
    let clean = FinalizationGates {
        no_terminal_errors: true,
        scheduler_qualified: true,
        measurement_stop_acknowledged: true,
        stream_stopped: true,
        final_progress_write_succeeded: true,
        worker_health: SourceWorkerHealth::Healthy,
        worker_thread_names: super::super::stream::expected_routing_worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
    };
    assert_eq!(result_status(&config, &metrics, 0, clean.clone()), "pass");
    let mut invalid = clean;
    invalid.worker_thread_names = super::super::stream::expected_worker_thread_names();
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");
}

#[test]
fn injected_deadline_or_panic_worker_health_fails_benchmark_finalization() {
    let config = config();
    let metrics = CallbackMetricsSnapshot {
        callback_count: 1,
        callback_frames_min: 1,
        callback_frames_max: 1,
        callback_frame_sample_count: 1,
        pre_mute_nonzero_samples: 1,
        worker_terminal: true,
        terminal_error: true,
        ..CallbackMetricsSnapshot::default()
    };
    for worker_health in [
        realtime_engine::synth::SourceWorkerHealth::DeadlineMiss,
        realtime_engine::synth::SourceWorkerHealth::WorkerExited,
    ] {
        let gates = FinalizationGates {
            no_terminal_errors: true,
            scheduler_qualified: true,
            measurement_stop_acknowledged: true,
            stream_stopped: true,
            final_progress_write_succeeded: true,
            worker_health,
            worker_thread_names: super::super::stream::expected_worker_thread_names(),
            joined_workers: 2,
            retirement_error: true,
            worker_timing_consistent: true,
        };
        assert_eq!(result_status(&config, &metrics, 0, gates), "fail");
    }
}

#[test]
fn pre_stream_failure_serializes_worker_timing_for_both_modes() {
    for (executor_mode, worker_timing_mode) in [
        (BenchmarkExecutorMode::Inline, WorkerTimingMode::Disabled),
        (
            BenchmarkExecutorMode::PersistentTwoWorkers,
            WorkerTimingMode::Enabled,
        ),
        (
            BenchmarkExecutorMode::PersistentTwoWorkers,
            WorkerTimingMode::Disabled,
        ),
    ] {
        let mut config = config();
        config.executor_mode = executor_mode;
        config.worker_timing_mode = worker_timing_mode;
        let root = std::env::temp_dir().join(format!(
            "octessera-pre-stream-{}-{}-{}",
            std::process::id(),
            worker_timing_mode.as_str(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        config.result_path = root.join("result.json");
        config.progress_path = root.join("progress.json");
        config.readiness_path = root.join("readiness.json");
        config.release_gate_path = root.join("release.json");

        let previous_invocation = std::env::var_os("INVOCATION_ID");
        std::env::remove_var("INVOCATION_ID");
        let outcome = crate::orange_audio_benchmark::run_inner(&config);
        match previous_invocation {
            Some(value) => std::env::set_var("INVOCATION_ID", value),
            None => std::env::remove_var("INVOCATION_ID"),
        }

        assert!(outcome.is_err());
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config.result_path).unwrap()).unwrap();
        assert_eq!(value["executor_mode"], executor_mode.as_str());
        assert_eq!(value["worker_timing_mode"], worker_timing_mode.as_str());
        if worker_timing_mode == WorkerTimingMode::Enabled {
            let timing =
                serde_json::from_value::<BenchmarkWorkerTiming>(value["worker_timing"].clone())
                    .unwrap();
            assert!(timing.coordinator.frozen);
            assert_eq!(timing.coordinator.sequence, None);
            assert_eq!(timing.coordinator.deadline_ns, None);
            assert_eq!(timing.coordinator.dispatch_to_deadline_start_ns, None);
            assert_eq!(timing.coordinator.dispatch_to_deadline_elapsed_ns, None);
            assert!(timing.workers.iter().all(|worker| {
                !worker.finished
                    && worker.sequence.is_none()
                    && worker.render_ns.is_none()
                    && worker.dispatch_to_finish_ns.is_none()
                    && worker.cpu_start.is_none()
                    && worker.cpu_end.is_none()
            }));
            assert!(!timing.cpu_endpoint_changed);
        } else {
            assert!(value["worker_timing"].is_null());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
