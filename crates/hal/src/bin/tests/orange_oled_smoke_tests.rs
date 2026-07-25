use super::{
    black_frame, ensure_not_interrupted, parse_args, require_active_test_confirmation,
    static_test_pattern, Options, DISPLAY_HEIGHT, DISPLAY_WIDTH,
};

#[test]
fn requires_active_test_confirmation() {
    assert_eq!(
        parse_args(Vec::<String>::new()).unwrap(),
        Options {
            confirm_active_test: false,
            print_build_metadata: false,
        }
    );
    assert_eq!(
        parse_args(["--confirm-active-test"]).unwrap(),
        Options {
            confirm_active_test: true,
            print_build_metadata: false,
        }
    );
    assert!(require_active_test_confirmation(Options {
        confirm_active_test: false,
        print_build_metadata: false,
    })
    .is_err());
}

#[test]
fn metadata_mode_is_exclusive_with_active_test() {
    assert_eq!(
        parse_args(["--print-build-metadata"]).unwrap(),
        Options {
            confirm_active_test: false,
            print_build_metadata: true,
        }
    );
    assert!(parse_args(["--confirm-active-test", "--print-build-metadata"]).is_err());
}

#[test]
fn rejects_unknown_and_duplicate_arguments() {
    assert!(parse_args(["--nope"]).is_err());
    assert!(parse_args(["--confirm-active-test", "--confirm-active-test"]).is_err());
    assert!(parse_args(["--print-build-metadata", "--print-build-metadata"]).is_err());
}

#[test]
fn interruption_check_rejects_only_an_interrupted_run() {
    assert!(ensure_not_interrupted(false).is_ok());
    assert_eq!(
        ensure_not_interrupted(true),
        Err("Orange OLED smoke interrupted; cleanup is being attempted")
    );
}

#[test]
fn static_pattern_has_one_rgb565_pixel_per_display_pixel() {
    let pattern = static_test_pattern();
    assert_eq!(pattern.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT * 2);
    assert_ne!(&pattern[..2], &pattern[pattern.len() - 2..]);
}

#[test]
fn black_frame_is_complete_and_all_zero() {
    let frame = black_frame();
    assert_eq!(frame.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT * 2);
    assert!(frame.iter().all(|byte| *byte == 0));
}
