use super::LivePitchShift;

#[test]
fn live_pitch_interpolation_wraps_from_last_frame_to_first() {
    let buffer = [1.0_f32, -1.0, 3.0, -3.0];

    assert_eq!(LivePitchShift::interp(&buffer, 1.5, 0), 2.0);
    assert_eq!(LivePitchShift::interp(&buffer, 1.5, 1), -2.0);
}
