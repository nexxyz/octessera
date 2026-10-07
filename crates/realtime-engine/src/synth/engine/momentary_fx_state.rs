use super::live_pitch_shift::{
    pitch_activation_ramp_len, LivePitchShift, PITCH_DEFAULT_SLIDE_IN_MS,
    PITCH_DEFAULT_SLIDE_OUT_MS,
};
use super::support::param_f32;
use super::*;

pub(super) const FREEZE_INJECT_MS: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MomentaryFxKind {
    Stutter,
    Freeze,
    FilterSweep,
    PitchShift,
}

#[derive(Clone)]
pub(super) struct MomentaryFxState {
    pub(super) id: String,
    pub(super) epoch: u64,
    pub(super) kind: MomentaryFxKind,
    pub(super) runtime_params: MomentaryFxRuntimeParams,
    pub(super) target: MomentaryFxTarget,
    pub(super) releasing: bool,
    pub(super) release_pos: u32,
    pub(super) release_len: u32,
    pub(super) sweep_pos: f32,
    pub(super) filt_l: BiquadState,
    pub(super) filt_r: BiquadState,
    pub(super) pitch_shifter: LivePitchShift,
    pub(super) pitch_fill_pos: u32,
    pub(super) pitch_ramp_pos: u32,
    pub(super) pitch_ramp_len: u32,
    pub(super) pitch_amount_octaves: f32,
    pub(super) pitch_slide_start_octaves: f32,
    pub(super) pitch_slide_target_octaves: f32,
    pub(super) pitch_slide_pos: u32,
    pub(super) pitch_slide_len: u32,
    pub(super) stutter_l: Vec<f32>,
    pub(super) stutter_r: Vec<f32>,
    pub(super) stutter_write: usize,
    pub(super) stutter_ready: bool,
    pub(super) stutter_segment_len: usize,
    pub(super) stutter_ramp_len: usize,
    pub(super) stutter_ramp_pos: usize,
    pub(super) freeze_bufs: [Vec<f32>; 4],
    pub(super) freeze_idxs: [usize; 4],
    pub(super) freeze_lp: [f32; 4],
    pub(super) freeze_inject_pos: u32,
    pub(super) freeze_inject_len: u32,
    pub(super) freeze_ready_len: u32,
    pub(super) freeze_activation_pos: u32,
    pub(super) freeze_activation_len: u32,
}

