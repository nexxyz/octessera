use super::super::super::fx_param::{apply_fx_param, FxParamId, FxParamMutation};
use super::super::super::fx_params::FxBusParams;

#[test]
fn fx_param_id_is_a_closed_camel_case_whitelist() {
    assert_eq!(FxParamId::ALL.len(), 30);
    for id in FxParamId::ALL {
        let encoded = serde_json::to_string(&id).expect("FX parameter id serializes");
        let decoded: FxParamId = serde_json::from_str(&encoded).expect("FX parameter id parses");
        assert_eq!(decoded, id);
    }
    assert_eq!(
        serde_json::to_string(&FxParamId::RateHz).unwrap(),
        "\"rateHz\""
    );
    assert_eq!(
        serde_json::to_string(&FxParamId::WarpDepthPct).unwrap(),
        "\"warpDepthPct\""
    );
}

#[test]
fn every_whitelisted_fx_param_has_owned_clamping_and_compatibility() {
    let mut tremolo = FxBusParams::Tremolo {
        rate_hz: 1.0,
        depth: 0.5,
    };
    assert_changed(&mut tremolo, FxParamId::RateHz, 100.0);
    assert_eq!(tremolo_rate(&tremolo), 40.0);
    assert_changed(&mut tremolo, FxParamId::DepthPct, 120.0);
    assert_eq!(tremolo_depth(&tremolo), 1.0);
    assert_rejected(&mut tremolo, FxParamId::Feedback);

    let mut delay = FxBusParams::Delay {
        time_ms: 20.0,
        feedback: 0.2,
        mix: 0.3,
        spread: 0.4,
    };
    assert_changed(&mut delay, FxParamId::Feedback, -1.0);
    assert_eq!(delay_feedback(&delay), 0.0);
    assert_changed(&mut delay, FxParamId::MixPct, 120.0);
    assert_eq!(delay_mix(&delay), 1.0);
    assert_changed(&mut delay, FxParamId::SpreadPct, -1.0);
    assert_eq!(delay_spread(&delay), 0.0);
    assert_rejected(&mut delay, FxParamId::RateHz);

    let mut mod_delay = FxBusParams::ModDelay {
        rate_hz: 1.0,
        depth_ms: 4.0,
        base_ms: 8.0,
        feedback: 0.2,
        mix: 0.3,
    };
    assert_changed(&mut mod_delay, FxParamId::RateHz, -1.0);
    assert_eq!(mod_rate(&mod_delay), 0.02);
    assert_changed(&mut mod_delay, FxParamId::Feedback, -2.0);
    assert_eq!(mod_feedback(&mod_delay), -0.95);
    assert_changed(&mut mod_delay, FxParamId::MixPct, 120.0);
    assert_eq!(mod_mix(&mod_delay), 1.0);
    assert_rejected(&mut mod_delay, FxParamId::DepthPct);

    let mut filter = FxBusParams::FilterLfo {
        kind: super::super::super::fx_params::FilterLfoKind::FilterLfo,
        rate_hz: 1.0,
        depth: 0.5,
        center_hz: 1_000.0,
        q: 1.0,
    };
    assert_changed(&mut filter, FxParamId::RateHz, -1.0);
    assert_eq!(filter_rate(&filter), 0.02);
    assert_changed(&mut filter, FxParamId::CenterHz, 0.0);
    assert_eq!(filter_center(&filter), 40.0);
    assert_changed(&mut filter, FxParamId::DepthPct, 120.0);
    assert_eq!(filter_depth(&filter), 1.0);
    assert_changed(&mut filter, FxParamId::Q, 100.0);
    assert_eq!(filter_q(&filter), 20.0);

    let mut reverb = FxBusParams::Reverb {
        mix: 0.3,
        decay: 0.5,
        damp: 0.5,
    };
    assert_changed(&mut reverb, FxParamId::Decay, 2.0);
    assert_eq!(reverb_decay(&reverb), 0.995);
    assert_changed(&mut reverb, FxParamId::Damp, -1.0);
    assert_eq!(reverb_damp(&reverb), 0.0);
    assert_changed(&mut reverb, FxParamId::MixPct, 120.0);
    assert_eq!(reverb_mix(&reverb), 1.0);

    let mut glitch = FxBusParams::Glitch {
        chance: 0.3,
        slice_ms: 80.0,
        mix: 0.3,
    };
    assert_changed(&mut glitch, FxParamId::ChancePct, -1.0);
    assert_eq!(glitch_chance(&glitch), 0.0);
    assert_changed(&mut glitch, FxParamId::SliceMs, 1.0);
    assert_eq!(glitch_slice(&glitch), 5.0);
    assert_changed(&mut glitch, FxParamId::MixPct, 120.0);
    assert_eq!(glitch_mix(&glitch), 1.0);

    let mut auto_pan = FxBusParams::AutoPan {
        rate_hz: 1.0,
        depth: 0.5,
    };
    assert_changed(&mut auto_pan, FxParamId::RateHz, -1.0);
    assert_eq!(auto_pan_rate(&auto_pan), 0.02);
    assert_changed(&mut auto_pan, FxParamId::DepthPct, 120.0);
    assert_eq!(auto_pan_depth(&auto_pan), 1.0);

    let mut duck = FxBusParams::Duck {
        source: super::super::super::fx_params::DuckSource::Instrument(0),
        source_tap: super::super::super::fx_params::DuckSourceTap::Pre,
        threshold: 0.5,
        amount: 0.5,
        attack_ms: 8.0,
        release_ms: 160.0,
    };
    assert_changed(&mut duck, FxParamId::Threshold, -1.0);
    assert_eq!(duck_threshold(&duck), 0.0);
    assert_changed(&mut duck, FxParamId::AmountPct, 120.0);
    assert_eq!(duck_amount(&duck), 1.0);
    assert_changed(&mut duck, FxParamId::AttackMs, 0.0);
    assert_eq!(duck_attack(&duck), 1.0);
    assert_changed(&mut duck, FxParamId::ReleaseMs, 6_000.0);
    assert_eq!(duck_release(&duck), 5_000.0);

    let mut saturator = FxBusParams::Saturator {
        drive: 1.0,
        mix: 0.5,
    };
    assert_changed(&mut saturator, FxParamId::Drive, 30.0);
    assert_eq!(saturator_drive(&saturator), 20.0);
    assert_changed(&mut saturator, FxParamId::MixPct, 120.0);
    assert_eq!(saturator_mix(&saturator), 1.0);

    let mut distortion = FxBusParams::Distortion {
        drive: 1.0,
        clip: 0.5,
        mix: 0.5,
    };
    assert_changed(&mut distortion, FxParamId::Drive, 60.0);
    assert_eq!(distortion_drive(&distortion), 50.0);
    assert_changed(&mut distortion, FxParamId::Clip, 0.0);
    assert_eq!(distortion_clip(&distortion), 0.05);
    assert_changed(&mut distortion, FxParamId::MixPct, 120.0);
    assert_eq!(distortion_mix(&distortion), 1.0);

    let mut bitcrusher = FxBusParams::Bitcrusher {
        rate_div: 4,
        bits: 6,
        mix: 0.5,
    };
    assert_changed(&mut bitcrusher, FxParamId::Bits, 0.4);
    assert_eq!(bitcrusher_bits(&bitcrusher), 1);
    assert_changed(&mut bitcrusher, FxParamId::RateDiv, 129.0);
    assert_eq!(bitcrusher_rate_div(&bitcrusher), 128);
    assert_changed(&mut bitcrusher, FxParamId::MixPct, 120.0);
    assert_eq!(bitcrusher_mix(&bitcrusher), 1.0);

    let mut compressor = FxBusParams::Compressor {
        threshold_db: -24.0,
        ratio: 4.0,
        attack_ms: 10.0,
        release_ms: 100.0,
        makeup_db: 0.0,
        mix: 0.5,
    };
    assert_changed(&mut compressor, FxParamId::ThresholdDb, -100.0);
    assert_eq!(compressor_threshold(&compressor), -60.0);
    assert_changed(&mut compressor, FxParamId::Ratio, 30.0);
    assert_eq!(compressor_ratio(&compressor), 20.0);
    assert_changed(&mut compressor, FxParamId::AttackMs, 0.0);
    assert_eq!(compressor_attack(&compressor), 0.1);
    assert_changed(&mut compressor, FxParamId::ReleaseMs, 3_000.0);
    assert_eq!(compressor_release(&compressor), 2_000.0);
    assert_changed(&mut compressor, FxParamId::MakeupDb, 30.0);
    assert_eq!(compressor_makeup(&compressor), 24.0);
    assert_changed(&mut compressor, FxParamId::MixPct, 120.0);
    assert_eq!(compressor_mix(&compressor), 1.0);

    let mut eq = FxBusParams::Eq {
        low_gain_db: 0.0,
        mid_gain_db: 0.0,
        mid_freq_hz: 1_000.0,
        mid_q: 1.0,
        high_gain_db: 0.0,
        mix: 0.5,
    };
    assert_changed(&mut eq, FxParamId::LowGainDb, -20.0);
    assert_eq!(eq_low_gain(&eq), -12.0);
    assert_changed(&mut eq, FxParamId::MidGainDb, 20.0);
    assert_eq!(eq_mid_gain(&eq), 12.0);
    assert_changed(&mut eq, FxParamId::MidFreqHz, 0.0);
    assert_eq!(eq_mid_freq(&eq), 40.0);
    assert_changed(&mut eq, FxParamId::MidQ, 0.0);
    assert_eq!(eq_mid_q(&eq), 0.25);
    assert_changed(&mut eq, FxParamId::HighGainDb, 20.0);
    assert_eq!(eq_high_gain(&eq), 12.0);
    assert_changed(&mut eq, FxParamId::MixPct, 120.0);
    assert_eq!(eq_mix(&eq), 1.0);

    let mut vinyl = FxBusParams::Vinyl {
        saturation: 0.1,
        crackle: 0.1,
        warp_depth: 0.1,
        mix: 0.5,
    };
    assert_changed(&mut vinyl, FxParamId::SaturationPct, 120.0);
    assert_eq!(vinyl_saturation(&vinyl), 1.0);
    assert_changed(&mut vinyl, FxParamId::CracklePct, -1.0);
    assert_eq!(vinyl_crackle(&vinyl), 0.0);
    assert_changed(&mut vinyl, FxParamId::WarpDepthPct, 120.0);
    assert_eq!(vinyl_warp(&vinyl), 1.0);
    assert_changed(&mut vinyl, FxParamId::MixPct, 120.0);
    assert_eq!(vinyl_mix(&vinyl), 1.0);
}

