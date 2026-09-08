use super::*;

#[test]
fn schema5_progress_and_readiness_reject_mismatched_executor_geometry() {
    let config = config();
    let metrics = CallbackMetricsSnapshot::default();
    let progress = BenchmarkProgress::new(
        &config,
        "prepared",
        0,
        0,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
    let mut invalid_progress = serde_json::to_value(&progress).unwrap();
    invalid_progress["lookahead_frames"] = 128.into();
    assert!(serde_json::from_value::<BenchmarkProgress>(invalid_progress).is_err());

    let readiness = readiness(
        &config,
        "invocation",
        "F32",
        2,
        44_100,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
    let mut invalid_readiness = serde_json::to_value(&readiness).unwrap();
    invalid_readiness["worker_thread_name_0"] = "oct-dsp-tree-0".into();
    assert!(serde_json::from_value::<BenchmarkReadiness>(invalid_readiness).is_err());
    let mut unknown_readiness = serde_json::to_value(&readiness).unwrap();
    unknown_readiness["unexpected"] = true.into();
    assert!(serde_json::from_value::<BenchmarkReadiness>(unknown_readiness).is_err());
}

#[test]
fn progress_and_readiness_report_the_selected_executor() {
    let mut config = config();
    config.executor_mode = crate::live_audio_benchmark::cli::BenchmarkExecutorMode::Inline;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    if super::super::super::geometry::is_raspberry_diagnostic() {
        config.output_frames = 128;
        config.expected_alsa_period_frames = 32;
        config.internal_frames = 32;
    }
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

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn schema13_routing_executor_reports_routing_geometry_and_worker_names() {
    let mut config = config();
    config.executor_mode =
        crate::live_audio_benchmark::cli::BenchmarkExecutorMode::RoutingTreePersistent;
    config.internal_frames = if super::super::super::geometry::is_raspberry_diagnostic() {
        64
    } else {
        128
    };
    let metrics = CallbackMetricsSnapshot::default();
    let progress = BenchmarkProgress::new(
        &config,
        "ready",
        0,
        0,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
    assert_eq!(progress.lookahead_frames, config.internal_frames);
    assert_eq!(progress.worker_thread_name_0, "oct-dsp-tree-0");
    assert_eq!(
        serde_json::from_value::<BenchmarkProgress>(serde_json::to_value(&progress).unwrap())
            .unwrap(),
        progress
    );

    let readiness = readiness(
        &config,
        "invocation",
        "F32",
        2,
        44_100,
        &metrics,
        SourceWorkerHealth::Healthy,
    );
    assert_eq!(readiness.lookahead_frames, config.internal_frames);
    assert_eq!(readiness.worker_thread_name_1, "oct-dsp-tree-1");
    assert_eq!(
        serde_json::from_value::<BenchmarkReadiness>(serde_json::to_value(&readiness).unwrap())
            .unwrap(),
        readiness
    );

    let mut result = benchmark_result(WorkerTimingMode::Disabled, None);
    result.executor_mode = "routing_tree_persistent".into();
    result.internal_block_frames = config.internal_frames;
    result.lookahead_frames = config.internal_frames;
    result.effective_output_latency_frames =
        result.requested_output_buffer_frames as usize + config.internal_frames;
    result.worker_thread_name_0 = "oct-dsp-tree-0".into();
    result.worker_thread_name_1 = "oct-dsp-tree-1".into();
    assert!(
        serde_json::from_value::<BenchmarkResult>(serde_json::to_value(result).unwrap()).is_ok()
    );
}
