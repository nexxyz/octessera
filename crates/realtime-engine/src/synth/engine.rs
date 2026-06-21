use super::fx::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, process_fx_bus_slot, process_master_fx_slot, FxBusState,
    MasterFxState,
};
use super::fx_params::{compile_fx_bus_params, FxBusParams};
use super::runtime_state::*;
use super::types::*;
use serde_json::Value;
use std::collections::BTreeMap;

mod control;
mod render;
mod render_voice;
mod support;
#[cfg(test)]
mod test_support;
mod voice_budget;

use support::{
    midi_note_to_hz, mono_frame, osc_sample, pan_gains, pan_gains_float, param_f32,
    parse_instrument_kind, parse_momentary_fx_kind, parse_route, sample_slot_for_note,
    InstrumentKind, MomentaryFxKind, MomentaryFxState, PreviewSampleVoice, SampleVoice,
    DRY_HISTORY_FRAMES,
};

#[cfg(test)]
pub(in crate::synth) const FREEZE_INJECT_MS: u32 = support::FREEZE_INJECT_MS;

pub struct SynthEngine {
    sample_rate: u32,
    sample_clock: u64,
    slot_kind: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
    instruments: [SynthConfig; INSTRUMENT_SLOT_COUNT],
    sample_banks: Vec<SampleBankConfig>,
    mods: [InstrumentMod; INSTRUMENT_SLOT_COUNT],
    voices: [[Voice; VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
    sample_voices: [[SampleVoice; VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
    preview_sample_voices: Vec<PreviewSampleVoice>,
    slot_route: [usize; INSTRUMENT_SLOT_COUNT],
    slot_pan_pos: [usize; INSTRUMENT_SLOT_COUNT],
    slot_volume: [f32; INSTRUMENT_SLOT_COUNT],
    bus_pan_pos: Vec<usize>,
    bus_mono_scratch: Vec<f32>,
    bus_mono_snapshot: Vec<f32>,
    bus_slot_params: Vec<[FxBusParams; BUS_SLOTS_PER_BUS]>,
    bus_slot_state: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    master_slot_params: Vec<FxBusParams>,
    master_slot_state: Vec<MasterFxState>,
    pan_positions: usize,
    master_volume: f32,
    voice_stealing_mode: VoiceStealingMode,
    smoothed_load_ratio: f32,
    voice_steal_since_status: bool,
    momentary_fx: Vec<MomentaryFxState>,
    dry_history: Vec<f32>,
    dry_history_pos: usize,
}

impl SynthEngine {
    pub fn new(sample_rate: u32) -> Self {
        let default = default_synth_config();
        Self {
            sample_rate,
            sample_clock: 0,
            slot_kind: [InstrumentKind::Synth; INSTRUMENT_SLOT_COUNT],
            instruments: [default; INSTRUMENT_SLOT_COUNT],
            sample_banks: vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT],
            mods: [InstrumentMod::new(); INSTRUMENT_SLOT_COUNT],
            voices: [[Voice::off(); VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
            sample_voices: [[SampleVoice::off(); VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
            preview_sample_voices: Vec::new(),
            slot_route: [0; INSTRUMENT_SLOT_COUNT],
            slot_pan_pos: [DEFAULT_PAN_POSITIONS / 2; INSTRUMENT_SLOT_COUNT],
            slot_volume: [1.0; INSTRUMENT_SLOT_COUNT],
            bus_pan_pos: Vec::new(),
            bus_mono_scratch: Vec::new(),
            bus_mono_snapshot: Vec::new(),
            bus_slot_params: Vec::new(),
            bus_slot_state: Vec::new(),
            master_slot_params: Vec::new(),
            master_slot_state: Vec::new(),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 1.0,
            voice_stealing_mode: VoiceStealingMode::Balanced,
            smoothed_load_ratio: 0.0,
            voice_steal_since_status: false,
            momentary_fx: Vec::new(),
            dry_history: vec![0.0; DRY_HISTORY_FRAMES * 2],
            dry_history_pos: 0,
        }
    }
}
