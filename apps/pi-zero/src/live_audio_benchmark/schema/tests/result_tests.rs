use super::*;

#[test]
fn result_schema13_requires_worker_timing_and_rejects_unknown_fields() {
    let result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["schema_version"], 13);
    assert_eq!(value["lookahead_frames"], 128);
    assert_eq!(value["effective_output_latency_frames"], 384);
    assert_eq!(value["callback_scheduling_cpu"], 1);
    assert_eq!(value["worker_timing_mode"], "enabled");
    assert_eq!(value["continue_on_recovered_miss"], false);
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
    let unsupported_schema = encoded.replacen("\"schema_version\":13", "\"schema_version\":10", 1);
    assert!(serde_json::from_str::<BenchmarkResult>(&unsupported_schema).is_err());
    let missing_timing = value_without_worker_timing(&result);
    assert!(serde_json::from_value::<BenchmarkResult>(missing_timing).is_err());
    let mut null_timing = serde_json::to_value(&result).unwrap();
    null_timing["worker_timing"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BenchmarkResult>(null_timing).is_err());
}

#[cfg(feature = "routing-tree-benchmark")]
fn continuation_result() -> BenchmarkResult {
    let mut result = benchmark_result(WorkerTimingMode::Disabled, None);
    result.status = "fail".into();
    result.scenario = "capacity_analogue_16".into();
    result.requested_output_buffer_frames = 256;
    result.expected_alsa_buffer_frames = 256;
    result.expected_alsa_period_frames = 64;
    result.internal_block_frames =
        if crate::live_audio_benchmark::geometry::is_raspberry_diagnostic() {
            128
        } else {
            64
        };
    result.lookahead_frames = result.internal_block_frames;
    result.effective_output_latency_frames = 256 + result.lookahead_frames;
    result.measure_seconds = 120;
    result.executor_mode = "routing_tree_persistent".into();
    result.continue_on_recovered_miss = true;
    result.worker_health = "deadline_miss".into();
    result.terminal_error = None;
    result.worker_thread_name_0 = "oct-dsp-tree-0".into();
    result.worker_thread_name_1 = "oct-dsp-tree-1".into();
    result
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn schema13_continuation_identity_round_trips_and_requires_exact_terminal_contract() {
    let result = continuation_result();
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["continue_on_recovered_miss"], true);
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
        result
    );

    let mut invalid = serde_json::to_value(&result).unwrap();
    invalid["continue_on_recovered_miss"] = false.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&result).unwrap();
    invalid["worker_timing_mode"] = "enabled".into();
    invalid["worker_timing"] = serde_json::to_value(worker_timing()).unwrap();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&result).unwrap();
    invalid["measure_seconds"] = 30.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    for health in [
        "dispatch_failed",
        "completion_failed",
        "worker_exited",
        "invalid_block",
    ] {
        let mut invalid = serde_json::to_value(&result).unwrap();
        invalid["worker_health"] = health.into();
        assert!(serde_json::from_value::<BenchmarkResult>(invalid.clone()).is_err());

        invalid["terminal_error"] = "worker failed".into();
        assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_ok());
    }
}

#[test]
fn result_schema13_requires_observable_persistent_provenance_to_be_consistent() {
    let result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    let encoded = serde_json::to_string(&result).unwrap();
    let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["schema_version"], 13);
    assert_eq!(
        value["persistent_output_provenance"]["repeated_pcm_frames"],
        0
    );
    assert_eq!(
        serde_json::from_str::<BenchmarkResult>(&encoded).unwrap(),
        result
    );

    let mut invalid = value;
    invalid["persistent_output_provenance"]["observable"] = false.into();
    invalid["persistent_output_provenance"]["repeated_pcm_frames"] = 1.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());
}

