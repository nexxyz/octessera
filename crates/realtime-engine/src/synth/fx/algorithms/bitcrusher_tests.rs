use super::process_bitcrusher;
use crate::synth::fx::FxBusState;

fn crush(input: &[f32], rate_div: u32, bits: u32) -> Vec<f32> {
    let mut state = FxBusState::Bitcrusher {
        hold: rate_div,
        count: 0,
        last: 0.0,
    };
    input
        .iter()
        .map(|sample| process_bitcrusher(&mut state, *sample, rate_div, bits, 1.0))
        .collect()
}

#[test]
fn silence_stays_silent() {
    assert!(crush(&[0.0; 64], 4, 8).iter().all(|sample| *sample == 0.0));
}

#[test]
fn a_fading_tail_below_half_a_step_rounds_to_zero_instead_of_toggling() {
    let tail: Vec<f32> = (0..256)
        .map(|index| 0.003 * if index % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    assert!(crush(&tail, 1, 8).iter().all(|sample| *sample == 0.0));
}

#[test]
fn the_crusher_is_symmetric_around_zero() {
    let up = crush(&[0.3], 1, 4)[0];
    let down = crush(&[-0.3], 1, 4)[0];
    assert_eq!(up, -down);
    assert_eq!(up, 0.25);
}
