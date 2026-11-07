use super::momentary_fx_state::{parse_momentary_fx_kind, MomentaryFxKind, MomentaryFxState};
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
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    Some(prepare_voice_update(kind, epoch, &params, sample_rate))
}

fn validate_momentary_params(
    kind: MomentaryFxKind,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<()> {
    if sample_rate == 0 {
        return None;
    }
    let keys = voice_param_keys(kind);
    params
        .iter()
        .all(|(key, value)| {
            keys.contains(&key.as_str())
                && value
                    .as_f64()
                    .is_some_and(|value| (value as f32).is_finite())
        })
        .then_some(())
}

#[cfg(test)]
mod shipped_stamp_tests;
