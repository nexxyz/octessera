use super::fx_params::{FilterLfoKind, FxBusParams, FxKind, ModDelayKind};
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

    const fn is_percent(self) -> bool {
        matches!(
            self,
            Self::DepthPct
                | Self::MixPct
                | Self::SpreadPct
                | Self::ChancePct
                | Self::AmountPct
                | Self::SaturationPct
                | Self::CracklePct
                | Self::WarpDepthPct
        )
    }
}

/// `(min, max, default)` in config units; percent params are 0..=100.
pub(super) const fn fx_param_range(kind: FxKind, id: FxParamId) -> Option<(f32, f32, f32)> {
    use FxKind as K;
    use FxParamId as P;
    Some(match (kind, id) {
        (K::Tremolo, P::RateHz) => (0.05, 40.0, 4.0),
        (K::Tremolo, P::DepthPct) => (0.0, 100.0, 60.0),
        (K::Delay, P::Feedback) => (0.0, 0.98, 0.35),
        (K::Delay, P::MixPct) => (0.0, 100.0, 35.0),
        (K::Delay, P::SpreadPct) => (0.0, 100.0, 0.0),
        (K::Vibrato | K::Chorus | K::Flanger, P::RateHz) => (0.02, 20.0, 0.8),
        (K::Vibrato | K::Chorus, P::Feedback) => (-0.95, 0.95, 0.0),
        (K::Flanger, P::Feedback) => (-0.95, 0.95, 0.35),
        (K::Vibrato, P::MixPct) => (0.0, 100.0, 100.0),
        (K::Chorus | K::Flanger, P::MixPct) => (0.0, 100.0, 45.0),
        (K::FilterLfo, P::RateHz) => (0.02, 20.0, 0.5),
        (K::Wah, P::RateHz) => (0.02, 20.0, 1.2),
        (K::FilterLfo | K::Wah, P::DepthPct) => (0.0, 100.0, 70.0),
        (K::FilterLfo, P::CenterHz) => (40.0, 12_000.0, 1600.0),
        (K::Wah, P::CenterHz) => (40.0, 12_000.0, 900.0),
        (K::FilterLfo, P::Q) => (0.25, 20.0, 1.0),
        (K::Wah, P::Q) => (0.25, 20.0, 6.0),
        (K::Reverb, P::MixPct) => (0.0, 100.0, 30.0),
        (K::Reverb, P::Decay) => (0.0, 0.995, 0.72),
        (K::Reverb, P::Damp) => (0.0, 0.98, 0.35),
        (K::Glitch, P::ChancePct) => (0.0, 100.0, 8.0),
        (K::Glitch, P::SliceMs) => (5.0, 500.0, 80.0),
        (K::AutoPan, P::RateHz) => (0.02, 20.0, 0.5),
        (K::AutoPan, P::DepthPct) => (0.0, 100.0, 100.0),
        (K::Duck, P::Threshold) => (0.0, 1.0, 0.08),
        (K::Duck, P::AmountPct) => (0.0, 100.0, 60.0),
        (K::Duck, P::AttackMs) => (1.0, 500.0, 8.0),
        (K::Duck, P::ReleaseMs) => (1.0, 5000.0, 160.0),
        (K::Saturator, P::Drive) => (0.0, 20.0, 1.8),
        (K::Distortion, P::Drive) => (0.0, 50.0, 2.5),
        (K::Distortion, P::Clip) => (0.05, 2.0, 0.6),
        (K::Bitcrusher, P::RateDiv) => (1.0, 128.0, 4.0),
        (K::Bitcrusher, P::Bits) => (1.0, 16.0, 6.0),
        (K::Compressor, P::ThresholdDb) => (-60.0, 0.0, -24.0),
        (K::Compressor, P::Ratio) => (1.0, 20.0, 4.0),
        (K::Compressor, P::AttackMs) => (1.0, 200.0, 10.0),
        (K::Compressor, P::ReleaseMs) => (5.0, 2000.0, 100.0),
        (K::Compressor, P::MakeupDb) => (0.0, 24.0, 0.0),
        (K::Eq, P::LowGainDb | P::MidGainDb | P::HighGainDb) => (-12.0, 12.0, 0.0),
        (K::Eq, P::MidFreqHz) => (40.0, 8000.0, 1000.0),
        (K::Eq, P::MidQ) => (0.25, 20.0, 1.0),
        (K::Vinyl, P::SaturationPct) => (0.0, 100.0, 15.0),
        (K::Vinyl, P::CracklePct) => (0.0, 100.0, 8.0),
        (K::Vinyl, P::WarpDepthPct) => (0.0, 100.0, 5.0),
        (
            K::Glitch
            | K::Saturator
            | K::Distortion
            | K::Bitcrusher
            | K::Compressor
            | K::Eq
            | K::Vinyl,
            P::MixPct,
        ) => (0.0, 100.0, 100.0),
        _ => return None,
    })
}

/// Clamps a config-unit value into its range; percent params come back as 0..=1.
pub(super) fn clamp_fx_param(id: FxParamId, (min, max, _): (f32, f32, f32), value: f32) -> f32 {
    let value = value.clamp(min, max);
    if id.is_percent() {
        value / 100.0
    } else {
        value
    }
}

