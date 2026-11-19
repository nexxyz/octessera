use super::super::pan_law::pan_gains;
use super::super::SynthEngine;
use crate::synth::types::{DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT};
use crate::synth::ScalarMutation;

const RATE: u32 = 44_100;
const RAMP_FRAMES: usize = 221;

fn frame(engine: &mut SynthEngine, slot_level: f32, preview: f32) -> (f32, f32) {
    let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
    slot_out[0] = slot_level;
    engine.finish_serial_frame::<false>(slot_out, true, preview, true, false, 0)
}

fn assert_monotonic_ramp(outputs: &[f32], from: f32, to: f32) {
    let falling = to < from;
    for (index, pair) in outputs.windows(2).enumerate() {
        let moved_wrong_way = if falling {
            pair[1] > pair[0]
        } else {
            pair[1] < pair[0]
        };
        assert!(
            !moved_wrong_way,
            "frame {index}: {} then {}",
            pair[0], pair[1]
        );
    }
    assert!(
        (outputs[0] - from).abs() > 0.0,
        "the first frame already moves"
    );
    assert!(outputs[RAMP_FRAMES - 2] != to, "the ramp is not a jump");
    assert!(outputs[RAMP_FRAMES - 1..].iter().all(|value| *value == to));
}

#[test]
fn a_master_volume_step_ramps_linearly_over_five_ms() {
    let mut engine = SynthEngine::new(RATE);
    assert_eq!(frame(&mut engine, 0.0, 0.8).0, 0.8);
    assert_eq!(engine.set_master_volume(25.0), ScalarMutation::Changed);
    let outputs: Vec<f32> = (0..400).map(|_| frame(&mut engine, 0.0, 0.8).0).collect();

    assert_monotonic_ramp(&outputs, 0.8, 0.8 * 0.25);
    for (index, output) in outputs[..RAMP_FRAMES].iter().enumerate() {
        let expected = 0.8 * (1.0 - 0.75 * (index + 1) as f32 / RAMP_FRAMES as f32);
        assert!((output - expected).abs() < 1.0e-5, "frame {index}");
    }
    assert_eq!(engine.set_master_volume(25.0), ScalarMutation::Unchanged);
}

#[test]
fn an_instrument_volume_and_pan_step_ramp_to_the_new_gains() {
    let mut engine = SynthEngine::new(RATE);
    let (left_before, right_before) = frame(&mut engine, 0.5, 0.0);
    assert_eq!(
        engine.set_instrument_mixer(0, Some(40.0), Some(0)),
        ScalarMutation::Changed
    );
    let outputs: Vec<(f32, f32)> = (0..400).map(|_| frame(&mut engine, 0.5, 0.0)).collect();
    let (gain_l, gain_r) = pan_gains(0, DEFAULT_PAN_POSITIONS);
    let left: Vec<f32> = outputs.iter().map(|output| output.0).collect();
    let right: Vec<f32> = outputs.iter().map(|output| output.1).collect();

    assert_monotonic_ramp(&left, left_before, 0.5 * 0.4 * gain_l);
    assert_monotonic_ramp(&right, right_before, 0.5 * 0.4 * gain_r);
}

#[test]
fn a_retarget_mid_ramp_keeps_moving_from_where_the_gain_is() {
    let mut engine = SynthEngine::new(RATE);
    engine.set_master_volume(0.0);
    let falling: Vec<f32> = (0..100).map(|_| frame(&mut engine, 0.0, 1.0).0).collect();
    engine.set_master_volume(100.0);
    let rising: Vec<f32> = (0..400).map(|_| frame(&mut engine, 0.0, 1.0).0).collect();

    let fall_step = 1.0 / RAMP_FRAMES as f32;
    let pending = falling[falling.len() - 1] - fall_step;
    assert!((rising[0] - pending).abs() < 1.0e-5);
    for (index, pair) in rising.windows(2).enumerate() {
        assert!(
            pair[1] >= pair[0],
            "frame {index}: {} then {}",
            pair[0],
            pair[1]
        );
    }
    assert!(rising[RAMP_FRAMES - 1] < 1.0);
    assert!(rising[RAMP_FRAMES..].iter().all(|value| *value == 1.0));
}

#[test]
fn unchanged_mixer_settings_leave_the_output_untouched() {
    let mut steady = SynthEngine::new(RATE);
    let mut touched = SynthEngine::new(RATE);
    let pan = DEFAULT_PAN_POSITIONS / 2;
    assert_eq!(touched.set_master_volume(100.0), ScalarMutation::Unchanged);
    assert_eq!(
        touched.set_instrument_mixer(0, Some(100.0), Some(pan)),
        ScalarMutation::Unchanged
    );
    assert!(!touched.mixer_gain_ramps.master_active);
    assert!(!touched.mixer_gain_ramps.mix_active());
    for index in 0..300 {
        let level = (index as f32 * 0.05).sin() * 0.7;
        let expected = frame(&mut steady, level, 0.1);
        let actual = frame(&mut touched, level, 0.1);
        assert_eq!(actual.0.to_bits(), expected.0.to_bits());
        assert_eq!(actual.1.to_bits(), expected.1.to_bits());
    }
}

#[test]
fn a_config_load_lands_running_ramps_and_snaps_its_own_gains() {
    let mut engine = SynthEngine::new(RATE);
    engine.set_master_volume(10.0);
    engine.set_instrument_mixer(0, Some(10.0), Some(0));
    frame(&mut engine, 0.5, 0.0);
    engine.set_instruments(crate::synth::InstrumentsConfig {
        instruments: Vec::new(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 60.0,
    });

    assert_eq!(engine.master_volume, 0.6);
    assert_eq!(engine.slot_volume[0], 0.1);
    assert!(!engine.mixer_gain_ramps.master_active);
    assert!(!engine.mixer_gain_ramps.mix_active());
}
