use super::render_momentary_fx::process_momentary_fx_states;
use super::{MomentaryFxKind, MomentaryFxState, SynthEngine, PITCH_FILL_FRAMES};
use crate::synth::MomentaryFxTarget;
use serde_json::json;
use std::collections::BTreeMap;

const SAMPLE_RATE: u32 = 48_000;
const RENDER_FRAMES: usize = 3_000;

#[test]
fn freeze_startup_passes_stage_input_until_all_taps_are_ready() {
    let target = MomentaryFxTarget::Global;
    for &(sample_rate, expected_ready, expected_ramp) in
        &[(44_100, 1_617, 220), (48_000, 1_760, 240)]
    {
        let mut states = vec![MomentaryFxState::new(
            "freeze".to_string(),
            MomentaryFxKind::Freeze,
            &freeze_params(100.0),
            target,
            sample_rate,
        )];
        assert_eq!(states[0].freeze_ready_len, expected_ready);
        assert_eq!(states[0].freeze_activation_len, expected_ramp);

        for frame in 0..expected_ready as usize {
            let input = (0.25 + frame as f32 * 0.0001, -0.5);
            let output = process_momentary_fx_states(
                &mut states,
                target,
                input.0,
                input.1,
                None,
                sample_rate,
            );
            assert_eq!(output.0.to_bits(), input.0.to_bits(), "left frame {frame}");
            assert_eq!(output.1.to_bits(), input.1.to_bits(), "right frame {frame}");
            assert!(output.0 != 0.0 || output.1 != 0.0, "silent frame {frame}");
        }
        assert_eq!(states[0].freeze_inject_pos, expected_ready);
    }
}

#[test]
fn freeze_activation_uses_shared_ramp_and_requested_mix() {
    let target = MomentaryFxTarget::Global;
    for &(sample_rate, ready, ramp) in &[(44_100, 1_617_usize, 220_usize), (48_000, 1_760, 240)] {
        let mut full = vec![MomentaryFxState::new(
            "full".to_string(),
            MomentaryFxKind::Freeze,
            &freeze_params(100.0),
            target,
            sample_rate,
        )];
        let mut half = vec![MomentaryFxState::new(
            "half".to_string(),
            MomentaryFxKind::Freeze,
            &freeze_params(50.0),
            target,
            sample_rate,
        )];

        for frame in 0..=ready + ramp {
            let input = (0.2 + (frame % 17) as f32 * 0.01, -0.35);
            let full_output =
                process_momentary_fx_states(&mut full, target, input.0, input.1, None, sample_rate);
            let half_output =
                process_momentary_fx_states(&mut half, target, input.0, input.1, None, sample_rate);
            if frame == ready {
                assert_eq!(
                    full_output.0.to_bits(),
                    input.0.to_bits(),
                    "rate {sample_rate} frame {frame} ready {ready}"
                );
                assert_eq!(
                    full_output.1.to_bits(),
                    input.1.to_bits(),
                    "rate {sample_rate} frame {frame} ready {ready}"
                );
                assert_eq!(full[0].freeze_activation_pos, 1);
            }
            if frame == ready + ramp / 2 || frame == ready + ramp {
                assert!((half_output.0 - (input.0 + (full_output.0 - input.0) * 0.5)).abs() < 1e-6);
                assert!((half_output.1 - (input.1 + (full_output.1 - input.1) * 0.5)).abs() < 1e-6);
            }
            if frame == ready + ramp / 2 {
                assert_eq!(full[0].freeze_activation_pos, (ramp / 2 + 1) as u32);
            }
            if frame == ready + ramp {
                assert_eq!(full[0].freeze_activation_pos, ramp as u32);
            }
        }
    }
}