fn assert_changed(params: &mut FxBusParams, id: FxParamId, value: f32) {
    assert_eq!(apply_fx_param(params, id, value), FxParamMutation::Changed);
}

fn assert_rejected(params: &mut FxBusParams, id: FxParamId) {
    assert_eq!(apply_fx_param(params, id, 1.0), FxParamMutation::Rejected);
}

macro_rules! fx_field {
    ($params:expr, $pattern:pat => $value:expr) => {{
        let $pattern = $params else { unreachable!() };
        $value
    }};
}

fn tremolo_rate(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Tremolo { rate_hz, .. } => *rate_hz)
}
fn tremolo_depth(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Tremolo { depth, .. } => *depth)
}
fn delay_feedback(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Delay { feedback, .. } => *feedback)
}
fn delay_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Delay { mix, .. } => *mix)
}
fn delay_spread(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Delay { spread, .. } => *spread)
}
fn mod_rate(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::ModDelay { rate_hz, .. } => *rate_hz)
}
fn mod_feedback(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::ModDelay { feedback, .. } => *feedback)
}
fn mod_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::ModDelay { mix, .. } => *mix)
}
fn filter_rate(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::FilterLfo { rate_hz, .. } => *rate_hz)
}
fn filter_center(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::FilterLfo { center_hz, .. } => *center_hz)
}
fn filter_depth(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::FilterLfo { depth, .. } => *depth)
}
fn filter_q(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::FilterLfo { q, .. } => *q)
}
fn reverb_decay(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Reverb { decay, .. } => *decay)
}
fn reverb_damp(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Reverb { damp, .. } => *damp)
}
fn reverb_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Reverb { mix, .. } => *mix)
}
fn glitch_chance(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Glitch { chance, .. } => *chance)
}
fn glitch_slice(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Glitch { slice_ms, .. } => *slice_ms)
}
fn glitch_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Glitch { mix, .. } => *mix)
}
fn auto_pan_rate(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::AutoPan { rate_hz, .. } => *rate_hz)
}
fn auto_pan_depth(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::AutoPan { depth, .. } => *depth)
}
fn duck_threshold(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Duck { threshold, .. } => *threshold)
}
fn duck_amount(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Duck { amount, .. } => *amount)
}
fn duck_attack(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Duck { attack_ms, .. } => *attack_ms)
}
fn duck_release(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Duck { release_ms, .. } => *release_ms)
}
fn saturator_drive(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Saturator { drive, .. } => *drive)
}
fn saturator_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Saturator { mix, .. } => *mix)
}
fn distortion_drive(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Distortion { drive, .. } => *drive)
}
fn distortion_clip(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Distortion { clip, .. } => *clip)
}
fn distortion_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Distortion { mix, .. } => *mix)
}
fn bitcrusher_bits(params: &FxBusParams) -> u32 {
    fx_field!(params, FxBusParams::Bitcrusher { bits, .. } => *bits)
}
fn bitcrusher_rate_div(params: &FxBusParams) -> u32 {
    fx_field!(params, FxBusParams::Bitcrusher { rate_div, .. } => *rate_div)
}
fn bitcrusher_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Bitcrusher { mix, .. } => *mix)
}
fn compressor_threshold(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { threshold_db, .. } => *threshold_db)
}
fn compressor_ratio(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { ratio, .. } => *ratio)
}
fn compressor_attack(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { attack_ms, .. } => *attack_ms)
}
fn compressor_release(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { release_ms, .. } => *release_ms)
}
fn compressor_makeup(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { makeup_db, .. } => *makeup_db)
}
fn compressor_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Compressor { mix, .. } => *mix)
}
fn eq_low_gain(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { low_gain_db, .. } => *low_gain_db)
}
fn eq_mid_gain(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { mid_gain_db, .. } => *mid_gain_db)
}
fn eq_mid_freq(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { mid_freq_hz, .. } => *mid_freq_hz)
}
fn eq_mid_q(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { mid_q, .. } => *mid_q)
}
fn eq_high_gain(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { high_gain_db, .. } => *high_gain_db)
}
fn eq_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Eq { mix, .. } => *mix)
}
fn vinyl_saturation(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Vinyl { saturation, .. } => *saturation)
}
fn vinyl_crackle(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Vinyl { crackle, .. } => *crackle)
}
fn vinyl_warp(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Vinyl { warp_depth, .. } => *warp_depth)
}
fn vinyl_mix(params: &FxBusParams) -> f32 {
    fx_field!(params, FxBusParams::Vinyl { mix, .. } => *mix)
}
