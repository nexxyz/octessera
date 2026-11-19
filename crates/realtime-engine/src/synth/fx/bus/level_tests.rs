use super::*;

const SAMPLE_RATE: u32 = 48_000;

#[test]
fn tremolo_gain_swings_from_full_level_down_by_the_depth() {
    let depth = 0.6;
    let params = FxBusParams::Tremolo {
        rate_hz: 1.0,
        depth,
    };
    let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
    let gains: Vec<f32> = (0..SAMPLE_RATE)
        .map(|_| process_fx_bus_slot(&params, &mut state, 1.0, 0.0, SAMPLE_RATE))
        .collect();
    let quarter = SAMPLE_RATE as usize / 4;
    for (frame, expected) in [
        (0, 1.0 - depth * 0.5),
        (quarter, 1.0),
        (2 * quarter, 1.0 - depth * 0.5),
        (3 * quarter, 1.0 - depth),
    ] {
        let gain = gains[frame];
        assert!(
            (gain - expected).abs() < 2.0e-3,
            "frame {frame}: gain {gain}, expected {expected}"
        );
    }
}

#[test]
fn auto_pan_passes_the_signal_and_moves_its_position_with_the_lfo() {
    for depth in [1.0, 0.5] {
        let params = FxBusParams::AutoPan {
            rate_hz: 1.0,
            depth,
        };
        let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
        let quarter = SAMPLE_RATE as usize / 4;
        for frame in 0..=3 * quarter {
            let input = (frame as f32 * 0.01).sin();
            let output = process_fx_bus_slot(&params, &mut state, input, 0.0, SAMPLE_RATE);
            assert_eq!(output.to_bits(), input.to_bits());
            let FxBusState::AutoPan { pos, .. } = state else {
                panic!("auto pan state");
            };
            let expected = match frame {
                0 => 0.5,
                f if f == quarter => 0.5 + 0.5 * depth,
                f if f == 2 * quarter => 0.5,
                f if f == 3 * quarter => 0.5 - 0.5 * depth,
                _ => continue,
            };
            assert!(
                (pos - expected).abs() < 2.0e-3,
                "depth {depth} frame {frame}: position {pos}, expected {expected}"
            );
        }
    }
}

#[test]
fn duck_cuts_by_the_amount_as_the_source_rises_past_the_threshold() {
    let (threshold, amount) = (0.1, 0.6);
    let params = FxBusParams::Duck {
        source: crate::synth::fx_params::DuckSource::Instrument(1),
        source_tap: crate::synth::fx_params::DuckSourceTap::Pre,
        threshold,
        amount,
        attack_ms: 1.0,
        release_ms: 1.0,
    };
    for (source, expected_gain) in [
        (0.05, 1.0),
        (0.1, 1.0),
        (0.15, 1.0 - amount * 0.5),
        (0.2, 1.0 - amount),
        (0.9, 1.0 - amount),
    ] {
        let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
        let mut output = 0.0;
        for _ in 0..2_000 {
            output = process_fx_bus_slot(&params, &mut state, 0.5, source, SAMPLE_RATE);
        }
        assert!(
            (output - 0.5 * expected_gain).abs() < 1.0e-4,
            "source {source}: output {output}, expected {}",
            0.5 * expected_gain
        );
    }
}
