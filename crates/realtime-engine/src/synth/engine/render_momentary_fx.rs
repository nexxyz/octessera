use super::retired_state::{store_retired_momentary, PREVIEW_AUDITION_SLOTS};
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
            self.sample_rate,
        )
    }
}

pub(super) fn process_momentary_fx_states(
    states: &mut Vec<MomentaryFxState>,
    target: MomentaryFxTarget,
    left: f32,
    right: f32,
    mut retired: Option<&mut [Option<MomentaryFxState>; PREVIEW_AUDITION_SLOTS]>,
    sample_rate: u32,
) -> (f32, f32) {
    if states.is_empty() {
        return (left, right);
    }
    let mut l = left;
    let mut r = right;
    for fx in states.iter_mut() {
        if let Some((next_l, next_r)) = process_momentary_fx_state(fx, target, l, r, sample_rate) {
            l = next_l;
            r = next_r;
        }
    }

    for index in (0..states.len()).rev() {
        let completed = {
            let fx = &states[index];
            fx.target == target
                && fx.releasing
                && match fx.kind {
                    MomentaryFxKind::FilterSweep => fx.sweep_pos <= 0.0,
                    MomentaryFxKind::Freeze => fx.release_pos >= fx.release_len,
                    _ => true,
                }
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
    sample_rate: u32,
) -> Option<(f32, f32)> {
    if fx.target != target {
        return None;
    }
    Some(match fx.kind {
        MomentaryFxKind::Stutter => process_stutter(fx, left, right),
        MomentaryFxKind::Freeze => process_freeze(fx, left, right),
        MomentaryFxKind::FilterSweep => process_filter_sweep(fx, left, right, sample_rate),
        MomentaryFxKind::PitchShift => process_pitch_shift(fx, left, right),
    })
}

fn process_stutter(fx: &mut MomentaryFxState, left: f32, right: f32) -> (f32, f32) {
    let MomentaryFxRuntimeParams::Stutter { depth } = fx.runtime_params else {
        return (left, right);
    };
    let segment_len = fx.stutter_segment_len.min(fx.stutter_l.len()).max(1);
    let ramp_len = fx.stutter_ramp_len.min(segment_len / 4).max(1);
    let mut l = left;
    let mut r = right;

    if !fx.stutter_ready {
        fx.stutter_l[fx.stutter_write] = l;
        fx.stutter_r[fx.stutter_write] = r;
        fx.stutter_write += 1;
        if fx.stutter_write >= segment_len {
            fx.stutter_ready = true;
            fx.stutter_write = 0;
            fx.stutter_ramp_pos = 0;
        }
    } else {
        let read = fx.stutter_write;
        let mut wet_l = fx.stutter_l[read];
        let mut wet_r = fx.stutter_r[read];
        let eff_wet = if fx.stutter_ramp_pos < ramp_len {
            let ramp = fx.stutter_ramp_pos as f32 / ramp_len as f32;
            fx.stutter_ramp_pos += 1;
            depth * ramp
        } else {
            depth
        };

        if read < ramp_len {
            let fade_in = read as f32 / ramp_len as f32;
            let end_read = segment_len - ramp_len + read;
            wet_l = wet_l * fade_in + fx.stutter_l[end_read] * (1.0 - fade_in);
            wet_r = wet_r * fade_in + fx.stutter_r[end_read] * (1.0 - fade_in);
        }

        l = l * (1.0 - eff_wet) + wet_l * eff_wet;
        r = r * (1.0 - eff_wet) + wet_r * eff_wet;
        fx.stutter_write += 1;
        if fx.stutter_write >= segment_len {
            fx.stutter_write = 0;
        }
    }

    (l, r)
}

fn process_freeze(fx: &mut MomentaryFxState, left: f32, right: f32) -> (f32, f32) {
    let MomentaryFxRuntimeParams::Freeze { mix, .. } = fx.runtime_params else {
        return (left, right);
    };
    let feedback = 0.997_f32;
    let damp = 0.35_f32;
    let mut wet_l = 0.0_f32;
    let mut wet_r = 0.0_f32;

    if fx.releasing {
        let total = fx.release_len.max(1) as f32;
        let fade = 1.0 - (fx.release_pos as f32 / total);
        fx.release_pos += 1;
        for i in 0..4 {
            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
            fx.freeze_bufs[i][fx.freeze_idxs[i]] = fx.freeze_lp[i] * feedback;
            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
            if i < 2 {
                wet_l += delayed;
            } else {
                wet_r += delayed;
            }
        }
        wet_l *= 0.5;
        wet_r *= 0.5;
        (
            left * (1.0 - mix * fade) + wet_l * mix,
            right * (1.0 - mix * fade) + wet_r * mix,
        )
    } else {
        let injecting = fx.freeze_inject_pos < fx.freeze_inject_len;
        let inject_gain = if injecting { 1.0 } else { 0.0 };
        if injecting {
            fx.freeze_inject_pos += 1;
        }
        for i in 0..4 {
            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
            let channel_in = if i < 2 { left } else { right };
            fx.freeze_bufs[i][fx.freeze_idxs[i]] =
                channel_in * inject_gain + fx.freeze_lp[i] * feedback;
            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
            if i < 2 {
                wet_l += delayed;
            } else {
                wet_r += delayed;
            }
        }
        wet_l *= 0.5;
        wet_r *= 0.5;
        (
            left * (1.0 - mix) + wet_l * mix,
            right * (1.0 - mix) + wet_r * mix,
        )
    }
}

fn process_filter_sweep(
    fx: &mut MomentaryFxState,
    left: f32,
    right: f32,
    sample_rate: u32,
) -> (f32, f32) {
    let MomentaryFxRuntimeParams::FilterSweep {
        target_cutoff,
        q,
        sweep_in_step,
        sweep_out_step,
    } = fx.runtime_params
    else {
        return (left, right);
    };

    if fx.releasing {
        fx.sweep_pos -= sweep_out_step;
        if fx.sweep_pos < 0.0 {
            fx.sweep_pos = 0.0;
        }
    } else {
        fx.sweep_pos += sweep_in_step;
        if fx.sweep_pos > 1.0 {
            fx.sweep_pos = 1.0;
        }
    }

    let cutoff = 20_000.0 + (target_cutoff - 20_000.0) * fx.sweep_pos;
    (
        fx.filt_l
            .process(left, FilterType::Lowpass, cutoff, q, sample_rate),
        fx.filt_r
            .process(right, FilterType::Lowpass, cutoff, q, sample_rate),
    )
}

fn process_pitch_shift(fx: &mut MomentaryFxState, left: f32, right: f32) -> (f32, f32) {
    let MomentaryFxRuntimeParams::PitchShift { ratio, mix } = fx.runtime_params else {
        return (left, right);
    };
    let (wet_l, wet_r) = fx.pitch_shifter.process_frame(left, right, ratio);
    let ramp = if fx.pitch_ramp_pos < fx.pitch_ramp_len {
        let ramp = fx.pitch_ramp_pos as f32 / fx.pitch_ramp_len as f32;
        fx.pitch_ramp_pos += 1;
        ramp
    } else {
        1.0
    };
    let wet_mix = mix * ramp;
    (
        left * (1.0 - wet_mix) + wet_l * wet_mix,
        right * (1.0 - wet_mix) + wet_r * wet_mix,
    )
}
