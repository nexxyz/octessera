use super::super::super::scalar_param::ScalarMutation;
use super::super::prepared_control_prepare::PreparedMomentaryFxUpdate;
use super::{MomentaryFxKind, MomentaryFxState, SynthEngine, PITCH_FILL_FRAMES};
use crate::synth::MomentaryFxTarget;
use serde_json::json;
use std::collections::BTreeMap;

const GLOBAL: MomentaryFxTarget = MomentaryFxTarget::Global;
const RATES: [(u32, u32); 2] = [(44_100, 441), (48_000, 480)];

#[test]
fn pitch_release_uses_full_rate_scaled_complementary_fade() {
    for &(sample_rate, release_len) in &RATES {
        for mix in [50.0, 100.0] {
            let mut engine = SynthEngine::new(sample_rate);
            let mut wet_state = new_pitch(sample_rate, 100.0);
            engine.momentary_fx_start(
                "pitch".into(),
                "pitch_shift".into(),
                pitch_params(mix),
                GLOBAL,
            );
            render_full_activation(&mut engine, &mut wet_state, release_len);
            let retired = engine.momentary_fx_stop("pitch");
            assert!(retired.displaced_momentary_fx.iter().all(Option::is_none));
            assert_eq!(engine.momentary_fx[0].release_len, release_len);
            let release_start_octaves = engine.momentary_fx[0].pitch_amount_octaves;

            for release_pos in 0..release_len {
                let input = (0.35, -0.2);
                let ratio =
                    pitch_ratio_for_release(release_start_octaves, release_pos, release_len);
                let wet = wet_state
                    .pitch_shifter
                    .process_frame(input.0, input.1, ratio);
                let output = engine.process_momentary_fx_target(GLOBAL, input.0, input.1);
                if release_pos == 0
                    || release_pos == release_len / 2
                    || release_pos == release_len - 2
                    || release_pos == release_len - 1
                {
                    let wet_gain = mix / 100.0 * (1.0 - release_pos as f32 / release_len as f32);
                    if release_pos + 1 < release_len {
                        assert_eq!(engine.momentary_fx[0].release_pos, release_pos + 1);
                    } else {
                        assert!(engine.momentary_fx.is_empty());
                    }
                    assert!(
                        (output.0 - (input.0 * (1.0 - wet_gain) + wet.0 * wet_gain)).abs() < 1e-6
                    );
                    assert!(
                        (output.1 - (input.1 * (1.0 - wet_gain) + wet.1 * wet_gain)).abs() < 1e-6
                    );
                }
            }
            assert!(engine.momentary_fx.is_empty());
            assert!(!engine.pending_render_retired_is_empty());
            let post = engine.process_momentary_fx_target(GLOBAL, 0.37, -0.41);
            assert_eq!(post.0.to_bits(), 0.37_f32.to_bits());
            assert_eq!(post.1.to_bits(), (-0.41_f32).to_bits());
        }
    }
}

#[test]
fn pitch_partial_activation_release_starts_at_current_wet_and_only_declines() {
    let sample_rate = 48_000;
    let release_len = 480;
    let activation_frames = release_len / 2;
    let mut engine = SynthEngine::new(sample_rate);
    let mut wet_state = new_pitch(sample_rate, 100.0);
    engine.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(100.0),
        GLOBAL,
    );
    for frame in 0..PITCH_FILL_FRAMES + activation_frames {
        let input = if frame < PITCH_FILL_FRAMES {
            (-1.0, -1.0)
        } else {
            (1.0, 1.0)
        };
        let ratio = if frame < PITCH_FILL_FRAMES {
            1.0
        } else {
            pitch_ratio_for_slide(frame - PITCH_FILL_FRAMES + 1, release_len)
        };
        let _ = wet_state
            .pitch_shifter
            .process_frame(input.0, input.1, ratio);
        let _ = engine.process_momentary_fx_target(GLOBAL, input.0, input.1);
    }
    let activation =
        engine.momentary_fx[0].pitch_ramp_pos as f32 / engine.momentary_fx[0].pitch_ramp_len as f32;
    assert!(activation > 0.0 && activation < 1.0);
    engine.momentary_fx_stop("pitch");
    assert_eq!(engine.momentary_fx[0].release_pos, 0);

    let mut previous_gain = activation;
    let release_start_octaves = engine.momentary_fx[0].pitch_amount_octaves;
    for release_pos in 0..release_len {
        let input = (1.0, 1.0);
        let ratio = pitch_ratio_for_release(release_start_octaves, release_pos, release_len);
        let wet = wet_state
            .pitch_shifter
            .process_frame(input.0, input.1, ratio);
        let output = engine.process_momentary_fx_target(GLOBAL, input.0, input.1);
        let wet_gain = activation * (1.0 - release_pos as f32 / release_len as f32);
        assert!(wet_gain <= previous_gain);
        assert!((output.0 - (input.0 * (1.0 - wet_gain) + wet.0 * wet_gain)).abs() < 1e-6);
        previous_gain = wet_gain;
    }
    assert!(engine.momentary_fx.is_empty());
}