#[test]
fn freeze_release_during_startup_does_not_advance_or_expose_silence() {
    let target = MomentaryFxTarget::Global;
    let mut states = vec![new_fx("freeze", MomentaryFxKind::Freeze, target)];
    for _ in 0..100 {
        let output = process_momentary_fx_states(&mut states, target, 0.4, -0.2, None, SAMPLE_RATE);
        assert_eq!(output.0.to_bits(), 0.4_f32.to_bits());
        assert_eq!(output.1.to_bits(), (-0.2_f32).to_bits());
    }
    states[0].releasing = true;
    states[0].release_len = 32;
    states[0].release_pos = 0;
    for _ in 0..8 {
        let output = process_momentary_fx_states(&mut states, target, 0.4, -0.2, None, SAMPLE_RATE);
        assert_eq!(output.0.to_bits(), 0.4_f32.to_bits());
        assert_eq!(output.1.to_bits(), (-0.2_f32).to_bits());
        assert_eq!(states[0].freeze_activation_pos, 0);
    }

    let mut states = vec![new_fx("freeze", MomentaryFxKind::Freeze, target)];
    for frame in 0..(states[0].freeze_ready_len as usize + 4) {
        let input = (0.4, -0.2);
        let _ =
            process_momentary_fx_states(&mut states, target, input.0, input.1, None, SAMPLE_RATE);
        if frame == states[0].freeze_ready_len as usize + 2 {
            states[0].releasing = true;
            states[0].release_len = 32;
            states[0].release_pos = 0;
            let activation_pos = states[0].freeze_activation_pos;
            let mut previous_effective =
                activation_pos as f32 / states[0].freeze_activation_len as f32;
            for _ in 0..8 {
                let output = process_momentary_fx_states(
                    &mut states,
                    target,
                    input.0,
                    input.1,
                    None,
                    SAMPLE_RATE,
                );
                assert!(output.0 != 0.0 || output.1 != 0.0);
                assert_eq!(states[0].freeze_activation_pos, activation_pos);
                let effective = activation_pos as f32 / states[0].freeze_activation_len as f32
                    * (32 - states[0].release_pos) as f32
                    / 32.0;
                assert!(effective <= previous_effective);
                previous_effective = effective;
            }
            return;
        }
    }
    unreachable!("freeze startup did not reach the release point");
}

#[test]
fn freeze_to_pitch_keeps_pitch_startup_bit_identical_to_freeze_only() {
    let target = MomentaryFxTarget::Global;
    let mut freeze_only = vec![new_fx("freeze", MomentaryFxKind::Freeze, target)];
    let mut freeze_then_pitch = vec![new_fx("freeze", MomentaryFxKind::Freeze, target)];
    for frame in 0..2_000 {
        let input = (0.2 + (frame % 13) as f32 * 0.01, -0.3);
        let _ = process_momentary_fx_states(
            &mut freeze_only,
            target,
            input.0,
            input.1,
            None,
            SAMPLE_RATE,
        );
        let _ = process_momentary_fx_states(
            &mut freeze_then_pitch,
            target,
            input.0,
            input.1,
            None,
            SAMPLE_RATE,
        );
    }
    freeze_then_pitch.push(new_fx("pitch", MomentaryFxKind::PitchShift, target));
    for frame in 0..PITCH_FILL_FRAMES {
        let input = (0.15 + (frame % 11) as f32 * 0.01, -0.25);
        let freeze_output = process_momentary_fx_states(
            &mut freeze_only,
            target,
            input.0,
            input.1,
            None,
            SAMPLE_RATE,
        );
        let pitch_output = process_momentary_fx_states(
            &mut freeze_then_pitch,
            target,
            input.0,
            input.1,
            None,
            SAMPLE_RATE,
        );
        assert_eq!(
            pitch_output.0.to_bits(),
            freeze_output.0.to_bits(),
            "left frame {frame}"
        );
        assert_eq!(
            pitch_output.1.to_bits(),
            freeze_output.1.to_bits(),
            "right frame {frame}"
        );
    }
}

#[test]
fn momentary_pitch_filter_activation_is_order_independent_for_shared_targets() {
    let input = controlled_input();
    for target in [
        MomentaryFxTarget::Global,
        MomentaryFxTarget::Instrument { index: 0 },
        MomentaryFxTarget::FxBus { index: 0 },
    ] {
        let filter_first = render_combined_fx(target, false, &input);
        let pitch_first = render_combined_fx(target, true, &input);
        assert_eq!(filter_first.len(), pitch_first.len());
        for (frame, ((actual_l, actual_r), (expected_l, expected_r))) in
            filter_first.iter().zip(&pitch_first).enumerate()
        {
            assert_eq!(
                actual_l.to_bits(),
                expected_l.to_bits(),
                "left sample differs at frame {frame} for {target:?}"
            );
            assert_eq!(
                actual_r.to_bits(),
                expected_r.to_bits(),
                "right sample differs at frame {frame} for {target:?}"
            );
        }
    }
}

