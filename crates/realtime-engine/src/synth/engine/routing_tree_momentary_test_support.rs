use super::SynthEngine;

pub(super) fn momentary_state(engine: &SynthEngine) -> Vec<String> {
    engine
        .momentary_fx
        .iter()
        .map(|fx| {
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                fx.id,
                fx.kind,
                fx.target,
                fx.releasing,
                fx.release_pos,
                fx.release_len,
                fx.sweep_pos,
                fx.filt_l,
                fx.filt_r,
                fx.pitch_fill_pos,
                fx.pitch_ramp_pos,
                fx.pitch_ramp_len,
                fx.stutter_write,
                fx.stutter_ready,
                fx.stutter_segment_len,
                fx.stutter_ramp_len,
                fx.stutter_ramp_pos,
                fx.freeze_idxs,
                fx.freeze_lp,
                fx.freeze_inject_pos,
                fx.freeze_inject_len,
                fx.freeze_ready_len,
                fx.freeze_activation_pos,
                fx.freeze_activation_len,
                fx.pitch_shifter.write_pos,
            )
        })
        .collect()
}
