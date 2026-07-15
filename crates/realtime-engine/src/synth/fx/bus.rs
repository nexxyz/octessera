use super::algorithms::*;
use super::{fx_bus_state_from_params, FxBusParams, FxBusState, INSTRUMENT_SLOT_COUNT, PI};

pub(in crate::synth) fn process_fx_bus_slot(
    params: &FxBusParams,
    state: &mut FxBusState,
    input: f32,
    slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    bus_in: &[f32],
    sample_rate: u32,
) -> f32 {
    match *params {
        FxBusParams::None => input,
        FxBusParams::Tremolo { rate_hz, depth } => {
            let FxBusState::Tremolo { phase } = state else {
                *state = FxBusState::Tremolo { phase: 0.0 };
                return input;
            };
            let gain = (1.0 - depth) + depth * ((phase.sin() + 1.0) * 0.5);
            *phase = wrap_phase(*phase + 2.0 * PI * rate_hz / sample_rate as f32);
            input * gain
        }
        FxBusParams::Delay {
            time_ms,
            feedback,
            mix,
            spread: _,
        } => process_delay(state, input, time_ms, feedback, mix, sample_rate),
        FxBusParams::ModDelay {
            rate_hz,
            depth_ms,
            base_ms,
            feedback,
            mix,
        } => process_mod_delay(
            state,
            input,
            ModDelayParams {
                rate_hz,
                depth_ms,
                base_ms,
                feedback,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::FilterLfo {
            kind,
            rate_hz,
            depth,
            center_hz,
            q,
        } => process_filter_lfo(
            state,
            input,
            FilterLfoParams {
                kind,
                rate_hz,
                depth,
                center_hz,
                q,
            },
            sample_rate,
        ),
        FxBusParams::Reverb { mix, decay, damp } => {
            let FxBusState::Reverb { bufs, idxs, lp } = state else {
                *state = fx_bus_state_from_params(params, sample_rate);
                return input;
            };
            let mut wet = 0.0;
            for i in 0..4 {
                let delayed = bufs[i][idxs[i]];
                lp[i] = delayed * (1.0 - damp) + lp[i] * damp;
                bufs[i][idxs[i]] = input + lp[i] * decay;
                idxs[i] = (idxs[i] + 1) % bufs[i].len();
                wet += delayed;
            }
            (input * (1.0 - mix) + wet * 0.25 * mix).clamp(-1.5, 1.5)
        }
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => process_glitch(state, input, chance, slice_ms, mix, sample_rate),
        FxBusParams::AutoPan { rate_hz, depth } => {
            let FxBusState::AutoPan { phase, pos } = state else {
                *state = FxBusState::AutoPan {
                    phase: 0.0,
                    pos: 0.5,
                };
                return input;
            };
            *pos = 0.5 + ((*phase).sin() * 0.5 * depth);
            *phase = wrap_phase(*phase + 2.0 * PI * rate_hz / sample_rate as f32);
            input
        }
        FxBusParams::Duck {
            source,
            threshold,
            amount,
            attack_ms,
            release_ms,
        } => process_duck(
            state,
            input,
            DuckParams {
                source,
                threshold,
                amount,
                attack_ms,
                release_ms,
            },
            slot_out,
            bus_in,
            sample_rate,
        ),
        FxBusParams::Saturator { drive, mix } => process_saturator(input, drive, mix),
        FxBusParams::Distortion { drive, clip, mix } => process_distortion(input, drive, clip, mix),
        FxBusParams::Bitcrusher {
            rate_div,
            bits,
            mix,
        } => process_bitcrusher(state, input, rate_div, bits, mix),
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => process_compressor(
            state,
            input,
            CompressorParams {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_db,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => process_eq(
            state,
            input,
            EqParams {
                low_gain_db,
                mid_gain_db,
                mid_freq_hz,
                mid_q,
                high_gain_db,
                mix,
            },
            sample_rate,
        ),
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            let FxBusState::Vinyl(vinyl) = state else {
                *state = FxBusState::Vinyl(VinylState::new());
                return input;
            };
            process_vinyl_mono_bus(
                vinyl,
                input,
                VinylParams {
                    saturation,
                    crackle,
                    warp_depth,
                    mix,
                },
                sample_rate,
            )
        }
    }
}
