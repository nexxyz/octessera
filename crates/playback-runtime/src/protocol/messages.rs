use super::{
    RuntimeAudioCommand, RuntimePlatformEffect, RuntimeStatus, RuntimeStoreResult, SyncSource,
};
use crate::runtime::RuntimeConfig;
use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A grid cell playing an instrument voice: a Drum kit voice or a Sampler sample slot,
/// pitched by the cell's stamped offset. Velocity 0 releases the voices that cell started
/// (Sampler only; Drum hits are one-shot).
pub struct CellHit {
    pub instrument_slot: u8,
    pub voice: u8,
    pub cell: u8,
    pub pitch_cents: i16,
    pub velocity: u8,
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
    CellHits {
        hits: Vec<CellHit>,
    },
    MidiEvents {
        events: Vec<MusicalEvent>,
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
