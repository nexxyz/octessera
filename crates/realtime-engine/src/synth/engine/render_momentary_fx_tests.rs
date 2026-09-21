use super::render_momentary_fx::process_momentary_fx_states;
use super::{MomentaryFxKind, MomentaryFxState, SynthEngine, DRY_HISTORY_FRAMES};
use crate::synth::MomentaryFxTarget;
use serde_json::json;
use std::collections::BTreeMap;

const SAMPLE_RATE: u32 = 48_000;
const RENDER_FRAMES: usize = 512;

#[test]
fn momentary_pitch_filter_activation_is_order_independent_for_shared_targets() {
    let history = controlled_history();
    let input = controlled_input();
    for target in [
        MomentaryFxTarget::Global,
        MomentaryFxTarget::Instrument { index: 0 },
        MomentaryFxTarget::FxBus { index: 0 },
    ] {
        let filter_first = render_combined_fx(target, false, &history, &input);
        let pitch_first = render_combined_fx(target, true, &history, &input);
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
    let history = controlled_history();
    let input = controlled_input();
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    let mut history_left = Vec::with_capacity(DRY_HISTORY_FRAMES);
    let mut history_right = Vec::with_capacity(DRY_HISTORY_FRAMES);
    for frame in 0..DRY_HISTORY_FRAMES {
        history_left.push(history[frame * 2]);
        history_right.push(history[frame * 2 + 1]);
    }
    engine.finish_persistent_block(DRY_HISTORY_FRAMES, &mut history_left, &mut history_right);
    start_filter_and_pitch(&mut engine, false, MomentaryFxTarget::Global);
    let mut filter_first_left: Vec<f32> = input.iter().map(|(left, _)| *left).collect();
    let mut filter_first_right: Vec<f32> = input.iter().map(|(_, right)| *right).collect();
    engine.finish_persistent_block(
        RENDER_FRAMES,
        &mut filter_first_left,
        &mut filter_first_right,
    );

    let mut reference = SynthEngine::new(SAMPLE_RATE);
    let mut reference_history_left = Vec::with_capacity(DRY_HISTORY_FRAMES);
    let mut reference_history_right = Vec::with_capacity(DRY_HISTORY_FRAMES);
    for frame in 0..DRY_HISTORY_FRAMES {
        reference_history_left.push(history[frame * 2]);
        reference_history_right.push(history[frame * 2 + 1]);
    }
    reference.finish_persistent_block(
        DRY_HISTORY_FRAMES,
        &mut reference_history_left,
        &mut reference_history_right,
    );
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
    history: &[f32],
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
    prefill_pitch(&mut states, history);
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
        _ => unreachable!("test only creates filter and pitch FX"),
    };
    MomentaryFxState::new(id.to_string(), kind, &params, target, SAMPLE_RATE)
}

fn prefill_pitch(states: &mut [MomentaryFxState], history: &[f32]) {
    states
        .iter_mut()
        .find(|fx| fx.kind == MomentaryFxKind::PitchShift)
        .expect("pitch FX")
        .pitch_shifter
        .prefill_from_ring(history, 0);
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

fn controlled_history() -> Vec<f32> {
    let mut history = vec![0.0; DRY_HISTORY_FRAMES * 2];
    for frame in 0..DRY_HISTORY_FRAMES {
        let impulse = if frame % 29 == 0 { 1.0 } else { 0.0 };
        let sustained = if frame % 2 == 0 { 0.65 } else { -0.65 };
        history[frame * 2] = impulse + sustained;
        history[frame * 2 + 1] = impulse - sustained;
    }
    history
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
