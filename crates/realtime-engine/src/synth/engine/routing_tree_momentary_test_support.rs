use super::momentary_fx_voice::MomentaryFxVoice;
use super::SynthEngine;

/// The positions and filter memories of each running Play FX; capture buffers are left out, as
/// the routed and inline renders they hold may differ by reassociation.
pub(super) fn momentary_state(engine: &SynthEngine) -> Vec<String> {
    engine
        .momentary_fx
        .iter()
        .map(|fx| {
            let voice = match &fx.voice {
                MomentaryFxVoice::Stutter(voice) => format!(
                    "{:?}",
                    (voice.write, voice.ready, voice.params, voice.ramp_pos)
                ),
                MomentaryFxVoice::Freeze(voice) => format!(
                    "{:?}",
                    (
                        voice.idxs,
                        voice.lp,
                        voice.inject_pos,
                        voice.ready_len,
                        voice.activation_pos,
                        voice.activation_len,
                        voice.release_pos,
                        voice.release_len,
                    )
                ),
                MomentaryFxVoice::FilterSweep(voice) => {
                    format!("{:?}", (voice.sweep_pos, voice.filt_l, voice.filt_r))
                }
                MomentaryFxVoice::PitchShift(voice) => format!(
                    "{:?}",
                    (
                        voice.fill_pos,
                        voice.ramp_pos,
                        voice.ramp_len,
                        voice.amount_octaves,
                        voice.slide_start_octaves,
                        voice.slide_target_octaves,
                        voice.slide_pos,
                        voice.slide_len,
                        voice.release_pos,
                        voice.release_len,
                        voice.shifter.write_pos,
                    )
                ),
                _ => String::new(),
            };
            format!(
                "{}|{:?}|{:?}|{}|{:?}|{voice}",
                fx.id, fx.kind, fx.target, fx.releasing, fx.retire_fade
            )
        })
        .collect()
}
