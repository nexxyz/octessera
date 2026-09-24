use super::super::scalar_param::ScalarMutation;
use super::support::{MomentaryFxKind, MomentaryFxRuntimeParams, MomentaryFxState};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) enum PitchStopAction {
    Retire,
    Release,
    Ignore,
}

pub(super) fn stop(fx: &mut MomentaryFxState) -> PitchStopAction {
    if fx.releasing {
        return PitchStopAction::Ignore;
    }
    let activation = if fx.pitch_fill_pos < super::support::PITCH_FILL_FRAMES {
        0.0
    } else {
        (fx.pitch_ramp_pos as f32 / fx.pitch_ramp_len.max(1) as f32).clamp(0.0, 1.0)
    };
    let MomentaryFxRuntimeParams::PitchShift {
        mix, slide_out_len, ..
    } = fx.runtime_params
    else {
        return PitchStopAction::Ignore;
    };
    if mix * activation == 0.0 {
        return PitchStopAction::Retire;
    }
    fx.releasing = true;
    fx.release_pos = 0;
    fx.release_len = fx.pitch_ramp_len.max(slide_out_len);
    fx.pitch_slide_start_octaves = fx.pitch_amount_octaves;
    fx.pitch_slide_target_octaves = 0.0;
    fx.pitch_slide_pos = 0;
    fx.pitch_slide_len = slide_out_len;
    PitchStopAction::Release
}

pub(super) fn update(
    fx: &mut MomentaryFxState,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) {
    if fx.releasing {
        return;
    }
    let next =
        MomentaryFxRuntimeParams::from_params(MomentaryFxKind::PitchShift, params, sample_rate);
    let _ = update_runtime_params(fx, next);
}

pub(super) fn update_runtime_params(
    fx: &mut MomentaryFxState,
    next: MomentaryFxRuntimeParams,
) -> ScalarMutation {
    let MomentaryFxRuntimeParams::PitchShift {
        target_octaves,
        mix,
        slide_in_len,
        slide_out_len,
    } = next
    else {
        return ScalarMutation::Rejected;
    };
    let MomentaryFxRuntimeParams::PitchShift {
        target_octaves: current_target_octaves,
        mix: current_mix,
        slide_in_len: current_slide_in_len,
        slide_out_len: current_slide_out_len,
    } = fx.runtime_params
    else {
        return ScalarMutation::Rejected;
    };
    if current_target_octaves.to_bits() == target_octaves.to_bits()
        && current_mix.to_bits() == mix.to_bits()
        && current_slide_in_len == slide_in_len
        && current_slide_out_len == slide_out_len
    {
        return ScalarMutation::Unchanged;
    }
    let target_changed = current_target_octaves.to_bits() != target_octaves.to_bits();
    let slide_in_changed = current_slide_in_len != slide_in_len;
    fx.runtime_params = MomentaryFxRuntimeParams::PitchShift {
        target_octaves,
        mix,
        slide_in_len,
        slide_out_len,
    };
    if target_changed || (slide_in_changed && fx.pitch_slide_pos < fx.pitch_slide_len) {
        fx.pitch_slide_start_octaves = fx.pitch_amount_octaves;
        fx.pitch_slide_target_octaves = target_octaves;
        fx.pitch_slide_pos = 0;
        fx.pitch_slide_len = slide_in_len;
    } else if slide_in_changed {
        fx.pitch_slide_start_octaves = target_octaves;
        fx.pitch_slide_target_octaves = target_octaves;
        fx.pitch_slide_pos = slide_in_len;
        fx.pitch_slide_len = slide_in_len;
    }
    ScalarMutation::Changed
}
