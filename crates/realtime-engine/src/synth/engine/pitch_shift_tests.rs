use super::process_momentary_fx_states;
use super::{MomentaryFxKind, MomentaryFxState, SynthEngine, PITCH_FILL_FRAMES};
use crate::synth::MomentaryFxTarget;
use serde_json::json;
use std::collections::BTreeMap;

const TARGET: MomentaryFxTarget = MomentaryFxTarget::Global;

#[test]
fn pitch_ramp_is_rate_scaled_and_fill_is_bit_exact() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut states = vec![new_pitch(sample_rate, 100.0)];
        for frame in 0..PITCH_FILL_FRAMES {
            let input = (0.17 + frame as f32 * 0.0001, -0.43);
            let output = process(&mut states, sample_rate, input);
            assert_eq!(output.0.to_bits(), input.0.to_bits(), "left frame {frame}");
            assert_eq!(output.1.to_bits(), input.1.to_bits(), "right frame {frame}");
        }
        assert_eq!(states[0].pitch_fill_pos, PITCH_FILL_FRAMES);
        assert_eq!(states[0].pitch_ramp_len, expected_ramp);
        let input = (0.31, -0.27);
        let output = process(&mut states, sample_rate, input);
        assert_eq!(output.0.to_bits(), input.0.to_bits());
        assert_eq!(output.1.to_bits(), input.1.to_bits());
        assert_eq!(states[0].pitch_ramp_pos, 1);
    }
}

#[test]
fn pitch_ramp_uses_complementary_gain_at_key_phases_for_all_mixes() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut states = vec![
            vec![new_pitch(sample_rate, 0.0)],
            vec![new_pitch(sample_rate, 50.0)],
            vec![new_pitch(sample_rate, 100.0)],
        ];
        let mut wet_state = new_pitch(sample_rate, 100.0);
        let ratio = pitch_ratio();
        for _ in 0..PITCH_FILL_FRAMES {
            let input = (-1.0, -1.0);
            let _ = wet_state
                .pitch_shifter
                .process_frame(input.0, input.1, ratio);
            for state in &mut states {
                let _ = process(state, sample_rate, input);
            }
        }

        let key_offsets = [1, expected_ramp / 2, expected_ramp - 1, expected_ramp];
        let mut saw_wet_difference = false;
        for offset in 0..=expected_ramp {
            let input = (1.0, 1.0);
            let wet = wet_state
                .pitch_shifter
                .process_frame(input.0, input.1, ratio);
            saw_wet_difference |= (wet.0 - input.0).abs() > 0.1;
            for (state, mix_pct) in states.iter_mut().zip([0.0, 0.5, 1.0]) {
                let output = process(state, sample_rate, input);
                if key_offsets.contains(&offset) {
                    let gain = offset as f32 / expected_ramp as f32;
                    let effective_mix = mix_pct * gain;
                    let expected_left = input.0 * (1.0 - effective_mix) + wet.0 * effective_mix;
                    let expected_right = input.1 * (1.0 - effective_mix) + wet.1 * effective_mix;
                    assert!((output.0 - expected_left).abs() < 1.0e-6);
                    assert!((output.1 - expected_right).abs() < 1.0e-6);
                }
            }
        }
        assert!(saw_wet_difference);
        assert_eq!(states[0][0].pitch_ramp_pos, expected_ramp);
        assert_eq!(states[1][0].pitch_ramp_pos, expected_ramp);
        assert_eq!(states[2][0].pitch_ramp_pos, expected_ramp);
    }
}

#[test]
fn pitch_same_phase_unity_stays_unity() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        for mix_pct in [0.0, 50.0, 100.0] {
            let mut states = vec![new_pitch(sample_rate, mix_pct)];
            for frame in 0..(PITCH_FILL_FRAMES + expected_ramp + 1) {
                let output = process(&mut states, sample_rate, (1.0, 1.0));
                if frame >= PITCH_FILL_FRAMES {
                    assert!((output.0 - 1.0).abs() < 1.0e-6);
                    assert!((output.1 - 1.0).abs() < 1.0e-6);
                }
            }
        }
    }
}

#[test]
fn pitch_opposite_phase_transition_is_bounded_over_the_full_ramp() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut states = vec![new_pitch(sample_rate, 100.0)];
        for _ in 0..PITCH_FILL_FRAMES {
            let _ = process(&mut states, sample_rate, (-1.0, -1.0));
        }
        let mut previous = 1.0_f32;
        let mut maximum_step = 0.0_f32;
        for _ in 0..=expected_ramp {
            let output = process(&mut states, sample_rate, (1.0, 1.0));
            maximum_step = maximum_step.max((output.0 - previous).abs());
            previous = output.0;
        }
        assert!(
            maximum_step < 0.1,
            "sample_rate={sample_rate} ramp={expected_ramp} maximum_step={maximum_step} final={previous}"
        );
        assert!(previous < -0.9);
    }
}

#[test]
fn pitch_update_preserves_progress_and_retrigger_resets() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut engine = SynthEngine::new(sample_rate);
        engine.momentary_fx_start(
            "pitch".to_string(),
            "pitch_shift".to_string(),
            pitch_params(100.0),
            TARGET,
        );
        for frame in 0..(PITCH_FILL_FRAMES + 16) {
            let _ = engine.process_momentary_fx_target(TARGET, frame as f32 * 0.001, -0.2);
        }
        assert_eq!(engine.momentary_fx[0].pitch_ramp_len, expected_ramp);
        assert_eq!(engine.momentary_fx[0].pitch_ramp_pos, 16);
        let fill_pos = engine.momentary_fx[0].pitch_fill_pos;
        let ramp_pos = engine.momentary_fx[0].pitch_ramp_pos;
        engine.momentary_fx_update(
            "pitch",
            &BTreeMap::from([
                ("semitones".to_string(), json!(5.0)),
                ("mixPct".to_string(), json!(50.0)),
            ]),
        );
        assert_eq!(engine.momentary_fx[0].pitch_fill_pos, fill_pos);
        assert_eq!(engine.momentary_fx[0].pitch_ramp_pos, ramp_pos);
        engine.momentary_fx_start(
            "pitch".to_string(),
            "pitch_shift".to_string(),
            pitch_params(100.0),
            TARGET,
        );
        assert_eq!(engine.momentary_fx[0].pitch_fill_pos, 0);
        assert_eq!(engine.momentary_fx[0].pitch_ramp_pos, 0);
        assert_eq!(engine.momentary_fx[0].pitch_shifter.write_pos, 0);
    }
}

fn process(states: &mut Vec<MomentaryFxState>, sample_rate: u32, input: (f32, f32)) -> (f32, f32) {
    process_momentary_fx_states(states, TARGET, input.0, input.1, None, sample_rate)
}

fn new_pitch(sample_rate: u32, mix_pct: f32) -> MomentaryFxState {
    MomentaryFxState::new(
        "pitch".to_string(),
        MomentaryFxKind::PitchShift,
        &pitch_params(mix_pct),
        TARGET,
        sample_rate,
    )
}

fn pitch_params(mix_pct: f32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("semitones".to_string(), json!(7.0)),
        ("mixPct".to_string(), json!(mix_pct)),
    ])
}

fn pitch_ratio() -> f32 {
    2.0_f32.powf(7.0 / 12.0)
}
