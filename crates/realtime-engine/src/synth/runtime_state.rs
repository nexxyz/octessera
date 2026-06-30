mod envelope;
mod filter;

pub(super) use envelope::{ms_to_samples, EnvStage, EnvState};
pub(super) use filter::BiquadState;

#[derive(Clone, Copy, Debug)]
pub(super) struct Voice {
    pub(super) active: bool,
    pub(super) instrument_slot: u8,
    pub(super) midi_note: u8,
    pub(super) velocity: u8,
    pub(super) velocity_norm: f32,
    pub(super) note_off_sample: u64,
    pub(super) started_sample: u64,
    pub(super) freq_hz: f32,
    pub(super) osc1_inc: f32,
    pub(super) osc2_inc: f32,
    pub(super) render_revision: u32,
    pub(super) phase1: f32,
    pub(super) phase2: f32,
    pub(super) amp_env: EnvState,
    pub(super) filt_env: EnvState,
    pub(super) filt: BiquadState,
}

impl Voice {
    pub(super) fn off() -> Self {
        Self {
            active: false,
            instrument_slot: 0,
            midi_note: 0,
            velocity: 0,
            velocity_norm: 0.0,
            note_off_sample: 0,
            started_sample: 0,
            freq_hz: 440.0,
            osc1_inc: 0.0,
            osc2_inc: 0.0,
            render_revision: 0,
            phase1: 0.0,
            phase2: 0.0,
            amp_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt_env: EnvState {
                stage: EnvStage::Off,
                level: 0.0,
                stage_pos: 0,
                stage_len: 0,
                sustain: 0.0,
                release_start: 0.0,
            },
            filt: BiquadState::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InstrumentMod {
    pub(super) cutoff_cc: f32,
    pub(super) resonance_cc: f32,
}

impl InstrumentMod {
    pub(super) fn new() -> Self {
        Self {
            cutoff_cc: 0.0,
            resonance_cc: 0.0,
        }
    }
}
