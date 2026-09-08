use super::*;

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
#[test]
fn raspberry_result_status_requires_zero_persistent_output_events() {
    let config = crate::live_audio_benchmark::cli::parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "64".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    let mut counters = PersistentOutputCountersEvidence::for_executor(
        crate::live_audio_benchmark::cli::BenchmarkExecutorMode::RoutingTreePersistent,
    );
    counters.start.rendered_quantums = 1;
    counters.end.rendered_quantums = 2;
    counters.delta.rendered_quantums = 1;
    counters.delta.dropped_quantums = 1;
    assert!(!persistent_output_counters_passes(&config, &counters));
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
#[test]
fn raspberry_callback_events_fail_the_clean_policy_at_each_duration() {
    let config = crate::live_audio_benchmark::cli::parse(vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_cross_slot_96_steal".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "64".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ])
    .unwrap();
    for measure_seconds in [30, 120, 180, 300] {
        assert!(!result_passes(
            &crate::live_audio_benchmark::cli::BenchmarkConfig {
                measure_seconds,
                ..config.clone()
            },
            &CallbackMetricsSnapshot {
                callback_count: 1,
                callback_frames_min: 1,
                callback_frames_max: 1,
                callback_frame_sample_count: 1,
                over_audio_duration_budget_count: 1,
                pre_mute_nonzero_samples: 1,
                ..Default::default()
            },
            0,
        ));
    }
}
