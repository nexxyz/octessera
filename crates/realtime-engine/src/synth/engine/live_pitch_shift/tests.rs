use super::LivePitchShift;

#[test]
fn live_pitch_interpolation_wraps_from_last_frame_to_first() {
    let buffer = [1.0_f32, -1.0, 3.0, -3.0];

    assert_eq!(LivePitchShift::interp(&buffer, 1.5, 0), 2.0);
    assert_eq!(LivePitchShift::interp(&buffer, 1.5, 1), -2.0);
}

#[test]
fn live_pitch_read_position_keeps_advancing_by_one_minus_ratio_through_a_long_hold() {
    let ratio = 2.0_f32.powf(1.0 / 12.0);
    let range = 1024.0_f32;
    let expected = (1.0 - ratio).rem_euclid(range);
    let mut shift = LivePitchShift::new(44_100);
    let mut previous = shift.pos;
    for frame in 0..44_100 * 60 * 12 {
        shift.process_frame(0.0, 0.0, ratio);
        assert!(
            (0.0..range).contains(&shift.pos),
            "frame {frame}: pos {}",
            shift.pos
        );
        let advance = (shift.pos - previous).rem_euclid(range);
        assert!(
            (advance - expected).abs() < 1.0e-3,
            "frame {frame}: advanced {advance}, expected {expected}"
        );
        previous = shift.pos;
    }
}
