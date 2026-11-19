use super::{comb_bank_step, comb_buffers, ReverbInputGain};
use crate::synth::fx::{fx_bus_state_from_params, process_fx_bus_slot, FxBusParams};

const RATE: u32 = 44_100;

fn reverb(decay: f32) -> FxBusParams {
    FxBusParams::Reverb {
        mix: 0.7,
        decay,
        damp: 0.3,
    }
}

fn input(frame: usize) -> f32 {
    ((frame as f32) * 0.031).sin() * 0.8
}

#[test]
fn reverb_up_to_decay_point_nine_matches_the_unscaled_comb_input() {
    for decay in [0.0, 0.5, 0.85, 0.9] {
        let params = reverb(decay);
        let mut state = fx_bus_state_from_params(&params, RATE);
        let mut bufs = comb_buffers(1, RATE);
        let mut idxs = [0; 4];
        let mut lp = [0.0; 4];
        for frame in 0..20_000 {
            let x = input(frame);
            let actual = process_fx_bus_slot(&params, &mut state, x, 0.0, RATE);
            let wet = comb_bank_step(&mut bufs, &mut idxs, &mut lp, [x; 4], decay, 0.3)
                .iter()
                .fold(0.0, |wet, delayed| wet + delayed);
            let expected = (x * (1.0 - 0.7) + wet * 0.25 * 0.7).clamp(-1.5, 1.5);
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "decay {decay} frame {frame}"
            );
        }
    }
}

#[test]
fn reverb_input_gain_follows_the_tail_energy_above_decay_point_nine() {
    let mut gain = ReverbInputGain::default();
    assert_eq!(gain.for_decay(0.9), 1.0);
    assert_eq!(gain.for_decay(0.4), 1.0);
    assert!((gain.for_decay(0.95) - (0.0975_f32 / 0.19).sqrt()).abs() < 1.0e-5);
    assert!((gain.for_decay(0.995) - 0.229_129).abs() < 1.0e-4);
    assert!((gain.for_decay(0.900_01) - 1.0).abs() < 1.0e-3);
}