#[test]
fn momentary_pitch_filter_global_activation_matches_host_render_order() {
    let input = controlled_input();
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    start_filter_and_pitch(&mut engine, false, MomentaryFxTarget::Global);
    let mut filter_first_left: Vec<f32> = input.iter().map(|(left, _)| *left).collect();
    let mut filter_first_right: Vec<f32> = input.iter().map(|(_, right)| *right).collect();
    engine.finish_persistent_block(
        RENDER_FRAMES,
        &mut filter_first_left,
        &mut filter_first_right,
    );

    let mut reference = SynthEngine::new(SAMPLE_RATE);
    start_filter_and_pitch(&mut reference, true, MomentaryFxTarget::Global);
    let mut pitch_first_left: Vec<f32> = input.iter().map(|(left, _)| *left).collect();
    let mut pitch_first_right: Vec<f32> = input.iter().map(|(_, right)| *right).collect();
    reference.finish_persistent_block(RENDER_FRAMES, &mut pitch_first_left, &mut pitch_first_right);

    for frame in 0..RENDER_FRAMES {
        assert_eq!(
            filter_first_left[frame].to_bits(),
            pitch_first_left[frame].to_bits(),
            "left host sample differs at frame {frame}"
        );
        assert_eq!(
            filter_first_right[frame].to_bits(),
            pitch_first_right[frame].to_bits(),
            "right host sample differs at frame {frame}"
        );
    }
}

fn render_combined_fx(
    target: MomentaryFxTarget,
    pitch_first: bool,
    input: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let mut states = if pitch_first {
        vec![
            new_fx("pitch", MomentaryFxKind::PitchShift, target),
            new_fx("filter", MomentaryFxKind::FilterSweep, target),
        ]
    } else {
        vec![
            new_fx("filter", MomentaryFxKind::FilterSweep, target),
            new_fx("pitch", MomentaryFxKind::PitchShift, target),
        ]
    };
    input
        .iter()
        .map(|(left, right)| {
            process_momentary_fx_states(&mut states, target, *left, *right, None, SAMPLE_RATE)
        })
        .collect()
}

fn start_filter_and_pitch(engine: &mut SynthEngine, pitch_first: bool, target: MomentaryFxTarget) {
    let filter_params = filter_params();
    let pitch_params = pitch_params();
    if pitch_first {
        engine.momentary_fx_start(
            "pitch".to_string(),
            "pitch_shift".to_string(),
            pitch_params,
            target,
        );
        engine.momentary_fx_start(
            "filter".to_string(),
            "filter_sweep".to_string(),
            filter_params,
            target,
        );
    } else {
        engine.momentary_fx_start(
            "filter".to_string(),
            "filter_sweep".to_string(),
            filter_params,
            target,
        );
        engine.momentary_fx_start(
            "pitch".to_string(),
            "pitch_shift".to_string(),
            pitch_params,
            target,
        );
    }
}

fn new_fx(id: &str, kind: MomentaryFxKind, target: MomentaryFxTarget) -> MomentaryFxState {
    let params = match kind {
        MomentaryFxKind::FilterSweep => filter_params(),
        MomentaryFxKind::PitchShift => pitch_params(),
        MomentaryFxKind::Freeze => freeze_params(100.0),
        _ => unreachable!("test only creates filter, pitch, and freeze FX"),
    };
    MomentaryFxState::new(id.to_string(), kind, &params, target, SAMPLE_RATE)
}

fn filter_params() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("cutoffPct".to_string(), json!(0.0)),
        ("resonancePct".to_string(), json!(0.0)),
        ("sweepInMs".to_string(), json!(1.0)),
    ])
}

fn pitch_params() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("semitones".to_string(), json!(7.0)),
        ("mixPct".to_string(), json!(100.0)),
    ])
}

fn freeze_params(mix_pct: f32) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([("mixPct".to_string(), json!(mix_pct))])
}

fn controlled_input() -> Vec<(f32, f32)> {
    (0..RENDER_FRAMES)
        .map(|frame| {
            let impulse = if frame % 31 == 0 { 1.0 } else { 0.0 };
            let sustained = if frame % 2 == 0 { 0.55 } else { -0.55 };
            (impulse + sustained, impulse - sustained)
        })
        .collect()
}
