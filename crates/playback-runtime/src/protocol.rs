use platform_core::MusicalEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    Internal,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTransportState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatusState {
    Idle,
    Running,
    Paused,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub state: RuntimeStatusState,
    pub transport: RuntimeTransportState,
    #[serde(rename = "currentPpqnPulse")]
    pub current_ppqn_pulse: u64,
    #[serde(rename = "pendingResync")]
    pub pending_resync: bool,
    #[serde(rename = "syncSource")]
    pub sync_source: SyncSource,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeMomentaryFxTarget {
    Global,
    FxBus { index: usize },
    Instrument { index: usize },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeAudioCommand {
    MomentaryFxStart {
        id: String,
        #[serde(rename = "fxType")]
        fx_type: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
        target: RuntimeMomentaryFxTarget,
    },
    MomentaryFxUpdate {
        id: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
    SamplePreview {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        path: String,
        velocity: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimePlatformEffect {
    StoreListPresets,
    StoreLoadPreset {
        name: String,
    },
    StoreSavePreset {
        name: String,
        payload: Value,
        #[serde(default)]
        mode: Option<String>,
    },
    StoreDeletePreset {
        name: String,
    },
    StoreLoadDefault,
    StoreSaveDefault {
        payload: Value,
        #[serde(default)]
        mode: Option<String>,
    },
    MidiListOutputsRequest,
    MidiListInputsRequest,
    MidiSelectOutput {
        id: Option<String>,
    },
    MidiSelectInput {
        id: Option<String>,
    },
    MidiPanic,
    Shutdown,
    SampleListRequest {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        dir: String,
    },
    AudioCommand {
        command: RuntimeAudioCommand,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeStoreResult {
    ListPresetsResult {
        names: Vec<String>,
    },
    LoadPresetResult {
        name: String,
        payload: Option<Value>,
    },
    SavePresetResult {
        name: String,
        outcome: String,
    },
    DeletePresetResult {
        name: String,
        ok: bool,
    },
    LoadDefaultResult {
        payload: Option<Value>,
    },
    SaveDefaultResult {
        ok: bool,
        #[serde(default, rename = "isAuto")]
        is_auto: Option<bool>,
    },
    StoreError {
        message: String,
    },
    MidiListOutputsResult {
        outputs: Vec<MidiPort>,
    },
    MidiListInputsResult {
        inputs: Vec<MidiPort>,
    },
    MidiStatus {
        ok: bool,
        #[serde(default)]
        message: Option<String>,
        #[serde(default, rename = "selectedOutId")]
        selected_out_id: Option<String>,
        #[serde(default, rename = "selectedInId")]
        selected_in_id: Option<String>,
    },
    SampleListResult {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        dir: String,
        entries: Vec<SampleEntry>,
    },
    SampleListError {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        dir: String,
        message: String,
    },
    SamplePreviewError {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPort {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
}

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
