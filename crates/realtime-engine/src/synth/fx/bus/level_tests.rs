use super::*;
use crate::synth::fx::fx_bus_state_from_params;

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

/// The duck starts dipping at half the threshold and reaches the full amount at it, at every
/// threshold: 1.0 still ducks under a full-scale source, and 0 waits for -60 dBFS.
#[test]
fn duck_reaches_the_amount_at_the_threshold_from_half_of_it() {
    let amount = 0.6;
    for (threshold, source, depth) in [
        (0.1, 0.025, 0.0),
        (0.1, 0.05, 0.0),
        (0.1, 0.075, 0.5),
        (0.1, 0.1, 1.0),
        (0.1, 0.9, 1.0),
        (0.8, 0.6, 0.5),
        (0.8, 1.0, 1.0),
        (1.0, 0.75, 0.5),
        (1.0, 1.0, 1.0),
        (0.0, 0.0004, 0.0),
        (0.0, 0.01, 1.0),
    ] {
        let params = FxBusParams::Duck {
            source: crate::synth::fx_params::DuckSource::Instrument(1),
            source_tap: crate::synth::fx_params::DuckSourceTap::Pre,
            threshold,
            amount,
            attack_ms: 1.0,
            release_ms: 1.0,
        };
        let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
        let mut output = 0.0;
        for _ in 0..2_000 {
            output = process_fx_bus_slot(&params, &mut state, 0.5, source, SAMPLE_RATE);
        }
        let expected = 0.5 * (1.0 - amount * depth);
        assert!(
            (output - expected).abs() < 1.0e-4,
            "threshold {threshold} source {source}: output {output}, expected {expected}"
        );
    }
}

#[test]
fn reverb_without_its_state_passes_the_input_through_without_allocating() {
    let params = FxBusParams::Reverb {
        mix: 1.0,
        decay: 0.8,
        damp: 0.2,
    };
    let mut state = FxBusState::None;
    let (output, allocations, _) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            process_fx_bus_slot(&params, &mut state, 0.5, 0.0, SAMPLE_RATE)
        });
    assert_eq!(output, 0.5);
    assert_eq!(allocations, 0);
    assert!(matches!(state, FxBusState::None));
}

fn shape(params: FxBusParams, input: f32) -> f32 {
    let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
    process_fx_bus_slot(&params, &mut state, input, 0.0, SAMPLE_RATE)
}

#[test]
fn distortion_drive_adds_gain_into_a_clip_at_clip() {
    let distortion = |drive, clip| FxBusParams::Distortion {
        drive,
        clip,
        mix: 1.0,
    };
    assert_eq!(shape(distortion(0.0, 0.6), 0.3), 0.3);
    assert_eq!(shape(distortion(0.0, 0.6), -0.9), -0.6);
    assert_eq!(shape(distortion(1.0, 0.6), 0.2), 0.4);
    assert_eq!(shape(distortion(1.0, 0.25), 0.2), 0.25);
    assert_eq!(shape(distortion(4.0, 1.0), 0.1), 0.5);
}

#[test]
fn saturator_drive_zero_is_clean_and_full_scale_stays_full_scale() {
    let saturator = |drive| FxBusParams::Saturator { drive, mix: 1.0 };
    assert_eq!(shape(saturator(0.0), 0.4), 0.4);
    for drive in [0.5_f32, 1.8, 20.0] {
        assert!((shape(saturator(drive), 1.0) - 1.0).abs() < 1.0e-6);
        let quiet = shape(saturator(drive), 0.001);
        let expected = 0.001 * drive / drive.tanh();
        assert!((quiet / expected - 1.0).abs() < 1.0e-3, "{drive}");
    }
}
