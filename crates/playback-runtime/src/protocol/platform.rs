use super::RuntimeAudioCommand;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    StoreSaveBackup {
        payload: Value,
    },
    StoreSaveRecovery {
        payload: Value,
    },
    UsbApplyReboot {
        payload: Value,
    },
    RecordingStartAudio {
        #[serde(rename = "maxMinutes")]
        max_minutes: u16,
    },
    RecordingStop,
    MidiListOutputsRequest,
    MidiListInputsRequest,
    MidiSelectOutput {
        id: Option<String>,
    },
    MidiSelectInput {
        id: Option<String>,
    },
    MidiPanic,
    Reboot,
    Shutdown,
    HardwareTest,
    UpdateCheck,
    UpdateApply,
    Rollback,
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
    SaveBackupResult {
        ok: bool,
    },
    SaveRecoveryResult {
        ok: bool,
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
