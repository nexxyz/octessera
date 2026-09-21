use super::*;
use crate::live_audio_benchmark::geometry::is_raspberry_diagnostic;

fn valid_args() -> Vec<String> {
    vec![
        "--benchmark-orange-audio".into(),
        "--scenario".into(),
        "synth_ramp_16".into(),
        "--output-frames".into(),
        "256".into(),
        "--engine-block-frames".into(),
        "64".into(),
        "--release-gate".into(),
        "release.json".into(),
        "--artifact-sha256".into(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
    ]
}

fn args_for(output_frames: u32, internal_frames: usize) -> Vec<String> {
    let mut args = valid_args();
    set_arg(&mut args, "--output-frames", output_frames.to_string());
    set_arg(
        &mut args,
        "--engine-block-frames",
        internal_frames.to_string(),
    );
    if output_frames > 256 {
        args.extend([
            "--executor".into(),
            "inline".into(),
            "--worker-timing".into(),
            "disabled".into(),
        ]);
    }
    args
}

fn inline_args_for(scenario: &str, output_frames: u32, internal_frames: usize) -> Vec<String> {
    let mut args = args_for(output_frames, internal_frames);
    set_arg(&mut args, "--scenario", scenario.into());
    args.extend([
        "--executor".into(),
        "inline".into(),
        "--worker-timing".into(),
        "disabled".into(),
    ]);
    args
}

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
fn recovered_miss_args(scenario: &str) -> Vec<String> {
    let mut args = valid_args();
    set_arg(&mut args, "--scenario", scenario.into());
    set_arg(
        &mut args,
        "--engine-block-frames",
        if is_raspberry_diagnostic() {
            "128".into()
        } else {
            "64".into()
        },
    );
    args.extend([
        "--measure-seconds".into(),
        "120".into(),
        "--executor".into(),
        "routing_tree_persistent".into(),
        "--worker-timing".into(),
        "disabled".into(),
        "--continue-on-recovered-miss".into(),
    ]);
    args
}

fn set_arg(args: &mut [String], name: &str, value: String) {
    let index = args.iter().position(|arg| arg == name).unwrap();
    args[index + 1] = value;
}

fn remove_arg(args: &mut Vec<String>, name: &str) {
    let index = args.iter().position(|arg| arg == name).unwrap();
    args.drain(index..=index + 1);
}

#[test]
fn approved_cli_tuples_store_independent_geometry() {
    if is_raspberry_diagnostic() {
        let inline = parse(inline_args_for("capacity_analogue_1", 256, 128)).unwrap();
        assert_eq!(inline.executor_mode, BenchmarkExecutorMode::Inline);
        assert_eq!(inline.output_frames, 256);
        assert_eq!(inline.expected_alsa_period_frames, 64);
        assert_eq!(inline.internal_frames, 128);

        let mut routing_args = valid_args();
        set_arg(&mut routing_args, "--engine-block-frames", "128".into());
        routing_args.extend(["--executor".into(), "routing_tree_persistent".into()]);
        let routing = parse(routing_args).unwrap();
        assert_eq!(
            routing.executor_mode,
            BenchmarkExecutorMode::RoutingTreePersistent
        );
        assert_eq!(routing.output_frames, 256);
        assert_eq!(routing.expected_alsa_period_frames, 64);
        assert_eq!(routing.internal_frames, 128);
    } else {
        let tuples = vec![
            (128, 32, 32),
            (256, 64, 64),
            (256, 128, 64),
            (256, 256, 64),
            (512, 128, 128),
            (1024, 256, 256),
        ];
        for (output, internal, period) in tuples {
            let config = parse(args_for(output, internal)).unwrap();
            assert_eq!(config.output_frames, output);
            assert_eq!(config.expected_alsa_period_frames, period);
            assert_eq!(config.internal_frames, internal);
        }
    }
    let config = parse(valid_args()).unwrap();
    assert_eq!(
        config.executor_mode,
        BenchmarkExecutorMode::RoutingTreePersistent
    );
    assert_eq!(config.worker_timing_mode, WorkerTimingMode::Enabled);
    assert_ne!(config.result_path, config.progress_path);
    assert_ne!(config.result_path, config.readiness_path);
    assert_ne!(config.progress_path, config.readiness_path);
    assert!(!config.continue_on_recovered_miss);
}

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
#[test]
fn continue_on_recovered_miss_accepts_only_the_routing_observation_cell() {
    let max_units = realtime_engine::synth::SAMPLE_VOICE_LANE_CAPACITY
        .min(realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY / 3);
    for units in 1..=max_units {
        let config = parse(recovered_miss_args(&format!("capacity_analogue_{units}"))).unwrap();
        assert!(config.continue_on_recovered_miss);
        assert_eq!(config.worker_timing_mode, WorkerTimingMode::Disabled);
        preflight(&config).unwrap();
    }

    let mut enabled_timing = recovered_miss_args("capacity_analogue_16");
    set_arg(&mut enabled_timing, "--worker-timing", "enabled".into());
    assert!(parse(enabled_timing).is_err());

    let mut invalid_measure = recovered_miss_args("capacity_analogue_16");
    invalid_measure.extend(["--measure-seconds".into(), "30".into()]);
    assert!(parse(invalid_measure).is_err());
}

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    feature = "routing-tree-benchmark"
))]
#[test]
fn continue_on_recovered_miss_rejects_non_analogue_cells() {
    let mut args = recovered_miss_args("synth_ramp_16");
    assert!(parse(args.clone()).is_err());
    args = recovered_miss_args("capacity_analogue_16");
    args.extend(["--executor".into(), "persistent_two_workers".into()]);
    assert!(parse(args).is_err());
}

