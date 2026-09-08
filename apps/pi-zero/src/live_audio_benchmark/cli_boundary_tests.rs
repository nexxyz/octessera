use super::*;

#[test]
fn routing_tree_executor_rejects_output_buffers_above_256() {
    let mut args = args_for(512, 128);
    args.extend(["--executor".into(), "routing_tree_persistent".into()]);
    assert_eq!(
        parse(args).unwrap_err(),
        "routing_tree_persistent executor requires output frames <= 256"
    );
}

#[test]
fn mixed_boundary_cli_accepts_only_approved_geometry_and_duration() {
    let tuples = if is_raspberry_diagnostic() {
        vec![(256, 128)]
    } else {
        vec![
            (128, 32),
            (256, 64),
            (256, 128),
            (256, 256),
            (512, 128),
            (1024, 256),
        ]
    };
    for (output, internal) in tuples {
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