#[derive(Clone, Copy)]
pub(super) enum MomentaryFxRuntimeParams {
    Stutter {
        depth: f32,
    },
    Freeze {
        mix: f32,
        release_len: u32,
    },
    FilterSweep {
        target_cutoff: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    PitchShift {
        target_octaves: f32,
        mix: f32,
        slide_in_len: u32,
        slide_out_len: u32,
    },
}

impl MomentaryFxRuntimeParams {
    pub(super) fn from_params(
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        sample_rate: u32,
    ) -> Self {
        match kind {
            MomentaryFxKind::Stutter => Self::Stutter {
                depth: (param_f32(params, "depthPct", 100.0) / 100.0).clamp(0.0, 1.0),
            },
            MomentaryFxKind::Freeze => Self::Freeze {
                mix: (param_f32(params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0),
                release_len: ms_to_samples(param_f32(params, "releaseMs", 500.0), sample_rate)
                    .max(1),
            },
            MomentaryFxKind::FilterSweep => {
                let cutoff_pct = (param_f32(params, "cutoffPct", 35.0) / 100.0).clamp(0.0, 1.0);
                let resonance_pct =
                    (param_f32(params, "resonancePct", 70.0) / 100.0).clamp(0.0, 1.0);
                let in_len =
                    ms_to_samples(param_f32(params, "sweepInMs", 200.0), sample_rate).max(1) as f32;
                let out_len = ms_to_samples(param_f32(params, "sweepOutMs", 500.0), sample_rate)
                    .max(1) as f32;
                Self::FilterSweep {
                    target_cutoff: 120.0 + cutoff_pct * 8_000.0,
                    q: 0.5 + resonance_pct * 11.5,
                    sweep_in_step: 1.0 / in_len,
                    sweep_out_step: 1.0 / out_len,
                }
            }
            MomentaryFxKind::PitchShift => {
                let semitones = param_f32(params, "semitones", 7.0).clamp(-24.0, 24.0);
                let cents = param_f32(params, "cents", 0.0).clamp(-100.0, 100.0);
                let mix = (param_f32(params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0);
                let slide_in_len = ms_to_samples(
                    param_f32(params, "slideInMs", PITCH_DEFAULT_SLIDE_IN_MS),
                    sample_rate,
                )
                .max(1);
                let slide_out_len = ms_to_samples(
                    param_f32(params, "slideOutMs", PITCH_DEFAULT_SLIDE_OUT_MS),
                    sample_rate,
                )
                .max(1);
                Self::PitchShift {
                    target_octaves: (semitones + cents / 100.0) / 12.0,
                    mix,
                    slide_in_len,
                    slide_out_len,
                }
            }
        }
    }
}

impl MomentaryFxState {
    pub(super) fn new(
        id: String,
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        Self::new_with_epoch(id, 0, kind, params, target, sample_rate)
    }

    pub(super) fn new_with_epoch(
        id: String,
        epoch: u64,
        kind: MomentaryFxKind,
        params: &BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        let ramp_samples = ((sample_rate as f32 * 0.002) as usize).max(1);
        let pitch_ramp_len = pitch_activation_ramp_len(sample_rate);
        let stutter_segment_len = stutter_segment_len(sample_rate, params);
        const DELAY_LENS: [usize; 4] = [1557, 1617, 1491, 1422];
        let freeze_bufs: [Vec<f32>; 4] =
            DELAY_LENS.map(|n| vec![0.0; (n * sample_rate as usize / 44_100).max(1)]);
        let freeze_inject_len = (sample_rate * FREEZE_INJECT_MS / 1000).max(1);
        let freeze_ready_len = freeze_bufs.iter().map(Vec::len).max().unwrap_or(1) as u32;
        let freeze_activation_len = (sample_rate * 5 / 1000).max(1);
        let runtime_params = MomentaryFxRuntimeParams::from_params(kind, params, sample_rate);
        let (pitch_slide_target_octaves, pitch_slide_len) = match runtime_params {
            MomentaryFxRuntimeParams::PitchShift {
                target_octaves,
                slide_in_len,
                ..
            } => (target_octaves, slide_in_len),
            _ => (0.0, 1),
        };
        Self {
            id,
            epoch,
            kind,
            runtime_params,
            target,
            releasing: false,
            release_pos: 0,
            release_len: 0,
            sweep_pos: 0.0,
            filt_l: BiquadState::new(),
            filt_r: BiquadState::new(),
            pitch_shifter: LivePitchShift::new(sample_rate),
            pitch_fill_pos: 0,
            pitch_ramp_pos: 0,
            pitch_ramp_len,
            pitch_amount_octaves: 0.0,
            pitch_slide_start_octaves: 0.0,
            pitch_slide_target_octaves,
            pitch_slide_pos: 0,
            pitch_slide_len,
            stutter_l: vec![0.0; sample_rate as usize],
            stutter_r: vec![0.0; sample_rate as usize],
            stutter_write: 0,
            stutter_ready: false,
            stutter_segment_len,
            stutter_ramp_len: ramp_samples,
            stutter_ramp_pos: 0,
            freeze_bufs,
            freeze_idxs: [0; 4],
            freeze_lp: [0.0; 4],
            freeze_inject_pos: 0,
            freeze_inject_len,
            freeze_ready_len,
            freeze_activation_pos: 0,
            freeze_activation_len,
        }
    }
}

pub(super) fn stutter_segment_len(sample_rate: u32, params: &BTreeMap<String, Value>) -> usize {
    let rate = param_f32(params, "rateHz", 8.0).clamp(1.0, 32.0);
    ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize)
}

pub(super) fn parse_momentary_fx_kind(kind: &str) -> Option<MomentaryFxKind> {
    match kind {
        "stutter" => Some(MomentaryFxKind::Stutter),
        "freeze" => Some(MomentaryFxKind::Freeze),
        "filter_sweep" => Some(MomentaryFxKind::FilterSweep),
        "pitch_shift" => Some(MomentaryFxKind::PitchShift),
        _ => None,
    }
}
