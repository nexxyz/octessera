use super::retired_state::store_retired_momentary;
use super::*;

impl SynthEngine {
    pub(super) fn process_momentary_fx_target(
        &mut self,
        target: MomentaryFxTarget,
        left: f32,
        right: f32,
    ) -> (f32, f32) {
        process_momentary_fx_states(
            &mut self.momentary_fx,
            target,
            left,
            right,
            Some(&mut self.pending_render_retired.displaced_momentary_fx),
        )
    }
}

pub(super) fn process_momentary_fx_states(
    states: &mut Vec<MomentaryFxState>,
    target: MomentaryFxTarget,
    left: f32,
    right: f32,
    mut retired: Option<&mut [Option<MomentaryFxState>; MOMENTARY_FX_CAPACITY]>,
) -> (f32, f32) {
    if states.is_empty() {
        return (left, right);
    }
    let mut l = left;
    let mut r = right;
    for index in 0..states.len() {
        let kind = states[index].kind;
        let state_target = states[index].target;
        let pitch_deferred = kind == MomentaryFxKind::PitchShift
            && states[..index]
                .iter()
                .any(|fx| fx.target == target && fx.kind == MomentaryFxKind::FilterSweep);
        if pitch_deferred {
            continue;
        }
        if kind == MomentaryFxKind::FilterSweep && state_target == target {
            let pitch_index = states[index + 1..]
                .iter()
                .position(|fx| fx.target == target && fx.kind == MomentaryFxKind::PitchShift)
                .map(|offset| index + 1 + offset);
            if let Some(pitch_index) = pitch_index {
                if let Some((next_l, next_r)) =
                    process_momentary_fx_state(&mut states[pitch_index], target, l, r)
                {
                    l = next_l;
                    r = next_r;
                }
            }
        }
        if let Some((next_l, next_r)) = process_momentary_fx_state(&mut states[index], target, l, r)
        {
            l = next_l;
            r = next_r;
        }
    }

    for index in (0..states.len()).rev() {
        let completed = {
            let fx = &states[index];
            fx.target == target && (fx.retire_faded() || fx.releasing && fx.voice.finished())
        };
        if completed {
            let state = states.remove(index);
            if let Some(retired) = retired.as_deref_mut() {
                store_retired_momentary(retired, state);
            }
        }
    }

    (l, r)
}

fn process_momentary_fx_state(
    fx: &mut MomentaryFxState,
    target: MomentaryFxTarget,
    left: f32,
    right: f32,
) -> Option<(f32, f32)> {
    if fx.target != target {
        return None;
    }
    let (wet_l, wet_r) = fx.voice.process(left, right);
    let Some(fade) = fx.retire_fade.as_mut() else {
        return Some((wet_l, wet_r));
    };
    let gain = fade.toward(0.0);
    Some((left + (wet_l - left) * gain, right + (wet_r - right) * gain))
}

#[cfg(test)]
mod pitch_shift_release_tests;
#[cfg(test)]
mod pitch_shift_tests;
#[cfg(test)]
mod tests;
