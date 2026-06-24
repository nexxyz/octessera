use super::algorithms::*;
use super::{master_fx_state_from_params, FxBusParams, MasterFxState};

pub(in crate::synth) fn process_master_fx_slot(
    params: &FxBusParams,
    state: &mut MasterFxState,
    left: f32,
    right: f32,
    sample_rate: u32,
) -> (f32, f32) {
    match *params {
        FxBusParams::None => (left, right),
        FxBusParams::Saturator { drive, mix } => (
            process_saturator(left, drive, mix),
            process_saturator(right, drive, mix),
        ),
        FxBusParams::Distortion { drive, clip, mix } => (
            process_distortion(left, drive, clip, mix),
            process_distortion(right, drive, clip, mix),
        ),
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => {
            let MasterFxState::Compressor { env } = state else {
                *state = MasterFxState::Compressor { env: 0.0 };
                return (left, right);
            };
            let params = CompressorParams {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_db,
                mix,
            };
            let detector = left.abs().max(right.abs()).min(1.0);
            let gain = compressor_gain(env, detector, &params, sample_rate);
            (
                mix_sample(left, left * gain, mix),
                mix_sample(right, right * gain, mix),
            )
        }
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => {
            let MasterFxState::Eq { left: l, right: r } = state else {
                *state = master_fx_state_from_params(params);
                return (left, right);
            };
            let eq_params = EqParams {
                low_gain_db,
                mid_gain_db,
                mid_freq_hz,
                mid_q,
                high_gain_db,
                mix,
            };
            let wet_l = process_eq_channel(l.as_mut(), left, &eq_params, sample_rate);
            let wet_r = process_eq_channel(r.as_mut(), right, &eq_params, sample_rate);
            (mix_sample(left, wet_l, mix), mix_sample(right, wet_r, mix))
        }
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            let MasterFxState::Vinyl(vinyl) = state else {
                *state = MasterFxState::Vinyl(VinylState::new());
                return (left, right);
            };
            process_vinyl_stereo(
                vinyl,
                left,
                right,
                VinylParams {
                    saturation,
                    crackle,
                    warp_depth,
                    mix,
                },
                sample_rate,
            )
        }
        _ => (left, right),
    }
}