#[test]
fn schema13_worker_timing_modes_require_exact_consistent_evidence() {
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
fn schema13_executor_modes_require_exact_runtime_evidence() {
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
    invalid["callback_scheduling_cpu"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid = serde_json::to_value(&inline).unwrap();
    invalid["callback_scheduling_cpu"] = 2.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid).is_err());

    let mut invalid =
        serde_json::to_value(benchmark_result(WorkerTimingMode::Disabled, None)).unwrap();
    invalid["callback_scheduling_cpu"] = 2.into();
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
fn schema13_accepts_pre_stream_failures_for_both_executors() {
    let executor_modes = if crate::live_audio_benchmark::geometry::is_raspberry_diagnostic() {
        vec![
            crate::live_audio_benchmark::cli::BenchmarkExecutorMode::Inline,
            crate::live_audio_benchmark::cli::BenchmarkExecutorMode::RoutingTreePersistent,
        ]
    } else {
        vec![
            crate::live_audio_benchmark::cli::BenchmarkExecutorMode::Inline,
            crate::live_audio_benchmark::cli::BenchmarkExecutorMode::PersistentTwoWorkers,
        ]
    };
    for executor_mode in executor_modes {
        let mut result = inline_benchmark_result();
        result.executor_mode = executor_mode.as_str().into();
        result.persistent_output_counters =
            PersistentOutputCountersEvidence::for_executor(executor_mode);
        result.persistent_output_provenance = PersistentOutputProvenanceEvidence {
            observable: executor_mode == BenchmarkExecutorMode::RoutingTreePersistent
                && cfg!(feature = "routing-tree-benchmark"),
            ..Default::default()
        };
        result.status = "fail".into();
        result.scheduler_qualified = false;
        result.callback_scheduling_policy = None;
        result.callback_scheduling_priority = None;
        result.callback_scheduling_cpu = None;
        result.measurement_stop_acknowledged = false;
        result.stream_stopped = false;
        result.final_progress_write_succeeded = false;
        result.terminal_error = Some("stream build failed".into());
        if executor_mode
            == crate::live_audio_benchmark::cli::BenchmarkExecutorMode::RoutingTreePersistent
        {
            result.lookahead_frames = result.internal_block_frames;
            result.effective_output_latency_frames =
                result.requested_output_buffer_frames as usize + result.lookahead_frames;
        }
        if executor_mode
            == crate::live_audio_benchmark::cli::BenchmarkExecutorMode::PersistentTwoWorkers
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

#[test]
fn schema13_accepts_healthy_and_deadline_worker_timing() {
    for timing in [worker_timing(), deadline_worker_timing()] {
        let encoded =
            serde_json::to_string(&benchmark_result(WorkerTimingMode::Enabled, Some(timing)))
                .unwrap();
        assert!(serde_json::from_str::<BenchmarkResult>(&encoded).is_ok());
    }
}

#[test]
fn schema13_accepts_routing_observations_after_deadline_boundary() {
    let mut result = benchmark_result(WorkerTimingMode::Enabled, Some(routing_worker_timing()));
    result.executor_mode = "routing_tree_persistent".into();
    result.lookahead_frames = result.internal_block_frames;
    result.effective_output_latency_frames =
        result.requested_output_buffer_frames as usize + result.lookahead_frames;
    let names = stream::expected_routing_worker_thread_names();
    result.worker_thread_name_0 = names[0].clone();
    result.worker_thread_name_1 = names[1].clone();
    assert!(
        serde_json::from_value::<BenchmarkResult>(serde_json::to_value(result).unwrap()).is_ok()
    );
}

#[test]
fn schema13_validates_persistent_output_counter_evidence_and_detection() {
    let mut result = benchmark_result(WorkerTimingMode::Disabled, None);
    result.persistent_output_counters = PersistentOutputCountersEvidence {
        observable: true,
        warmup: rodio_engine_source::PersistentOutputCounters {
            rendered_quantums: 1,
            ..Default::default()
        },
        start: rodio_engine_source::PersistentOutputCounters {
            rendered_quantums: 2,
            dropped_quantums: 1,
            deadline_misses: 1,
            ..Default::default()
        },
        end: rodio_engine_source::PersistentOutputCounters {
            rendered_quantums: 3,
            dropped_quantums: 2,
            deadline_misses: 1,
            ..Default::default()
        },
        delta: rodio_engine_source::PersistentOutputCounters {
            rendered_quantums: 1,
            dropped_quantums: 1,
            ..Default::default()
        },
    };
    result.detected_continuity_events = 1;
    let encoded = serde_json::to_value(&result).unwrap();
    assert!(serde_json::from_value::<BenchmarkResult>(encoded.clone()).is_ok());

    let mut invalid_delta = encoded.clone();
    invalid_delta["persistent_output_counters"]["delta"]["dropped_quantums"] = 0.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid_delta).is_err());

    let mut invalid_detected = encoded;
    invalid_detected["detected_continuity_events"] = 0.into();
    assert!(serde_json::from_value::<BenchmarkResult>(invalid_detected).is_err());

    let mut invalid_capacity = result;
    invalid_capacity.measure_seconds = 180;
    invalid_capacity.detected_continuity_events = 1;
    assert!(serde_json::from_value::<BenchmarkResult>(
        serde_json::to_value(invalid_capacity).unwrap()
    )
    .is_err());
}
