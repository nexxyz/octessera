use super::super::super::scalar_param::ScalarMutation;
use super::process_momentary_fx_states;
use super::{
    MomentaryFxKind, MomentaryFxRuntimeParams, MomentaryFxState, SynthEngine, PITCH_FILL_FRAMES,
};
use crate::synth::MomentaryFxTarget;
use serde_json::json;
use std::collections::BTreeMap;

const TARGET: MomentaryFxTarget = MomentaryFxTarget::Global;

#[test]
fn pitch_ramp_is_rate_scaled_and_fill_is_bit_exact() {
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut states = vec![new_pitch_with_slides(sample_rate, 100.0, 1_000.0, 10.0)];
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
        for _ in 0..PITCH_FILL_FRAMES {
            let input = (-1.0, -1.0);
            let _ = wet_state.pitch_shifter.process_frame(input.0, input.1, 1.0);
            for state in &mut states {
                let _ = process(state, sample_rate, input);
            }
        }

        let key_offsets = [1, expected_ramp / 2, expected_ramp - 1, expected_ramp];
        let mut saw_wet_difference = false;
        for offset in 0..=expected_ramp {
            let input = (1.0, 1.0);
            let ratio = pitch_ratio_for_offset(offset + 1, expected_ramp);
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
        assert_eq!(states[0][0].pitch_ramp_pos, expected_ramp as u32);
        assert_eq!(states[1][0].pitch_ramp_pos, expected_ramp as u32);
        assert_eq!(states[2][0].pitch_ramp_pos, expected_ramp as u32);
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
fn pitch_slide_is_octave_linear_and_fill_does_not_advance_it() {
    for sample_rate in [44_100, 48_000] {
        for target_octaves in [1.0, -1.0] {
            let mut states = vec![MomentaryFxState::new(
                "pitch".into(),
                MomentaryFxKind::PitchShift,
                &pitch_params_with_target(target_octaves * 12.0, 100.0, 120.0, 180.0),
                TARGET,
                sample_rate,
            )];
            for frame in 0..PITCH_FILL_FRAMES {
                let input = (0.17 + frame as f32 * 0.0001, -0.43);
                let output = process(&mut states, sample_rate, input);
                assert_eq!(output.0.to_bits(), input.0.to_bits());
                assert_eq!(output.1.to_bits(), input.1.to_bits());
            }
            assert_eq!(states[0].pitch_slide_pos, 0);
            assert_eq!(states[0].pitch_amount_octaves.to_bits(), 0.0_f32.to_bits());
            let slide_len = states[0].pitch_slide_len;
            let midpoint = slide_len / 2;
            for _ in 0..midpoint {
                let _ = process(&mut states, sample_rate, (1.0, 1.0));
            }
            assert!((states[0].pitch_amount_octaves - target_octaves * 0.5).abs() < 1.0e-6);
            for _ in midpoint..slide_len {
                let _ = process(&mut states, sample_rate, (1.0, 1.0));
            }
            assert_eq!(states[0].pitch_slide_pos, slide_len);
            assert!((states[0].pitch_amount_octaves - target_octaves).abs() < 1.0e-6);
        }
    }
}

#[test]
fn pitch_wet_activation_finishes_while_a_long_pitch_slide_continues() {
    let mut states = vec![MomentaryFxState::new(
        "pitch".into(),
        MomentaryFxKind::PitchShift,
        &pitch_params_with_target(12.0, 100.0, 1_000.0, 180.0),
        TARGET,
        44_100,
    )];
    for _ in 0..PITCH_FILL_FRAMES {
        let _ = process(&mut states, 44_100, (-1.0, -1.0));
    }
    let activation_len = states[0].pitch_ramp_len;
    let mut output = (1.0, 1.0);
    for _ in 0..=activation_len {
        output = process(&mut states, 44_100, (1.0, 1.0));
    }
    assert_eq!(states[0].pitch_ramp_pos, activation_len);
    assert!(states[0].pitch_slide_pos < states[0].pitch_slide_len);
    assert!((states[0].pitch_amount_octaves - 1.0).abs() > 0.01);
    assert!((output.0 - 1.0).abs() > 0.1);
}

#[test]
fn pitch_direct_and_prepared_complete_updates_match() {
    let initial = pitch_params_with_target(12.0, 100.0, 1_000.0, 180.0);
    let complete = BTreeMap::from([
        ("semitones".into(), json!(-5.0)),
        ("cents".into(), json!(25.0)),
        ("mixPct".into(), json!(65.0)),
        ("slideInMs".into(), json!(120.0)),
        ("slideOutMs".into(), json!(180.0)),
    ]);
    let mut direct = SynthEngine::new(44_100);
    let mut prepared = SynthEngine::new(44_100);
    for engine in [&mut direct, &mut prepared] {
        engine.momentary_fx_start(
            "pitch".into(),
            "pitch_shift".into(),
            initial.clone(),
            TARGET,
        );
    }
    direct.momentary_fx_update("pitch", &complete);
    let epoch = prepared.momentary_fx[0].epoch;
    let update = super::super::prepared_control_prepare::prepare_momentary_fx_update(
        epoch,
        "pitch_shift".into(),
        complete,
        44_100,
    )
    .unwrap();
    assert_eq!(
        prepared.apply_prepared_momentary_fx_update(update),
        ScalarMutation::Changed
    );
    assert_eq!(
        pitch_runtime_snapshot(&direct.momentary_fx[0]),
        pitch_runtime_snapshot(&prepared.momentary_fx[0])
    );
    assert_eq!(
        direct.momentary_fx[0].pitch_slide_target_octaves.to_bits(),
        ((-5.0 + 0.25) / 12.0_f32).to_bits()
    );
}

#[test]
fn pitch_update_omissions_use_canonical_defaults() {
    let initial = pitch_params_with_target(-12.0, 25.0, 500.0, 700.0);
    let empty = BTreeMap::new();
    let mut direct = SynthEngine::new(44_100);
    let mut prepared = SynthEngine::new(44_100);
    for engine in [&mut direct, &mut prepared] {
        engine.momentary_fx_start(
            "pitch".into(),
            "pitch_shift".into(),
            initial.clone(),
            TARGET,
        );
    }
    direct.momentary_fx_update("pitch", &empty);
    let epoch = prepared.momentary_fx[0].epoch;
    let update = super::super::prepared_control_prepare::prepare_momentary_fx_update(
        epoch,
        "pitch_shift".into(),
        empty.clone(),
        44_100,
    )
    .unwrap();
    assert_eq!(
        prepared.apply_prepared_momentary_fx_update(update),
        ScalarMutation::Changed
    );
    let expected = pitch_runtime_snapshot_params(&empty, 44_100);
    assert_eq!(pitch_runtime_snapshot(&direct.momentary_fx[0]), expected);
    assert_eq!(pitch_runtime_snapshot(&prepared.momentary_fx[0]), expected);
}

#[test]
fn pitch_phase_continuous_activation_has_no_transient_click() {
    const MAX_INPUT_DELTA: f32 = 0.04;
    const MAX_OUTPUT_DELTA: f32 = 0.1;
    for &(sample_rate, expected_ramp) in &[(44_100, 441), (48_000, 480)] {
        let mut states = vec![new_pitch_with_slides(sample_rate, 100.0, 10.0, 10.0)];
        let mut previous_input: Option<(f32, f32)> = None;
        let mut previous_output: Option<(f32, f32)> = None;
        let mut maximum_input_delta = 0.0_f32;
        let mut maximum_output_delta = 0.0_f32;
        let mut maximum_effect_difference = 0.0_f32;
        for frame in 0..=PITCH_FILL_FRAMES + expected_ramp {
            let input = continuous_test_signal(frame);
            let output = process(&mut states, sample_rate, input);
            if let Some(previous) = previous_input {
                maximum_input_delta = maximum_input_delta
                    .max((input.0 - previous.0).abs())
                    .max((input.1 - previous.1).abs());
            }
            if frame >= PITCH_FILL_FRAMES {
                if let Some(previous) = previous_output {
                    maximum_output_delta = maximum_output_delta
                        .max((output.0 - previous.0).abs())
                        .max((output.1 - previous.1).abs());
                }
            }
            if frame >= PITCH_FILL_FRAMES + expected_ramp {
                maximum_effect_difference = maximum_effect_difference
                    .max((output.0 - input.0).abs())
                    .max((output.1 - input.1).abs());
            }
            previous_input = Some(input);
            previous_output = Some(output);
        }
        assert!(maximum_input_delta < MAX_INPUT_DELTA);
        assert!(
            maximum_output_delta < MAX_OUTPUT_DELTA,
            "sample_rate={sample_rate} maximum_output_delta={maximum_output_delta}"
        );
        assert!(maximum_effect_difference > 0.1);
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
        let slide_pos = engine.momentary_fx[0].pitch_slide_pos;
        engine.momentary_fx_update("pitch", &pitch_params_with_target(7.0, 50.0, 10.0, 10.0));
        assert_eq!(engine.momentary_fx[0].pitch_slide_pos, slide_pos);
        engine.momentary_fx_update("pitch", &pitch_params_with_target(7.0, 50.0, 10.0, 30.0));
        assert_eq!(engine.momentary_fx[0].pitch_slide_pos, slide_pos);
        engine.momentary_fx_update("pitch", &pitch_params_with_target(5.0, 50.0, 120.0, 180.0));
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
    new_pitch_with_slides(sample_rate, mix_pct, 10.0, 10.0)
}

fn new_pitch_with_slides(
    sample_rate: u32,
    mix_pct: f32,
    slide_in_ms: f32,
    slide_out_ms: f32,
) -> MomentaryFxState {
    MomentaryFxState::new(
        "pitch".to_string(),
        MomentaryFxKind::PitchShift,
        &pitch_params_with_slides(mix_pct, slide_in_ms, slide_out_ms),
        TARGET,
        sample_rate,
    )
}

fn pitch_params(mix_pct: f32) -> BTreeMap<String, serde_json::Value> {
    pitch_params_with_slides(mix_pct, 10.0, 10.0)
}

fn pitch_params_with_slides(
    mix_pct: f32,
    slide_in_ms: f32,
    slide_out_ms: f32,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("semitones".to_string(), json!(7.0)),
        ("cents".to_string(), json!(0.0)),
        ("slideInMs".to_string(), json!(slide_in_ms)),
        ("slideOutMs".to_string(), json!(slide_out_ms)),
        ("mixPct".to_string(), json!(mix_pct)),
    ])
}

fn pitch_params_with_target(
    semitones: f32,
    mix_pct: f32,
    slide_in_ms: f32,
    slide_out_ms: f32,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("semitones".to_string(), json!(semitones)),
        ("cents".to_string(), json!(0.0)),
        ("slideInMs".to_string(), json!(slide_in_ms)),
        ("slideOutMs".to_string(), json!(slide_out_ms)),
        ("mixPct".to_string(), json!(mix_pct)),
    ])
}

