use realtime_engine::synth::{
    PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot, PreparedInstrumentSlot,
    PreparedInstrumentsConfig, PreparedMomentaryFxStart, SampleBankConfig, VoiceStealingMode,
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
    SetPreparedInstruments(PreparedInstrumentsConfig),
    SetPreparedAudioConfig(PreparedAudioConfig),
    SetPreparedSampleBank {
        instrument_slot: usize,
        bank: SampleBankConfig,
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
    SetPreparedFxBusSlot {
        bus_index: usize,
        slot_index: usize,
        config: PreparedFxBusSlot,
    },
    SetPreparedGlobalFxSlot {
        slot_index: usize,
        config: PreparedGlobalFxSlot,
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
