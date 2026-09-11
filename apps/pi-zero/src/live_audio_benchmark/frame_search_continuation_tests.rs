#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
use super::super::cli::is_approved_continue_on_recovered_miss;
#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
use super::*;

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
#[test]
fn continuation_accepts_only_approved_routing_observations() {
    let routing_tuples = approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent);
    for (output_frames, internal_frames) in routing_tuples.iter().copied() {
        for measure_seconds in [180, 600] {
            let config = parse(args_for(
                BenchmarkExecutorMode::RoutingTreePersistent,
                "capacity_analogue_16",
                output_frames,
                internal_frames,
                measure_seconds,
                WorkerTimingMode::Disabled,
                true,
            ))
            .unwrap();
            assert!(config.continue_on_recovered_miss);
            assert!(is_approved_continue_on_recovered_miss(
                &config.scenario,
                config.executor_mode,
                config.output_frames,
                config.expected_alsa_period_frames,
                config.internal_frames,
                config.measure_seconds,
                config.worker_timing_mode,
            ));
        }
    }

    let legacy_tuple = if super::super::geometry::is_raspberry_diagnostic() {
        (256, 128)
    } else {
        (256, 64)
    };
    let legacy = parse(args_for(
        BenchmarkExecutorMode::RoutingTreePersistent,
        "capacity_analogue_16",
        legacy_tuple.0,
        legacy_tuple.1,
        120,
        WorkerTimingMode::Disabled,
        true,
    ))
    .unwrap();
    assert!(legacy.continue_on_recovered_miss);
}

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
#[test]
fn continuation_rejects_non_analogue_timing_duration_and_geometry_boundaries() {
    let approved = approved_tuples(BenchmarkExecutorMode::RoutingTreePersistent);
    let (output_frames, internal_frames) = approved[0];
    let legacy_tuple = if super::super::geometry::is_raspberry_diagnostic() {
        (256, 128)
    } else {
        (256, 64)
    };
    let cases = [
        (
            "synth_ramp_16",
            output_frames,
            internal_frames,
            180,
            WorkerTimingMode::Disabled,
        ),
        (
            "capacity_analogue_16",
            output_frames,
            internal_frames,
            180,
            WorkerTimingMode::Enabled,
        ),
        (
            "capacity_analogue_16",
            output_frames,
            internal_frames,
            30,
            WorkerTimingMode::Disabled,
        ),
        (
            "capacity_analogue_16",
            output_frames,
            internal_frames,
            300,
            WorkerTimingMode::Disabled,
        ),
        (
            "capacity_analogue_16",
            output_frames,
            internal_frames,
            120,
            WorkerTimingMode::Disabled,
        ),
    ];
    for (scenario, output, internal, measure, timing) in cases {
        assert!(parse(args_for(
            BenchmarkExecutorMode::RoutingTreePersistent,
            scenario,
            output,
            internal,
            measure,
            timing,
            true,
        ))
        .is_err());
    }

    for (output_frames, internal_frames) in approved.into_iter().skip(1) {
        if (output_frames, internal_frames) == legacy_tuple {
            continue;
        }
        assert!(parse(args_for(
            BenchmarkExecutorMode::RoutingTreePersistent,
            "capacity_analogue_16",
            output_frames,
            internal_frames,
            120,
            WorkerTimingMode::Disabled,
            true,
        ))
        .is_err());
    }

    let (inline_output, inline_internal) = super::super::geometry::is_raspberry_diagnostic()
        .then_some((256, 128))
        .unwrap_or((128, 32));
    assert!(parse(args_for(
        BenchmarkExecutorMode::Inline,
        "capacity_analogue_16",
        inline_output,
        inline_internal,
        600,
        WorkerTimingMode::Disabled,
        true,
    ))
    .is_err());
}
