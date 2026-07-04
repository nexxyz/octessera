use realtime_engine::synth::{InstrumentsConfig, SampleBankConfig, VoiceStealingMode};
use serde_json::Value;
use std::collections::BTreeMap;

pub enum EngineEvent {
    NoteOn {
        instrument_slot: u8,
        note: u8,
        velocity: u8,
        duration_ms: u32,
    },
    NoteOff {
        instrument_slot: u8,
        note: u8,
    },
    Cc {
        instrument_slot: u8,
        controller: u8,
        value: u8,
    },
    SetInstruments(InstrumentsConfig),
    SetSampleBanks(Vec<SampleBankConfig>),
    SetAudioConfig {
        instruments: InstrumentsConfig,
        sample_banks: Option<Vec<SampleBankConfig>>,
        voice_stealing_mode: Option<VoiceStealingMode>,
    },
    PreviewSample {
        instrument_slot: u8,
        buffer: realtime_engine::synth::SampleBuffer,
        velocity: u8,
    },
    SetVoiceStealingMode(VoiceStealingMode),
    SetMasterVolume {
        volume_pct: f32,
    },
    SetInstrumentMixer {
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    },
    SetInstrumentSlot {
        instrument_slot: usize,
        config: realtime_engine::synth::InstrumentSlotConfig,
    },
    SetFxBusMixer {
        bus_index: usize,
        pan_pos: Option<usize>,
    },
    SetSynthParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetSampleBankParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetFxBusSlot {
        bus_index: usize,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
    SetGlobalFxSlot {
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStart {
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: realtime_engine::synth::MomentaryFxTarget,
    },
    MomentaryFxUpdate {
        id: String,
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
}