fn pitch_ratio_for_offset(offset: usize, slide_len: usize) -> f32 {
    2.0_f32.powf(7.0 / 12.0 * (offset.min(slide_len) as f32 / slide_len as f32))
}

fn continuous_test_signal(frame: u32) -> (f32, f32) {
    let phase = frame as f32 * 0.07;
    (0.45 * phase.sin(), 0.35 * (phase + 0.8).sin())
}

fn pitch_runtime_snapshot(fx: &MomentaryFxState) -> (u32, u32, u32, u32) {
    match fx.runtime_params {
        MomentaryFxRuntimeParams::PitchShift {
            target_octaves,
            mix,
            slide_in_len,
            slide_out_len,
        } => (
            target_octaves.to_bits(),
            mix.to_bits(),
            slide_in_len,
            slide_out_len,
        ),
        _ => unreachable!("pitch test state has non-pitch runtime parameters"),
    }
}

fn pitch_runtime_snapshot_params(
    params: &BTreeMap<String, serde_json::Value>,
    sample_rate: u32,
) -> (u32, u32, u32, u32) {
    let runtime =
        MomentaryFxRuntimeParams::from_params(MomentaryFxKind::PitchShift, params, sample_rate);
    match runtime {
        MomentaryFxRuntimeParams::PitchShift {
            target_octaves,
            mix,
            slide_in_len,
            slide_out_len,
        } => (
            target_octaves.to_bits(),
            mix.to_bits(),
            slide_in_len,
            slide_out_len,
        ),
        _ => unreachable!("pitch test params have non-pitch runtime parameters"),
    }
}
