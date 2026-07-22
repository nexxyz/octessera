use realtime_engine::synth::{
    InstrumentsConfig, PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot,
    PreparedInstrumentSlot, PreparedInstrumentsConfig, PreparedMomentaryFxStart, SampleBankConfig,
    VoiceStealingMode,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::time::Instant;

#[derive(Clone)]
pub enum EngineEvent {
    AllNotesOff,
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
    SetSampleBank {
        instrument_slot: usize,
        bank: SampleBankConfig,
    },
    SetAudioConfig {
        instruments: InstrumentsConfig,
        sample_banks: Option<Vec<SampleBankConfig>>,
        voice_stealing_mode: Option<VoiceStealingMode>,
    },
    SetPreparedInstruments(PreparedInstrumentsConfig),
    SetPreparedAudioConfig(PreparedAudioConfig),
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
    SetPreparedInstrumentSlot {
        instrument_slot: usize,
        config: PreparedInstrumentSlot,
    },
    SetFxBusMixer {
        bus_index: usize,
        pan_pos: Option<usize>,
        volume_pct: Option<f32>,
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
    SetPreparedFxBusSlot {
        bus_index: usize,
        slot_index: usize,
        config: PreparedFxBusSlot,
    },
    SetGlobalFxSlot {
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
    SetPreparedGlobalFxSlot {
        slot_index: usize,
        config: PreparedGlobalFxSlot,
    },
    MomentaryFxStart {
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: realtime_engine::synth::MomentaryFxTarget,
    },
    PreparedMomentaryFxStart(PreparedMomentaryFxStart),
    MomentaryFxUpdate {
        id: String,
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
    ProbeMark {
        sent_at: Instant,
        report_tx: Sender<u128>,
    },
}
