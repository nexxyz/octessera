use super::{
    RuntimeAudioCommand, RuntimePlatformEffect, RuntimeStatus, RuntimeStoreResult, SyncSource,
};
use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    DeviceInput {
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
    RuntimeResult {
        result: RuntimeStoreResult,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeUiPulse {
    TransportFlash {
        flash: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
    TriggerPulse {
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerMessage {
    Snapshot { snapshot: Value },
    PlatformEffects { effects: Vec<RuntimePlatformEffect> },
    MusicalEvents { events: Vec<MusicalEvent> },
    AudioCommands { commands: Vec<RuntimeAudioCommand> },
    UiPulse { pulse: RuntimeUiPulse },
    RuntimeStatus { status: RuntimeStatus },
}
