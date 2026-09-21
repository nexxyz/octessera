use realtime_engine::synth::{
    DspRuntimeConfig, FxParamId, PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot,
    PreparedInstrumentSlot, PreparedInstrumentsConfig, PreparedMomentaryFxStart,
    PreparedMomentaryFxUpdate, SampleBankConfig, SampleBankParamId, SampleBuffer, SynthParamId,
    VoiceStealingMode,
};
use std::sync::mpsc::SyncSender;
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
    SetPreparedInstruments {
        generation: u64,
        config: PreparedInstrumentsConfig,
    },
    SetPreparedAudioConfig {
        generation: u64,
        config: PreparedAudioConfig,
    },
    SetPreparedSampleBank {
        instrument_slot: u8,
        generation: u64,
        bank: SampleBankConfig,
    },
    SetPreparedInstrumentOwner {
        instrument_slot: u8,
        generation: u64,
        config: PreparedInstrumentSlot,
        sample_bank: Option<SampleBankConfig>,
    },
    PreviewSample {
        instrument_slot: u8,
        generation: u64,
        buffer: SampleBuffer,
        velocity: u8,
    },
    SetVoiceStealingMode {
        generation: u64,
        mode: VoiceStealingMode,
    },
    SetDspConfig {
        generation: u64,
        config: DspRuntimeConfig,
    },
    SetMasterVolume {
        generation: u64,
        volume_pct: f32,
    },
    SetInstrumentMixer {
        instrument_slot: u8,
        generation: u64,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    },
    SetPreparedInstrumentSlot {
        instrument_slot: u8,
        generation: u64,
        config: PreparedInstrumentSlot,
    },
    SetFxBusMixer {
        bus_index: u8,
        generation: u64,
        pan_pos: Option<usize>,
        volume_pct: Option<f32>,
    },
    SetSynthParam {
        instrument_slot: u8,
        generation: u64,
        param: SynthParamId,
        value: f32,
    },
    SetSampleBankParam {
        instrument_slot: u8,
        generation: u64,
        param: SampleBankParamId,
        value: f32,
    },
    SetFxBusParam {
        bus_index: u8,
        slot_index: u8,
        generation: u64,
        param: FxParamId,
        value: f32,
    },
    SetPreparedFxBusSlot {
        bus_index: u8,
        slot_index: u8,
        generation: u64,
        config: PreparedFxBusSlot,
    },
    SetGlobalFxParam {
        slot_index: u8,
        generation: u64,
        param: FxParamId,
        value: f32,
    },
    SetPreparedGlobalFxSlot {
        slot_index: u8,
        generation: u64,
        config: PreparedGlobalFxSlot,
    },
    PreparedMomentaryFxStart {
        config: PreparedMomentaryFxStart,
    },
    MomentaryFxUpdate(PreparedMomentaryFxUpdate),
    MomentaryFxStop {
        epoch: u64,
    },
    ProbeMark {
        sent_at: Instant,
        report_tx: SyncSender<u128>,
    },
}
