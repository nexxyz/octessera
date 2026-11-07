use super::momentary_fx_state::{
    parse_momentary_fx_kind, MomentaryFxKind, MomentaryFxRuntimeParams, MomentaryFxState,
};
use super::momentary_fx_voice::{prepare_voice_update, voice_param_keys};
use super::*;

#[derive(Clone)]
pub struct PreparedMomentaryFxStart {
    pub(super) state: MomentaryFxState,
}

impl PreparedMomentaryFxStart {
    pub fn epoch(&self) -> u64 {
        self.state.epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreparedMomentaryFxUpdate {
    Stutter {
        epoch: u64,
        depth: f32,
        segment_len: usize,
    },
    Freeze {
        epoch: u64,
        mix: f32,
        release_len: u32,
    },
    FilterSweep {
        epoch: u64,
        target_cutoff: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    PitchShift {
        epoch: u64,
        target_octaves: f32,
        mix: f32,
        slide_in_len: u32,
        slide_out_len: u32,
    },
    TapeStop {
        epoch: u64,
        stop_len: u32,
        start_len: u32,
    },
    Reverse {
        epoch: u64,
        window_len: u32,
        mix: f32,
    },
    TranceGate {
        epoch: u64,
        period: f32,
        depth: f32,
        smooth: f32,
    },
    DubThrow {
        epoch: u64,
        delay_len: f32,
        feedback: f32,
        mix: f32,
    },
    Wash {
        epoch: u64,
        feedback: f32,
        mix: f32,
        release_len: u32,
    },
    HpRiser {
        epoch: u64,
        target_octaves: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    Crush {
        epoch: u64,
        bits: u32,
        rate_div: u32,
        mix: f32,
    },
}

impl PreparedMomentaryFxUpdate {
    pub fn epoch(self) -> u64 {
        match self {
            Self::Stutter { epoch, .. }
            | Self::Freeze { epoch, .. }
            | Self::FilterSweep { epoch, .. }
            | Self::PitchShift { epoch, .. }
            | Self::TapeStop { epoch, .. }
            | Self::Reverse { epoch, .. }
            | Self::TranceGate { epoch, .. }
            | Self::DubThrow { epoch, .. }
            | Self::Wash { epoch, .. }
            | Self::HpRiser { epoch, .. }
            | Self::Crush { epoch, .. } => epoch,
        }
    }
}

pub fn prepare_momentary_fx_start(
    id: String,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    prepare_momentary_fx_start_with_epoch(id, 0, fx_type, params, target, sample_rate)
}

pub fn prepare_momentary_fx_start_with_epoch(
    id: String,
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    Some(PreparedMomentaryFxStart {
        state: MomentaryFxState::new_with_epoch(id, epoch, kind, &params, target, sample_rate),
    })
}

pub fn prepare_momentary_fx_update(
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxUpdate> {
    if sample_rate == 0 {
        return None;
    }
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    if let Some(update) = prepare_voice_update(kind, epoch, &params, sample_rate) {
        return Some(update);
    }
    let value = |key: &str, fallback: f32| strict_param_f32(&params, key, fallback);
    Some(match kind {
        MomentaryFxKind::Stutter => {
            let depth = (value("depthPct", 100.0)? / 100.0).clamp(0.0, 1.0);
            let rate = value("rateHz", 8.0)?.clamp(1.0, 32.0);
            PreparedMomentaryFxUpdate::Stutter {
                epoch,
                depth,
                segment_len: ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize),
            }
        }
        MomentaryFxKind::Freeze => PreparedMomentaryFxUpdate::Freeze {
            epoch,
            mix: (value("mixPct", 100.0)? / 100.0).clamp(0.0, 1.0),
            release_len: ms_to_samples(value("releaseMs", 500.0)?, sample_rate).max(1),
        },
        MomentaryFxKind::FilterSweep => {
            let cutoff_pct = (value("cutoffPct", 35.0)? / 100.0).clamp(0.0, 1.0);
            let resonance_pct = (value("resonancePct", 70.0)? / 100.0).clamp(0.0, 1.0);
            let in_len = ms_to_samples(value("sweepInMs", 200.0)?, sample_rate).max(1) as f32;
            let out_len = ms_to_samples(value("sweepOutMs", 500.0)?, sample_rate).max(1) as f32;
            PreparedMomentaryFxUpdate::FilterSweep {
                epoch,
                target_cutoff: 120.0 + cutoff_pct * 8_000.0,
                q: 0.5 + resonance_pct * 11.5,
                sweep_in_step: 1.0 / in_len,
                sweep_out_step: 1.0 / out_len,
            }
        }
        MomentaryFxKind::PitchShift => {
            let MomentaryFxRuntimeParams::PitchShift {
                target_octaves,
                mix,
                slide_in_len,
                slide_out_len,
            } = MomentaryFxRuntimeParams::from_params(kind, &params, sample_rate)
            else {
                unreachable!("pitch_shift kind must produce pitch parameters")
            };
            PreparedMomentaryFxUpdate::PitchShift {
                epoch,
                target_octaves,
                mix,
                slide_in_len,
                slide_out_len,
            }
        }
        _ => return None,
    })
}

fn validate_momentary_params(
    kind: super::momentary_fx_state::MomentaryFxKind,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<()> {
    if sample_rate == 0 {
        return None;
    }
    for (key, value) in params {
        let allowed = match kind {
            super::momentary_fx_state::MomentaryFxKind::Stutter => {
                matches!(key.as_str(), "depthPct" | "rateHz")
            }
            super::momentary_fx_state::MomentaryFxKind::Freeze => {
                matches!(key.as_str(), "mixPct" | "releaseMs")
            }
            super::momentary_fx_state::MomentaryFxKind::FilterSweep => matches!(
                key.as_str(),
                "cutoffPct" | "resonancePct" | "sweepInMs" | "sweepOutMs"
            ),
            super::momentary_fx_state::MomentaryFxKind::PitchShift => {
                matches!(
                    key.as_str(),
                    "semitones" | "cents" | "mixPct" | "slideInMs" | "slideOutMs"
                )
            }
            _ => voice_param_keys(kind).is_some_and(|keys| keys.contains(&key.as_str())),
        };
        if !allowed
            || !value
                .as_f64()
                .map(|value| (value as f32).is_finite())
                .unwrap_or(false)
        {
            return None;
        }
    }
    Some(())
}

fn strict_param_f32(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> Option<f32> {
    params
        .get(key)
        .map(|value| value.as_f64().map(|value| value as f32))
        .unwrap_or(Some(fallback))
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod shipped_stamp_tests;
