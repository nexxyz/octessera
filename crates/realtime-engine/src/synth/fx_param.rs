use super::fx_params::FxBusParams;
use serde::{Deserialize, Serialize};

pub use super::scalar_param::ScalarMutation as FxParamMutation;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FxParamId {
    RateHz,
    DepthPct,
    Feedback,
    MixPct,
    SpreadPct,
    CenterHz,
    Q,
    Decay,
    Damp,
    ChancePct,
    SliceMs,
    Threshold,
    AmountPct,
    AttackMs,
    ReleaseMs,
    Drive,
    Clip,
    Bits,
    RateDiv,
    ThresholdDb,
    Ratio,
    MakeupDb,
    LowGainDb,
    MidGainDb,
    MidFreqHz,
    MidQ,
    HighGainDb,
    SaturationPct,
    CracklePct,
    WarpDepthPct,
}

impl FxParamId {
    pub const ALL: [Self; 30] = [
        Self::RateHz,
        Self::DepthPct,
        Self::Feedback,
        Self::MixPct,
        Self::SpreadPct,
        Self::CenterHz,
        Self::Q,
        Self::Decay,
        Self::Damp,
        Self::ChancePct,
        Self::SliceMs,
        Self::Threshold,
        Self::AmountPct,
        Self::AttackMs,
        Self::ReleaseMs,
        Self::Drive,
        Self::Clip,
        Self::Bits,
        Self::RateDiv,
        Self::ThresholdDb,
        Self::Ratio,
        Self::MakeupDb,
        Self::LowGainDb,
        Self::MidGainDb,
        Self::MidFreqHz,
        Self::MidQ,
        Self::HighGainDb,
        Self::SaturationPct,
        Self::CracklePct,
        Self::WarpDepthPct,
    ];
}

pub(super) fn apply_fx_param(
    params: &mut FxBusParams,
    id: FxParamId,
    value: f32,
) -> FxParamMutation {
    if !value.is_finite() {
        return FxParamMutation::Rejected;
    }
    match params {
        FxBusParams::None => FxParamMutation::Rejected,
        FxBusParams::Tremolo { rate_hz, depth } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value.clamp(0.05, 40.0)),
            FxParamId::DepthPct => set_f32(depth, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Delay {
            feedback,
            mix,
            spread,
            ..
        } => match id {
            FxParamId::Feedback => set_f32(feedback, value.clamp(0.0, 0.98)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            FxParamId::SpreadPct => set_f32(spread, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::ModDelay {
            rate_hz,
            feedback,
            mix,
            ..
        } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value.clamp(0.02, 20.0)),
            FxParamId::Feedback => set_f32(feedback, value.clamp(-0.95, 0.95)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::FilterLfo {
            rate_hz,
            depth,
            center_hz,
            q,
            ..
        } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value.clamp(0.02, 20.0)),
            FxParamId::CenterHz => set_f32(center_hz, value.clamp(40.0, 12_000.0)),
            FxParamId::DepthPct => set_f32(depth, pct(value)),
            FxParamId::Q => set_f32(q, value.clamp(0.25, 20.0)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Reverb { mix, decay, damp } => match id {
            FxParamId::Decay => set_f32(decay, value.clamp(0.0, 0.995)),
            FxParamId::Damp => set_f32(damp, value.clamp(0.0, 0.98)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => match id {
            FxParamId::ChancePct => set_f32(chance, pct(value)),
            FxParamId::SliceMs => set_f32(slice_ms, value.clamp(5.0, 500.0)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::AutoPan { rate_hz, depth } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value.clamp(0.02, 20.0)),
            FxParamId::DepthPct => set_f32(depth, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Duck {
            threshold,
            amount,
            attack_ms,
            release_ms,
            ..
        } => match id {
            FxParamId::Threshold => set_f32(threshold, value.clamp(0.0, 1.0)),
            FxParamId::AmountPct => set_f32(amount, pct(value)),
            FxParamId::AttackMs => set_f32(attack_ms, value.clamp(1.0, 500.0)),
            FxParamId::ReleaseMs => set_f32(release_ms, value.clamp(1.0, 5000.0)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Saturator { drive, mix } => match id {
            FxParamId::Drive => set_f32(drive, value.clamp(0.0, 20.0)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Distortion { drive, clip, mix } => match id {
            FxParamId::Drive => set_f32(drive, value.clamp(0.0, 50.0)),
            FxParamId::Clip => set_f32(clip, value.clamp(0.05, 2.0)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Bitcrusher {
            rate_div,
            bits,
            mix,
        } => match id {
            FxParamId::Bits => set_u32(bits, value.round().clamp(1.0, 16.0) as u32),
            FxParamId::RateDiv => set_u32(rate_div, value.round().clamp(1.0, 128.0) as u32),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => match id {
            FxParamId::ThresholdDb => set_f32(threshold_db, value.clamp(-60.0, 0.0)),
            FxParamId::Ratio => set_f32(ratio, value.clamp(1.0, 20.0)),
            FxParamId::AttackMs => set_f32(attack_ms, value.clamp(0.1, 200.0)),
            FxParamId::ReleaseMs => set_f32(release_ms, value.clamp(1.0, 2000.0)),
            FxParamId::MakeupDb => set_f32(makeup_db, value.clamp(0.0, 24.0)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => match id {
            FxParamId::LowGainDb => set_f32(low_gain_db, value.clamp(-12.0, 12.0)),
            FxParamId::MidGainDb => set_f32(mid_gain_db, value.clamp(-12.0, 12.0)),
            FxParamId::MidFreqHz => set_f32(mid_freq_hz, value.clamp(40.0, 8000.0)),
            FxParamId::MidQ => set_f32(mid_q, value.clamp(0.25, 20.0)),
            FxParamId::HighGainDb => set_f32(high_gain_db, value.clamp(-12.0, 12.0)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => match id {
            FxParamId::SaturationPct => set_f32(saturation, pct(value)),
            FxParamId::CracklePct => set_f32(crackle, pct(value)),
            FxParamId::WarpDepthPct => set_f32(warp_depth, pct(value)),
            FxParamId::MixPct => set_f32(mix, pct(value)),
            _ => FxParamMutation::Rejected,
        },
    }
}

fn pct(value: f32) -> f32 {
    (value / 100.0).clamp(0.0, 1.0)
}

fn set_f32(current: &mut f32, next: f32) -> FxParamMutation {
    if current.to_bits() == next.to_bits() {
        FxParamMutation::Unchanged
    } else {
        *current = next;
        FxParamMutation::Changed
    }
}

fn set_u32(current: &mut u32, next: u32) -> FxParamMutation {
    if *current == next {
        FxParamMutation::Unchanged
    } else {
        *current = next;
        FxParamMutation::Changed
    }
}
