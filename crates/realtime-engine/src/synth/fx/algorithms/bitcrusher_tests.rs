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

#[test]
fn bit_depth_sets_the_step_and_rounds_to_the_nearest_level() {
    let ramp: Vec<f32> = (0..=1000).map(|index| index as f32 / 500.0 - 1.0).collect();
    for bits in [2, 3, 8] {
        let step = 2.0 / (1_u32 << bits) as f32;
        for (input, output) in ramp.iter().zip(crush(&ramp, 1, bits)) {
            let levels = output / step;
            assert!(
                (levels - levels.round()).abs() < 1.0e-4,
                "{bits} bits: {output} is not a multiple of {step}"
            );
            assert!(
                (output - input).abs() <= step * 0.5 + 1.0e-6,
                "{bits} bits: {input} became {output}"
            );
        }
    }
}

#[test]
fn one_bit_keeps_only_the_sign_of_anything_above_minus_60_db() {
    let input = [0.9, 0.2, 0.01, 0.002, 0.0005, 0.0, -0.0005, -0.002, -0.3];
    assert_eq!(
        crush(&input, 1, 1),
        [0.5, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, -0.5, -0.5]
    );
}

#[test]
fn rate_division_holds_each_sample_for_that_many_frames() {
    let ramp: Vec<f32> = (0..64).map(|index| index as f32 / 64.0).collect();
    let held = crush(&ramp, 4, 16);
    let full_rate = crush(&ramp, 1, 16);
    for (frame, output) in held.iter().enumerate() {
        assert_eq!(*output, full_rate[frame - frame % 4], "frame {frame}");
    }
}

#[test]
fn the_mix_blends_the_dry_and_crushed_signal() {
    let mut state = FxBusState::Bitcrusher {
        hold: 1,
        count: 0,
        last: 0.0,
    };
    let output = process_bitcrusher(&mut state, 0.3, 1, 2, 0.25);
    assert!((output - (0.75 * 0.3 + 0.25 * 0.5)).abs() < 1.0e-6);
}

#[test]
fn a_level_above_full_scale_is_quantized_not_clipped_so_master_volume_can_still_bring_it_down() {
    let crushed = crush(&[1.4, -1.6], 1, 16);
    assert!((crushed[0] - 1.4).abs() < 1.0e-4, "{crushed:?}");
    assert!((crushed[1] + 1.6).abs() < 1.0e-4, "{crushed:?}");
}
