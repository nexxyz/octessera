use super::*;

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
    for (output, internal, period) in [
        (128, 32, 32),
        (256, 64, 64),
        (256, 128, 64),
        (256, 256, 64),
        (512, 128, 128),
        (1024, 256, 256),
    ] {
        let config = parse(args_for(output, internal)).unwrap();
        assert_eq!(config.output_frames, output);
        assert_eq!(config.expected_alsa_period_frames, period);
        assert_eq!(config.internal_frames, internal);
    }
    let config = parse(valid_args()).unwrap();
    assert_eq!(
        config.executor_mode,
        BenchmarkExecutorMode::PersistentTwoWorkers
    );
    assert_eq!(config.worker_timing_mode, WorkerTimingMode::Enabled);
    assert_ne!(config.result_path, config.progress_path);
    assert_ne!(config.result_path, config.readiness_path);
    assert_ne!(config.progress_path, config.readiness_path);
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
            "persistent_two_workers",
            BenchmarkExecutorMode::PersistentTwoWorkers,
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
        assert_eq!(config.scenario, id);
        assert_eq!(config.measure_seconds, 180);
    }
}

#[test]
fn mixed_boundary_cli_accepts_only_approved_geometry_and_duration() {
    for (output, internal) in [
        (128, 32),
        (256, 64),
        (256, 128),
        (256, 256),
        (512, 128),
        (1024, 256),
    ] {
        for seconds in [30, 120, 180, 300] {
            let mut args = args_for(output, internal);
            set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
            args.extend(["--measure-seconds".into(), seconds.to_string()]);
            assert_eq!(parse(args).unwrap().measure_seconds, seconds);
        }
    }
    for (output, internal) in [(128, 64), (256, 32), (512, 256), (1024, 128)] {
        let mut args = args_for(output, internal);
        set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
        assert!(parse(args).is_err());
    }
    for seconds in [299, 3000] {
        let mut args = valid_args();
        set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
        args.extend(["--measure-seconds".into(), seconds.to_string()]);
        assert!(parse(args).is_err());
    }
}

#[test]
fn engine_block_frames_are_mandatory_and_unsupported_tuples_are_rejected() {
    let mut missing = valid_args();
    remove_arg(&mut missing, "--engine-block-frames");
    assert_eq!(
        parse(missing).unwrap_err(),
        "--engine-block-frames is required"
    );
    let mut invalid_block = valid_args();
    set_arg(&mut invalid_block, "--engine-block-frames", "512".into());
    assert!(parse(invalid_block).is_err());
    for (output, internal) in [(128, 64), (64, 32), (256, 32), (512, 256), (1024, 128)] {
        assert!(parse(args_for(output, internal)).is_err());
    }
}

#[test]
fn invalid_scenario_duration_and_unmuted_are_rejected() {
    assert!(parse(vec!["--benchmark-orange-audio".into()]).is_err());
    let mut args = valid_args();
    args[1] = "--unmuted".into();
    assert!(parse(args).is_err());
    let mut args = valid_args();
    args.retain(|arg| arg != "--artifact-sha256" && arg.len() != 64);
    assert!(parse(args).is_err());
    let mut args = valid_args();
    args.push("--measure-seconds".into());
    args.push("300".into());
    assert_eq!(parse(args).unwrap().measure_seconds, 300);
    let mut args = valid_args();
    args.push("--measure-seconds".into());
    args.push("180".into());
    assert_eq!(parse(args).unwrap().measure_seconds, 180);
    for seconds in [31, 299, 3000] {
        let mut args = valid_args();
        args.push("--measure-seconds".into());
        args.push(seconds.to_string());
        assert!(
            parse(args).is_err(),
            "duration {seconds} should be rejected"
        );
    }
    let mut args = valid_args();
    set_arg(
        &mut args,
        "--artifact-sha256",
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF".into(),
    );
    assert!(parse(args).is_err());
}
