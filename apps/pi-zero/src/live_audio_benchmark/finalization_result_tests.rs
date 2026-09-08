use super::*;

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
        worker_thread_names: worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
        persistent_output_counters_clean: true,
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
            if crate::live_audio_benchmark::geometry::is_raspberry_diagnostic() {
                if allowed == 0 {
                    "pass"
                } else {
                    "fail"
                }
            } else {
                "pass"
            }
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
        worker_thread_names: worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
        persistent_output_counters_clean: true,
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
        persistent_output_counters_clean: true,
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
    config.internal_frames = if crate::live_audio_benchmark::geometry::is_raspberry_diagnostic() {
        64
    } else {
        128
    };
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
        worker_thread_names:
            crate::live_audio_benchmark::stream::expected_routing_worker_thread_names(),
        joined_workers: 2,
        retirement_error: true,
        worker_timing_consistent: true,
        persistent_output_counters_clean: true,
    };
    assert_eq!(result_status(&config, &metrics, 0, clean.clone()), "pass");
    let mut invalid = clean;
    invalid.worker_thread_names = worker_thread_names();
    assert_eq!(result_status(&config, &metrics, 0, invalid), "fail");
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn continuation_finalization_fixture_uses_disabled_worker_timing() {
    let config = super::continuation_config();
    assert!(config.continue_on_recovered_miss);
    assert_eq!(config.worker_timing_mode, WorkerTimingMode::Disabled);
    assert!(
        crate::live_audio_benchmark::cli::is_approved_continue_on_recovered_miss(
            config.scenario.as_str(),
            config.executor_mode,
            config.output_frames,
            config.expected_alsa_period_frames,
            config.internal_frames,
            config.measure_seconds,
            config.worker_timing_mode,
        )
    );
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
            worker_thread_names: worker_thread_names(),
            joined_workers: 2,
            retirement_error: true,
            worker_timing_consistent: true,
            persistent_output_counters_clean: true,
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
        let outcome = crate::live_audio_benchmark::run_inner(&config);
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
