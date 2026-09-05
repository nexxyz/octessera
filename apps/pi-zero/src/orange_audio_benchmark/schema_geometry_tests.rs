use super::*;
use crate::orange_audio_benchmark::cli::{
    validate_recorded_geometry, BenchmarkExecutorMode, RecordedGeometry,
};

fn inline_analogue_config() -> BenchmarkConfig {
    let mut config = config();
    config.scenario = "capacity_analogue_1".into();
    config.output_frames = 128;
    config.expected_alsa_period_frames = 32;
    config.internal_frames = 64;
    config.executor_mode = BenchmarkExecutorMode::Inline;
    config.worker_timing_mode = WorkerTimingMode::Disabled;
    config
}

#[test]
fn schema_accepts_inline_analogue_geometry_and_rejects_tampered_contracts() {
    let config = inline_analogue_config();
    let metrics = CallbackMetricsSnapshot::default();
    let progress = BenchmarkProgress::new(
        &config,
        "warmup",
        0,
        5,
        &metrics,
        SourceWorkerHealth::Disabled,
    );
    assert_eq!(progress.expected_alsa_period_frames, 32);
    assert_eq!(progress.lookahead_frames, 0);
    assert_eq!(
        serde_json::from_value::<BenchmarkProgress>(serde_json::to_value(&progress).unwrap())
            .unwrap(),
        progress
    );
    let mut invalid_progress = serde_json::to_value(&progress).unwrap();
    invalid_progress["scenario"] = "synth_ramp_16".into();
    assert!(serde_json::from_value::<BenchmarkProgress>(invalid_progress).is_err());

    let readiness = readiness(
        &config,
        "invocation",
        "F32",
        2,
        44_100,
        &metrics,
        SourceWorkerHealth::Disabled,
    );
    assert_eq!(readiness.expected_alsa_period_frames, 32);
    assert_eq!(readiness.lookahead_frames, 0);
    assert_eq!(
        serde_json::from_value::<BenchmarkReadiness>(serde_json::to_value(&readiness).unwrap())
            .unwrap(),
        readiness
    );
    let mut invalid_readiness = serde_json::to_value(&readiness).unwrap();
    invalid_readiness["lookahead_frames"] = 64.into();
    assert!(serde_json::from_value::<BenchmarkReadiness>(invalid_readiness).is_err());

    let mut result = inline_benchmark_result();
    result.scenario = config.scenario.clone();
    result.requested_output_buffer_frames = 128;
    result.expected_alsa_buffer_frames = 128;
    result.expected_alsa_period_frames = 32;
    result.internal_block_frames = 64;
    result.lookahead_frames = 0;
    result.effective_output_latency_frames = 128;
    assert!(
        serde_json::from_value::<BenchmarkResult>(serde_json::to_value(&result).unwrap()).is_ok()
    );

    for (field, value) in [
        ("scenario", serde_json::json!("synth_ramp_16")),
        ("expected_alsa_period_frames", serde_json::json!(64)),
        ("lookahead_frames", serde_json::json!(64)),
        ("effective_output_latency_frames", serde_json::json!(192)),
    ] {
        let mut invalid = serde_json::to_value(&result).unwrap();
        invalid[field] = value;
        assert!(
            serde_json::from_value::<BenchmarkResult>(invalid).is_err(),
            "tampered result field should fail: {field}"
        );
    }
    for executor in ["persistent_two_workers", "routing_tree_persistent"] {
        let mut invalid = serde_json::to_value(&result).unwrap();
        invalid["executor_mode"] = executor.into();
        assert!(
            serde_json::from_value::<BenchmarkResult>(invalid).is_err(),
            "analogue geometry should be rejected for {executor}"
        );
    }
}

#[test]
fn recorded_analogue_geometry_is_inline_only() {
    for (executor_mode, lookahead_frames, effective_output_latency_frames) in [
        (BenchmarkExecutorMode::PersistentTwoWorkers, 0, Some(128)),
        (BenchmarkExecutorMode::RoutingTreePersistent, 64, Some(192)),
    ] {
        assert_eq!(
            validate_recorded_geometry(RecordedGeometry {
                scenario: "capacity_analogue_1",
                executor_mode,
                requested_output_buffer_frames: 128,
                expected_alsa_buffer_frames: 128,
                expected_alsa_period_frames: 32,
                internal_block_frames: 64,
                lookahead_frames,
                effective_output_latency_frames,
            })
            .unwrap_err(),
            "unsupported Orange benchmark geometry tuple: output=128 internal=64"
        );
    }
}
