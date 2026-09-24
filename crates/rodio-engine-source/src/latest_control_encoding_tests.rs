use super::{decode_momentary, encode_momentary};
use realtime_engine::synth::PreparedMomentaryFxUpdate;

#[test]
fn pitch_update_encoding_round_trips_finite_concrete_values() {
    let update = PreparedMomentaryFxUpdate::PitchShift {
        epoch: 7,
        target_octaves: (-5.0 + 0.25) / 12.0,
        mix: 0.65,
        slide_in_len: 5_292,
        slide_out_len: 7_938,
    };
    let (kind, values) = encode_momentary(update);
    let PreparedMomentaryFxUpdate::PitchShift {
        epoch,
        target_octaves,
        mix,
        slide_in_len,
        slide_out_len,
    } = decode_momentary(7, kind, values).expect("pitch update")
    else {
        panic!("decoded non-pitch update");
    };
    assert_eq!(epoch, 7);
    assert!(target_octaves.is_finite());
    assert!(mix.is_finite());
    assert_eq!(
        target_octaves.to_bits(),
        ((-5.0_f32 + 0.25) / 12.0_f32).to_bits()
    );
    assert_eq!(mix.to_bits(), 0.65_f32.to_bits());
    assert_eq!(slide_in_len, 5_292);
    assert_eq!(slide_out_len, 7_938);
}