pub(super) const fn ranged_fx_kind(params: &FxBusParams) -> FxKind {
    match params {
        FxBusParams::None => FxKind::None,
        FxBusParams::Tremolo { .. } => FxKind::Tremolo,
        FxBusParams::Delay { .. } => FxKind::Delay,
        FxBusParams::ModDelay {
            kind: ModDelayKind::Vibrato,
            ..
        } => FxKind::Vibrato,
        FxBusParams::ModDelay {
            kind: ModDelayKind::Chorus,
            ..
        } => FxKind::Chorus,
        FxBusParams::ModDelay {
            kind: ModDelayKind::Flanger,
            ..
        } => FxKind::Flanger,
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::FilterLfo,
            ..
        } => FxKind::FilterLfo,
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::Wah,
            ..
        } => FxKind::Wah,
        FxBusParams::Reverb { .. } => FxKind::Reverb,
        FxBusParams::Glitch { .. } => FxKind::Glitch,
        FxBusParams::AutoPan { .. } => FxKind::AutoPan,
        FxBusParams::Duck { .. } => FxKind::Duck,
        FxBusParams::Saturator { .. } => FxKind::Saturator,
        FxBusParams::Distortion { .. } => FxKind::Distortion,
        FxBusParams::Bitcrusher { .. } => FxKind::Bitcrusher,
        FxBusParams::Compressor { .. } => FxKind::Compressor,
        FxBusParams::Eq { .. } => FxKind::Eq,
        FxBusParams::Vinyl { .. } => FxKind::Vinyl,
    }
}

pub(super) fn apply_fx_param(
    params: &mut FxBusParams,
    id: FxParamId,
    value: f32,
) -> FxParamMutation {
    if !value.is_finite() {
        return FxParamMutation::Rejected;
    }
    let Some(range) = fx_param_range(ranged_fx_kind(params), id) else {
        return FxParamMutation::Rejected;
    };
    let value = clamp_fx_param(id, range, value);
    match params {
        FxBusParams::None => FxParamMutation::Rejected,
        FxBusParams::Tremolo { rate_hz, depth } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value),
            FxParamId::DepthPct => set_f32(depth, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Delay {
            feedback,
            mix,
            spread,
            ..
        } => match id {
            FxParamId::Feedback => set_f32(feedback, value),
            FxParamId::MixPct => set_f32(mix, value),
            FxParamId::SpreadPct => set_f32(spread, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::ModDelay {
            rate_hz,
            feedback,
            mix,
            ..
        } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value),
            FxParamId::Feedback => set_f32(feedback, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::FilterLfo {
            rate_hz,
            depth,
            center_hz,
            q,
            ..
        } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value),
            FxParamId::CenterHz => set_f32(center_hz, value),
            FxParamId::DepthPct => set_f32(depth, value),
            FxParamId::Q => set_f32(q, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Reverb { mix, decay, damp } => match id {
            FxParamId::Decay => set_f32(decay, value),
            FxParamId::Damp => set_f32(damp, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => match id {
            FxParamId::ChancePct => set_f32(chance, value),
            FxParamId::SliceMs => set_f32(slice_ms, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::AutoPan { rate_hz, depth } => match id {
            FxParamId::RateHz => set_f32(rate_hz, value),
            FxParamId::DepthPct => set_f32(depth, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Duck {
            threshold,
            amount,
            attack_ms,
            release_ms,
            ..
        } => match id {
            FxParamId::Threshold => set_f32(threshold, value),
            FxParamId::AmountPct => set_f32(amount, value),
            FxParamId::AttackMs => set_f32(attack_ms, value),
            FxParamId::ReleaseMs => set_f32(release_ms, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Saturator { drive, mix } => match id {
            FxParamId::Drive => set_f32(drive, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Distortion { drive, clip, mix } => match id {
            FxParamId::Drive => set_f32(drive, value),
            FxParamId::Clip => set_f32(clip, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Bitcrusher {
            rate_div,
            bits,
            mix,
        } => match id {
            FxParamId::Bits => set_u32(bits, value.round() as u32),
            FxParamId::RateDiv => set_u32(rate_div, value.round() as u32),
            FxParamId::MixPct => set_f32(mix, value),
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
            FxParamId::ThresholdDb => set_f32(threshold_db, value),
            FxParamId::Ratio => set_f32(ratio, value),
            FxParamId::AttackMs => set_f32(attack_ms, value),
            FxParamId::ReleaseMs => set_f32(release_ms, value),
            FxParamId::MakeupDb => set_f32(makeup_db, value),
            FxParamId::MixPct => set_f32(mix, value),
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
            FxParamId::LowGainDb => set_f32(low_gain_db, value),
            FxParamId::MidGainDb => set_f32(mid_gain_db, value),
            FxParamId::MidFreqHz => set_f32(mid_freq_hz, value),
            FxParamId::MidQ => set_f32(mid_q, value),
            FxParamId::HighGainDb => set_f32(high_gain_db, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => match id {
            FxParamId::SaturationPct => set_f32(saturation, value),
            FxParamId::CracklePct => set_f32(crackle, value),
            FxParamId::WarpDepthPct => set_f32(warp_depth, value),
            FxParamId::MixPct => set_f32(mix, value),
            _ => FxParamMutation::Rejected,
        },
    }
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