#[test]
fn inline_128_64_requires_an_analogue_capacity_scenario() {
    assert_eq!(
        parse(inline_args_for("synth_ramp_16", 128, 64)).unwrap_err(),
        format!(
            "unsupported {} benchmark geometry tuple: output=128 internal=64",
            super::super::platform::BENCHMARK_LABEL
        )
    );
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[test]
fn continue_on_recovered_miss_rejects_inline_executor() {
    let mut args = inline_args_for(
        "capacity_analogue_1",
        if is_raspberry_diagnostic() { 256 } else { 128 },
        if is_raspberry_diagnostic() { 128 } else { 64 },
    );
    args.extend([
        "--measure-seconds".into(),
        "120".into(),
        "--continue-on-recovered-miss".into(),
    ]);
    assert!(parse(args).is_err());
}

#[cfg(all(
    any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ),
    not(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))
))]
#[test]
fn analogue_capacity_128_64_is_inline_only_and_preflight_validates_it() {
    let mut config = parse(inline_args_for("capacity_analogue_1", 128, 64)).unwrap();
    assert_eq!(config.expected_alsa_period_frames, 32);
    assert_eq!(config.executor_mode, BenchmarkExecutorMode::Inline);
    assert_eq!(config.internal_frames, 64);
    preflight(&config).unwrap();

    config.worker_timing_mode = WorkerTimingMode::Enabled;
    assert_eq!(
        preflight(&config).unwrap_err(),
        "inline executor requires worker timing disabled"
    );

    let mut persistent = args_for(128, 64);
    set_arg(&mut persistent, "--scenario", "capacity_analogue_1".into());
    assert!(parse(persistent).is_err());

    let mut routing = args_for(128, 64);
    set_arg(&mut routing, "--scenario", "capacity_analogue_1".into());
    routing.extend(["--executor".into(), "routing_tree_persistent".into()]);
    assert!(parse(routing).is_err());

    for scenario in [
        "capacity_analogue_0",
        "capacity_analogue_01",
        "capacity_analogue_1x",
    ] {
        assert!(
            parse(inline_args_for(scenario, 128, 64)).is_err(),
            "invalid analogue scenario should fail: {scenario}"
        );
    }
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
#[test]
fn raspberry_analogue_capacity_geometry_is_exact_by_executor() {
    let inline = parse(inline_args_for("capacity_analogue_16", 256, 128)).unwrap();
    assert_eq!(inline.expected_alsa_period_frames, 64);
    assert_eq!(inline.internal_frames, 128);

    let routing = parse(recovered_miss_args("capacity_analogue_16")).unwrap();
    assert_eq!(routing.expected_alsa_period_frames, 64);
    assert_eq!(routing.internal_frames, 128);

    for (executor, output, internal) in [
        ("inline", 128, 32),
        ("inline", 256, 256),
        ("routing_tree_persistent", 128, 32),
    ] {
        let mut args = valid_args();
        set_arg(&mut args, "--scenario", "capacity_analogue_16".into());
        set_arg(&mut args, "--output-frames", output.to_string());
        set_arg(&mut args, "--engine-block-frames", internal.to_string());
        args.extend(["--executor".into(), executor.into()]);
        if executor == "inline" {
            args.extend(["--worker-timing".into(), "disabled".into()]);
        }
        assert!(parse(args).is_err());
    }
}

#[test]
fn worker_timing_modes_round_trip_exactly_and_reject_invalid_values() {
    for (value, expected) in [
        ("enabled", WorkerTimingMode::Enabled),
        ("disabled", WorkerTimingMode::Disabled),
    ] {
        let mut args = valid_args();
        args.extend(["--worker-timing".into(), value.into()]);
        let config = parse(args).unwrap();
        assert_eq!(config.worker_timing_mode, expected);
        assert_eq!(config.worker_timing_mode.as_str(), value);
    }
    for value in ["Enabled", "disabled-now", "1"] {
        let mut args = valid_args();
        args.extend(["--worker-timing".into(), value.into()]);
        assert!(
            parse(args).is_err(),
            "worker timing value should fail: {value}"
        );
    }
}

#[test]
fn executor_modes_round_trip_exactly_and_require_disabled_inline_timing() {
    for (value, expected) in [
        ("inline", BenchmarkExecutorMode::Inline),
        (
            "routing_tree_persistent",
            BenchmarkExecutorMode::RoutingTreePersistent,
        ),
    ] {
        let mut args = valid_args();
        args.extend(["--executor".into(), value.into()]);
        if expected == BenchmarkExecutorMode::Inline {
            args.extend(["--worker-timing".into(), "disabled".into()]);
        }
        let config = parse(args).unwrap();
        assert_eq!(config.executor_mode, expected);
        assert_eq!(config.executor_mode.as_str(), value);
    }

    let mut routing = args_for(256, 128);
    routing.extend(["--executor".into(), "routing_tree_persistent".into()]);
    let routing_config = parse(routing).unwrap();
    assert_eq!(
        routing_config.executor_mode,
        BenchmarkExecutorMode::RoutingTreePersistent
    );
    assert_eq!(
        routing_config.executor_mode.as_str(),
        "routing_tree_persistent"
    );
    let mut invalid_combination = valid_args();
    invalid_combination.extend(["--executor".into(), "inline".into()]);
    assert_eq!(
        parse(invalid_combination).unwrap_err(),
        "inline executor requires worker timing disabled"
    );
    for value in ["INLINE", "persistent", "1"] {
        let mut args = valid_args();
        args.extend(["--executor".into(), value.into()]);
        assert!(parse(args).is_err(), "executor value should fail: {value}");
    }
    let mut removed = valid_args();
    removed.extend(["--executor".into(), "persistent_two_workers".into()]);
    assert_eq!(
        parse(removed).unwrap_err(),
        "persistent_two_workers executor was removed; use routing_tree_persistent"
    );
}

#[cfg(not(feature = "routing-tree-benchmark"))]
#[test]
fn routing_tree_selection_fails_preflight_before_runtime_access() {
    let mut args = args_for(256, 128);
    args.extend(["--executor".into(), "routing_tree_persistent".into()]);
    let config = parse(args).unwrap();
    assert_eq!(
        preflight(&config).unwrap_err(),
        "routing_tree_persistent executor requires a binary built with routing-tree-benchmark"
    );
}

#[test]
fn historical_order_is_unchanged_and_baseline_live_ids_are_separate() {
    let historical: Vec<_> = ScenarioId::ALL
        .into_iter()
        .map(ScenarioId::as_str)
        .collect();
    assert_eq!(
        historical,
        vec![
            "synth_ramp_16",
            "synth_ramp_32",
            "synth_ramp_64",
            "sample_ramp_64",
            "mixed_ramp_16_16",
            "mixed_ramp_32_32",
            "bus_heavy_6_bus_fx_2_global",
            "momentary_combined",
            "synth_cross_slot_96_steal",
            "sample_cross_slot_96_steal",
            "mixed_cross_slot_48_48_steal",
        ]
    );
    for id in ScenarioId::BASELINE_LIVE {
        assert_eq!(ScenarioId::parse(id.as_str()), Some(id));
    }
    assert_eq!(ScenarioId::MixedRamp16_48.as_str(), "mixed_ramp_16_48");
    assert!(ScenarioId::parse("baseline_idle").is_none());
}

#[test]
fn baseline_live_ids_match_canonical_scenarios_and_parse_at_180_seconds() {
    let native_ids = ScenarioId::BASELINE_LIVE.map(ScenarioId::as_str);
    assert_eq!(native_ids, crate::dsp_scenarios::BASELINE_LIVE_SCENARIO_IDS);
    assert_eq!(
        native_ids,
        [
            "synth_cross_slot_16",
            "sample_cross_slot_64",
            "mixed_16_synth_32_sample",
            "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
            "synth_cross_slot_32_no_steal",
            "mixed_ramp_16_48",
            "default_envelope_24_synth_8_sample",
            "default_headroom_32_synth_8_sample",
            "default_headroom_32_synth_16_sample",
            "default_headroom_40_synth_16_sample",
            "default_headroom_48_synth_16_sample",
            "default_capacity_64_synth_16_sample",
            "default_capacity_48_synth_64_sample",
            "default_capacity_64_synth_64_sample",
        ]
    );

    for id in ScenarioId::BASELINE_LIVE.into_iter().skip(6) {
        let mut args = args_for(256, 64);
        set_arg(&mut args, "--scenario", id.as_str().into());
        args.extend(["--measure-seconds".into(), "180".into()]);
        let config = parse(args).unwrap();
        assert_eq!(config.scenario, id.as_str());
        assert_eq!(config.measure_seconds, 180);
    }
}

#[cfg(not(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
)))]
#[test]
fn large_pool_scenario_names_are_rejected_in_normal_builds() {
    for name in [
        "capacity_synth_64",
        "capacity_sample_64",
        "capacity_mixed_64_64",
        "capacity_analogue_1",
    ] {
        let mut args = valid_args();
        set_arg(&mut args, "--scenario", name.into());
        assert!(parse(args).is_err(), "normal build accepted {name}");
    }
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[test]
fn large_pool_scenario_names_round_trip_as_exact_strings() {
    let capacity = realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY;
    for name in [
        format!("capacity_synth_{capacity}"),
        format!("capacity_sample_{capacity}"),
        format!("capacity_mixed_{capacity}_{capacity}"),
        format!(
            "capacity_analogue_{}",
            realtime_engine::synth::SAMPLE_VOICE_LANE_CAPACITY
                .min(realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY / 3)
        ),
    ] {
        let mut args = valid_args();
        set_arg(&mut args, "--scenario", name.clone());
        let config = parse(args).unwrap();
        assert_eq!(config.scenario, name);
    }
}

#[cfg(feature = "benchmark-voice-pools-128")]
#[test]
fn analogue_capacity_load_reaches_u36_without_duplicate_note_generation() {
    let scenario = crate::dsp_scenarios::live_scenario("capacity_analogue_36", 44_100, 180)
        .expect("U36 analogue capacity scenario");
    assert_eq!(scenario.expected.active_synth_voices, 108);
    assert_eq!(scenario.expected.active_sample_voices, 36);
    let note_on_count = scenario
        .events
        .iter()
        .filter(|event| matches!(event, rodio_engine_source::EngineEvent::NoteOn { .. }))
        .count();
    assert_eq!(note_on_count, 144);
}

#[path = "cli_boundary_tests.rs"]
mod boundary_tests;
