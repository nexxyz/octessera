use super::{
    RuntimeAudioCommand, RuntimePlatformEffect, RuntimeStatus, RuntimeStoreResult, SyncSource,
};
use crate::runtime::RuntimeConfig;
use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A grid tile playing an instrument voice: a Drum kit voice or a Sampler sample slot,
/// pitched by the tile's stamped offset. Velocity 0 releases the voices that tile started
/// (Sampler only; Drum hits are one-shot).
pub struct TileHit {
    pub instrument_slot: u8,
    pub voice: u8,
    pub tile: u8,
    pub pitch_cents: i16,
    pub velocity: u8,
    /// The General MIDI note System > MIDI > Send MIDI: all inst. copies this hit to.
    pub midi_note: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    DeviceInput {
        input: Value,
        #[serde(default, rename = "requestSnapshot")]
        request_snapshot: Option<bool>,
    },
    PresentedRuntimeErrorInput {
        input: Value,
    },
    TransportPulseStep {
        pulses: u32,
        source: SyncSource,
        #[serde(default, rename = "atPpqnPulse")]
        at_ppqn_pulse: Option<u64>,
        #[serde(default, rename = "requestSnapshot")]
        request_snapshot: Option<bool>,
    },
    MidiRealtimeClock {
        pulses: u32,
    },
    MidiRealtimeStart,
    MidiRealtimeContinue,
    MidiRealtimeStop,
    /// A Control Change from the selected MIDI In; `channel` is 0-15.
    MidiControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    TransportStop,
    RuntimeResult {
        result: RuntimeStoreResult,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerMessage {
    Snapshot {
        snapshot: Value,
    },
    OledFrame {
        revision: u64,
        width: usize,
        height: usize,
        format: String,
        #[serde(rename = "pixelsBase64", with = "super::oled::base64_bytes")]
        pixels: Vec<u8>,
    },
    PlatformEffects {
        effects: Vec<RuntimePlatformEffect>,
    },
    MusicalEvents {
        events: Vec<MusicalEvent>,
    },
    TileHits {
        hits: Vec<TileHit>,
    },
    MidiEvents {
        events: Vec<MusicalEvent>,
    },
    /// The event messages that follow in this batch sound this long after the step's stamp.
    SequencedOffset {
        nanos: u64,
    },
    AudioCommands {
        commands: Vec<RuntimeAudioCommand>,
    },
    RuntimeStatus {
        status: RuntimeStatus,
    },
    RuntimeConfigChanged {
        config: RuntimeConfig,
    },
    PresentedRuntimeErrorDismissed,
}
