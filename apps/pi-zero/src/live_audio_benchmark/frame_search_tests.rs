use super::cli::{
    validate_recorded_geometry, validate_requested_geometry, BenchmarkExecutorMode,
    RecordedGeometry, WorkerTimingMode,
};
use super::geometry::{expected_lookahead_frames, is_approved_geometry_tuple};
#[cfg(not(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
)))]
use super::metrics::CallbackMetricsSnapshot;
use super::parse;

const ARTIFACT_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn approved_tuples(executor_mode: BenchmarkExecutorMode) -> Vec<(u32, usize)> {
    if super::geometry::is_raspberry_diagnostic() {
        match executor_mode {
            BenchmarkExecutorMode::Inline => vec![(256, 64), (256, 128), (512, 128)],
            BenchmarkExecutorMode::RoutingTreePersistent => {
                vec![(256, 64), (256, 128), (256, 256)]
            }
            BenchmarkExecutorMode::PersistentTwoWorkers => Vec::new(),
        }
    } else {
        match executor_mode {
            BenchmarkExecutorMode::Inline => vec![(128, 32), (128, 64), (256, 64)],
            BenchmarkExecutorMode::RoutingTreePersistent => {
                vec![(128, 32), (256, 64), (256, 128)]
            }
            BenchmarkExecutorMode::PersistentTwoWorkers => Vec::new(),
        }
    }
}

fn scenario_for(
    executor_mode: BenchmarkExecutorMode,
    output_frames: u32,
    internal_frames: usize,
) -> &'static str {
    if cfg!(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    )) && !super::geometry::is_raspberry_diagnostic()
        && executor_mode == BenchmarkExecutorMode::Inline
        && output_frames == 128
        && internal_frames == 64
    {
        "capacity_analogue_1"
    } else {
        "synth_ramp_16"
    }
}

fn args_for(
    executor_mode: BenchmarkExecutorMode,
    scenario: &str,
    output_frames: u32,
    internal_frames: usize,
    measure_seconds: u64,
    worker_timing_mode: WorkerTimingMode,
    continue_on_recovered_miss: bool,
) -> Vec<String> {
    let mut args = vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        scenario.into(),
        "--output-frames".into(),
        output_frames.to_string(),
        "--engine-block-frames".into(),
        internal_frames.to_string(),
        "--executor".into(),
        executor_mode.as_str().into(),
        "--worker-timing".into(),
        worker_timing_mode.as_str().into(),
        "--measure-seconds".into(),
        measure_seconds.to_string(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        ARTIFACT_SHA256.into(),
    ];
    if continue_on_recovered_miss {
        args.push("--continue-on-recovered-miss".into());
    }
    args
}

#[test]
fn approved_frame_search_tuples_have_strict_period_and_lookahead_geometry() {
    for executor_mode in [
        BenchmarkExecutorMode::Inline,
        BenchmarkExecutorMode::RoutingTreePersistent,
    ] {
        for (output_frames, internal_frames) in approved_tuples(executor_mode) {
            assert!(is_approved_geometry_tuple(
                executor_mode,
                output_frames,
                internal_frames
            ));
            if !cfg!(any(
                feature = "benchmark-voice-pools-128",
                feature = "benchmark-voice-pools-256"
            )) && !super::geometry::is_raspberry_diagnostic()
                && executor_mode == BenchmarkExecutorMode::Inline
                && output_frames == 128
                && internal_frames == 64
            {
                continue;
            }
            let scenario = scenario_for(executor_mode, output_frames, internal_frames);
            validate_requested_geometry(scenario, executor_mode, output_frames, internal_frames)
                .unwrap();
            let period_frames = output_frames / 4;
            let lookahead_frames = expected_lookahead_frames(executor_mode, internal_frames);
            validate_recorded_geometry(RecordedGeometry {
                scenario,
                executor_mode,
                requested_output_buffer_frames: output_frames,
                expected_alsa_buffer_frames: output_frames,
                expected_alsa_period_frames: period_frames,
                internal_block_frames: internal_frames,
                lookahead_frames,
                effective_output_latency_frames: Some(output_frames as usize + lookahead_frames),
            })
            .unwrap();
        }
    }
}

#[test]
fn frame_search_rejects_cross_executor_and_unapproved_tuples() {
    for (executor_mode, rejected) in [
        (
            BenchmarkExecutorMode::Inline,
            if super::geometry::is_raspberry_diagnostic() {
                vec![(256, 256), (512, 64), (128, 32)]
            } else {
                vec![(64, 32), (512, 256), (1024, 128)]
            },
        ),
        (
            BenchmarkExecutorMode::RoutingTreePersistent,
            if super::geometry::is_raspberry_diagnostic() {
                vec![(128, 32), (512, 128), (256, 512)]
            } else {
                vec![(128, 64), (512, 128), (1024, 256)]
            },
        ),
    ] {
        for (output_frames, internal_frames) in rejected {
            assert!(!is_approved_geometry_tuple(
                executor_mode,
                output_frames,
                internal_frames
            ));
            assert!(validate_requested_geometry(
                "synth_ramp_16",
                executor_mode,
                output_frames,
                internal_frames
            )
            .is_err());
        }
    }
}

#[test]
fn generic_inline_geometry_is_accepted() {
    if super::geometry::is_raspberry_diagnostic() {
        return;
    }
    for (output_frames, internal_frames) in [
        (128, 32),
        (256, 64),
        (256, 128),
        (256, 256),
        (512, 128),
        (1024, 256),
    ] {
        assert!(
            validate_requested_geometry(
                "synth_ramp_16",
                BenchmarkExecutorMode::Inline,
                output_frames,
                internal_frames,
            )
            .is_ok(),
            "generic Inline tuple should remain accepted: output={output_frames} internal={internal_frames}"
        );
    }
}

#[test]
fn frame_search_preserves_all_approved_measurement_durations() {
    for measure_seconds in [30, 120, 180, 300, 600] {
        let config = parse(args_for(
            BenchmarkExecutorMode::RoutingTreePersistent,
            "synth_ramp_16",
            approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent)[0].0,
            approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent)[0].1,
            measure_seconds,
            WorkerTimingMode::Enabled,
            false,
        ))
        .unwrap();
        assert_eq!(config.measure_seconds, measure_seconds);
    }
    for measure_seconds in [31, 299, 301, 599, 601] {
        assert!(parse(args_for(
            BenchmarkExecutorMode::RoutingTreePersistent,
            "synth_ramp_16",
            approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent)[0].0,
            approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent)[0].1,
            measure_seconds,
            WorkerTimingMode::Enabled,
            false,
        ))
        .is_err());
    }
}

#[cfg(not(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
)))]
#[test]
fn six_hundred_second_clean_policy_uses_the_approved_overrun_budget() {
    let config = parse(args_for(
        BenchmarkExecutorMode::RoutingTreePersistent,
        "synth_ramp_16",
        128,
        32,
        600,
        WorkerTimingMode::Enabled,
        false,
    ))
    .unwrap();
    for (overrun_count, expected) in [(0, true), (9, true), (10, false)] {
        let metrics = CallbackMetricsSnapshot {
            callback_count: 1,
            callback_frames_min: 1,
            callback_frames_max: 1,
            callback_frame_sample_count: 1,
            over_audio_duration_budget_count: overrun_count,
            pre_mute_nonzero_samples: 1,
            ..Default::default()
        };
        assert_eq!(
            super::clean_policy::result_passes(&config, &metrics, 0),
            expected
        );
    }
}

#[path = "frame_search_continuation_tests.rs"]
mod continuation_tests;