#[test]
fn pitch_slide_out_holds_wet_until_the_final_fixed_activation_window() {
    let sample_rate = 44_100;
    let release_len = 1_323;
    let mut engine = SynthEngine::new(sample_rate);
    let mut wet_state = new_pitch_with_slides(sample_rate, 100.0, 10.0, 30.0);
    engine.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params_with_slides(100.0, 10.0, 30.0),
        GLOBAL,
    );
    render_full_activation(&mut engine, &mut wet_state, release_len);
    engine.momentary_fx_stop("pitch");
    assert_eq!(engine.momentary_fx[0].release_len, release_len);
    let start_octaves = engine.momentary_fx[0].pitch_amount_octaves;
    let fixed_len = engine.momentary_fx[0].pitch_ramp_len.max(1);
    let input = (0.35, -0.2);
    for release_pos in 0..release_len {
        let ratio = pitch_ratio_for_release(start_octaves, release_pos, release_len);
        let wet = wet_state
            .pitch_shifter
            .process_frame(input.0, input.1, ratio);
        let output = engine.process_momentary_fx_target(GLOBAL, input.0, input.1);
        let fade_start = release_len - fixed_len;
        let fade = if release_pos < fade_start {
            1.0
        } else {
            (release_len - release_pos) as f32 / fixed_len as f32
        };
        assert!((output.0 - (input.0 * (1.0 - fade) + wet.0 * fade)).abs() < 1.0e-6);
    }
    assert!(engine.momentary_fx.is_empty());
}

#[test]
fn pitch_fill_and_zero_mix_stop_immediately() {
    let mut filling = SynthEngine::new(48_000);
    filling.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(100.0),
        GLOBAL,
    );
    let retired = filling.momentary_fx_stop("pitch");
    assert!(filling.momentary_fx.is_empty());
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);

    let mut zero_mix = SynthEngine::new(48_000);
    let mut wet_state = new_pitch(48_000, 100.0);
    zero_mix.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(0.0),
        GLOBAL,
    );
    render_full_activation(&mut zero_mix, &mut wet_state, 480);
    let retired = zero_mix.momentary_fx_stop("pitch");
    assert!(zero_mix.momentary_fx.is_empty());
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);
}

#[test]
fn pitch_repeated_stop_does_not_reset_release_and_releasing_updates_are_rejected() {
    let sample_rate = 48_000;
    let mut engine = SynthEngine::new(sample_rate);
    let mut wet_state = new_pitch(sample_rate, 100.0);
    engine.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(100.0),
        GLOBAL,
    );
    render_full_activation(&mut engine, &mut wet_state, 480);
    engine.momentary_fx_stop("pitch");
    let release_len = engine.momentary_fx[0].release_len;
    let _ = engine.process_momentary_fx_target(GLOBAL, 0.2, -0.1);
    let release_pos = engine.momentary_fx[0].release_pos;
    engine.momentary_fx_stop("pitch");
    assert_eq!(engine.momentary_fx[0].release_pos, release_pos);
    assert_eq!(engine.momentary_fx[0].release_len, release_len);

    engine.momentary_fx_update("pitch", &pitch_params(50.0));
    assert_eq!(pitch_mix(&engine.momentary_fx[0]), 1.0);
    let epoch = engine.momentary_fx[0].epoch;
    assert_eq!(
        engine.apply_prepared_momentary_fx_update(PreparedMomentaryFxUpdate::PitchShift {
            epoch,
            target_octaves: 0.0,
            mix: 0.25,
            slide_in_len: 120,
            slide_out_len: 180,
        }),
        ScalarMutation::Rejected
    );
    assert_eq!(pitch_mix(&engine.momentary_fx[0]), 1.0);
}

#[test]
fn pitch_release_respects_target_isolation_and_retrigger_resets() {
    let target = MomentaryFxTarget::Instrument { index: 0 };
    let sample_rate = 48_000;
    let mut engine = SynthEngine::new(sample_rate);
    let mut wet_state = new_pitch(sample_rate, 100.0);
    engine.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(100.0),
        target,
    );
    render_full_activation_for_target(&mut engine, &mut wet_state, target, 480);
    engine.momentary_fx_stop("pitch");
    let _ = engine.process_momentary_fx_target(GLOBAL, 0.2, -0.1);
    assert_eq!(engine.momentary_fx[0].release_pos, 0);
    let _ = engine.process_momentary_fx_target(target, 0.2, -0.1);
    assert_eq!(engine.momentary_fx[0].release_pos, 1);

    engine.momentary_fx_start(
        "pitch".into(),
        "pitch_shift".into(),
        pitch_params(100.0),
        target,
    );
    assert!(!engine.momentary_fx[0].releasing);
    assert_eq!(engine.momentary_fx[0].pitch_fill_pos, 0);
    assert_eq!(engine.momentary_fx[0].pitch_ramp_pos, 0);
    assert_eq!(engine.momentary_fx[0].release_pos, 0);
}

fn render_full_activation(
    engine: &mut SynthEngine,
    wet_state: &mut MomentaryFxState,
    release_len: u32,
) {
    render_full_activation_for_target(engine, wet_state, GLOBAL, release_len);
}

fn render_full_activation_for_target(
    engine: &mut SynthEngine,
    wet_state: &mut MomentaryFxState,
    target: MomentaryFxTarget,
    release_len: u32,
) {
    let slide_in_len = engine.momentary_fx[0].pitch_slide_len;
    for frame in 0..PITCH_FILL_FRAMES + release_len + 1 {
        let input = if frame < PITCH_FILL_FRAMES {
            (-1.0, -1.0)
        } else {
            (1.0, 1.0)
        };
        let ratio = if frame < PITCH_FILL_FRAMES {
            1.0
        } else {
            pitch_ratio_for_slide(frame - PITCH_FILL_FRAMES + 1, slide_in_len)
        };
        let _ = wet_state
            .pitch_shifter
            .process_frame(input.0, input.1, ratio);
        let _ = engine.process_momentary_fx_target(target, input.0, input.1);
    }
}

fn new_pitch(sample_rate: u32, mix: f32) -> MomentaryFxState {
    new_pitch_with_slides(sample_rate, mix, 10.0, 10.0)
}

fn new_pitch_with_slides(
    sample_rate: u32,
    mix: f32,
    slide_in_ms: f32,
    slide_out_ms: f32,
) -> MomentaryFxState {
    MomentaryFxState::new(
        "pitch".into(),
        MomentaryFxKind::PitchShift,
        &pitch_params_with_slides(mix, slide_in_ms, slide_out_ms),
        GLOBAL,
        sample_rate,
    )
}

fn pitch_params(mix: f32) -> BTreeMap<String, serde_json::Value> {
    pitch_params_with_slides(mix, 10.0, 10.0)
}

fn pitch_params_with_slides(
    mix: f32,
    slide_in_ms: f32,
    slide_out_ms: f32,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("semitones".into(), json!(7.0)),
        ("slideInMs".into(), json!(slide_in_ms)),
        ("slideOutMs".into(), json!(slide_out_ms)),
        ("mixPct".into(), json!(mix)),
    ])
}

fn pitch_mix(fx: &MomentaryFxState) -> f32 {
    match fx.runtime_params {
        super::MomentaryFxRuntimeParams::PitchShift { mix, .. } => mix,
        _ => unreachable!("pitch test state has non-pitch runtime parameters"),
    }
}

fn pitch_ratio_for_slide(position: u32, slide_len: u32) -> f32 {
    pitch_ratio_for_amount(7.0 / 12.0 * (position.min(slide_len) as f32 / slide_len as f32))
}

fn pitch_ratio_for_release(start_octaves: f32, position: u32, release_len: u32) -> f32 {
    let position = position.saturating_add(1).min(release_len);
    pitch_ratio_for_amount(start_octaves * (1.0 - position as f32 / release_len as f32))
}

fn pitch_ratio_for_amount(octaves: f32) -> f32 {
    2.0_f32.powf(octaves)
}
