use super::{
    RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
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
    UsbSdTransferStart,
    UsbSdTransferStop,
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
    SystemInfoRequest,
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

impl RuntimePlatformEffect {
    pub fn operation(&self) -> RuntimeOperation {
        match self {
            Self::StoreListPresets => RuntimeOperation::StoreListPresets,
            Self::StoreLoadPreset { .. } => RuntimeOperation::StoreLoadPreset,
            Self::StoreSavePreset { .. } => RuntimeOperation::StoreSavePreset,
            Self::StoreDeletePreset { .. } => RuntimeOperation::StoreDeletePreset,
            Self::StoreLoadDefault => RuntimeOperation::StoreLoadDefault,
            Self::StoreSaveDefault { .. } => RuntimeOperation::StoreSaveDefault,
            Self::StoreSaveBackup { .. } => RuntimeOperation::StoreSaveBackup,
            Self::StoreSaveRecovery { .. } => RuntimeOperation::StoreSaveRecovery,
            Self::MidiListOutputsRequest => RuntimeOperation::MidiListOutputs,
            Self::MidiListInputsRequest => RuntimeOperation::MidiListInputs,
            Self::MidiSelectOutput { .. } | Self::MidiSelectInput { .. } | Self::MidiPanic => {
                RuntimeOperation::MidiStatus
            }
            Self::SampleListRequest { .. } => RuntimeOperation::SampleList,
            Self::AudioCommand { .. } | Self::RecordingStartAudio { .. } | Self::RecordingStop => {
                RuntimeOperation::AudioCommand
            }
            Self::UpdateCheck | Self::UpdateApply | Self::Rollback => {
                RuntimeOperation::DeviceUpdate
            }
            Self::UsbApplyReboot { .. }
            | Self::UsbSdTransferStart
            | Self::UsbSdTransferStop
            | Self::Reboot
            | Self::Shutdown
            | Self::HardwareTest => RuntimeOperation::RuntimeDispatch,
            Self::SystemInfoRequest => RuntimeOperation::SystemInfo,
        }
    }

    pub fn failure_facts(&self, message: String) -> RuntimeErrorFacts {
        RuntimeErrorFacts::new(
            self.error_domain(),
            crate::RuntimeErrorCode::OperationFailed,
            self.operation(),
            Some(message),
        )
    }

    pub fn unsupported_facts(&self, message: String) -> RuntimeErrorFacts {
        RuntimeErrorFacts::new(
            self.error_domain(),
            crate::RuntimeErrorCode::Unsupported,
            self.operation(),
            Some(message),
        )
    }

    pub fn error_domain(&self) -> RuntimeErrorDomain {
        match self {
            Self::MidiListOutputsRequest
            | Self::MidiListInputsRequest
            | Self::MidiSelectOutput { .. }
            | Self::MidiSelectInput { .. }
            | Self::MidiPanic => RuntimeErrorDomain::Midi,
            Self::SampleListRequest { .. } => RuntimeErrorDomain::Sample,
            Self::AudioCommand { .. } | Self::RecordingStartAudio { .. } | Self::RecordingStop => {
                RuntimeErrorDomain::Audio
            }
            Self::StoreListPresets
            | Self::StoreLoadPreset { .. }
            | Self::StoreSavePreset { .. }
            | Self::StoreDeletePreset { .. }
            | Self::StoreLoadDefault
            | Self::StoreSaveDefault { .. }
            | Self::StoreSaveBackup { .. }
            | Self::StoreSaveRecovery { .. } => RuntimeErrorDomain::Storage,
            Self::SystemInfoRequest => RuntimeErrorDomain::Runtime,
            _ => RuntimeErrorDomain::Runtime,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimePlatformRequest {
    pub effect: RuntimePlatformEffect,
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(default)]
    pub revision: Option<u64>,
}

impl RuntimePlatformRequest {
    pub fn new(effect: RuntimePlatformEffect, request_id: String, revision: Option<u64>) -> Self {
        Self {
            effect,
            request_id,
            revision,
        }
    }

    pub fn operation(&self) -> RuntimeOperation {
        self.effect.operation()
    }

    pub fn error_domain(&self) -> RuntimeErrorDomain {
        self.effect.error_domain()
    }

    pub fn failure_facts(&self, message: String) -> RuntimeErrorFacts {
        RuntimeErrorFacts::new(
            self.error_domain(),
            crate::RuntimeErrorCode::OperationFailed,
            self.operation(),
            Some(message),
        )
        .with_identity(Some(self.request_id.clone()), self.revision)
    }

    pub fn unsupported_facts(&self, message: String) -> RuntimeErrorFacts {
        self.effect
            .unsupported_facts(message)
            .with_identity(Some(self.request_id.clone()), self.revision)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSystemInfo {
    pub os: String,
    #[serde(rename = "osVersion")]
    pub os_version: String,
    #[serde(rename = "octesseraVersion")]
    pub octessera_version: String,
    #[serde(rename = "primaryIp")]
    pub primary_ip: Option<String>,
    #[serde(rename = "primaryMac")]
    pub primary_mac: Option<String>,
    pub hostname: String,
    #[serde(rename = "boardProfile")]
    pub board_profile: String,
}

impl RuntimeSystemInfo {
    pub fn sanitized(self) -> Self {
        Self {
            os: sanitize_system_info_text(&self.os),
            os_version: sanitize_system_info_text(&self.os_version),
            octessera_version: sanitize_system_info_text(&self.octessera_version),
            primary_ip: self.primary_ip.as_deref().map(sanitize_system_info_text),
            primary_mac: self.primary_mac.as_deref().map(sanitize_system_info_text),
            hostname: sanitize_system_info_text(&self.hostname),
            board_profile: sanitize_system_info_text(&self.board_profile),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSystemInfoError {
    pub code: RuntimeErrorCode,
    pub message: String,
}

impl RuntimeSystemInfoError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RuntimeErrorCode::Unavailable,
            message: sanitize_system_info_text(&message.into()),
        }
    }
}

fn sanitize_system_info_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(96)
        .collect()
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
    RuntimeFailure {
        error: RuntimeErrorFacts,
    },
    Identified {
        result: Box<RuntimeStoreResult>,
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(default)]
        revision: Option<u64>,
    },
    OperationSucceeded {
        operation: RuntimeOperation,
        #[serde(default, rename = "requestId")]
        request_id: Option<String>,
        #[serde(default)]
        revision: Option<u64>,
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
    DeviceUpdateStatus {
        #[serde(default)]
        ok: bool,
        #[serde(default)]
        message: String,
    },
    SystemInfoResult {
        info: RuntimeSystemInfo,
    },
    SystemInfoError {
        error: RuntimeSystemInfoError,
    },
    UsbSdTransferStatus {
        active: bool,
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

impl RuntimeStoreResult {
    pub fn with_identity(self, request_id: String, revision: Option<u64>) -> Self {
        Self::Identified {
            result: Box::new(self),
            request_id,
            revision,
        }
    }

    pub fn operation(&self) -> RuntimeOperation {
        match self {
            Self::ListPresetsResult { .. } => RuntimeOperation::StoreListPresets,
            Self::LoadPresetResult { .. } => RuntimeOperation::StoreLoadPreset,
            Self::SavePresetResult { .. } => RuntimeOperation::StoreSavePreset,
            Self::DeletePresetResult { .. } => RuntimeOperation::StoreDeletePreset,
            Self::LoadDefaultResult { .. } => RuntimeOperation::StoreLoadDefault,
            Self::SaveDefaultResult { .. } => RuntimeOperation::StoreSaveDefault,
            Self::SaveBackupResult { .. } => RuntimeOperation::StoreSaveBackup,
            Self::SaveRecoveryResult { .. } => RuntimeOperation::StoreSaveRecovery,
            Self::StoreError { .. } => RuntimeOperation::Store,
            Self::RuntimeFailure { error } => error.operation.clone(),
            Self::Identified { result, .. } => result.operation(),
            Self::OperationSucceeded { operation, .. } => operation.clone(),
            Self::MidiListOutputsResult { .. } => RuntimeOperation::MidiListOutputs,
            Self::MidiListInputsResult { .. } => RuntimeOperation::MidiListInputs,
            Self::MidiStatus { .. } => RuntimeOperation::MidiStatus,
            Self::SampleListResult { .. } | Self::SampleListError { .. } => {
                RuntimeOperation::SampleList
            }
            Self::SamplePreviewError { .. } => RuntimeOperation::SamplePreview,
            Self::DeviceUpdateStatus { .. } => RuntimeOperation::DeviceUpdate,
            Self::SystemInfoResult { .. } | Self::SystemInfoError { .. } => {
                RuntimeOperation::SystemInfo
            }
            Self::UsbSdTransferStatus { .. } => RuntimeOperation::RuntimeDispatch,
        }
    }

    pub fn error_facts(&self) -> Option<RuntimeErrorFacts> {
        let (domain, message) = match self {
            Self::RuntimeFailure { error } => return Some(error.clone()),
            Self::Identified {
                result,
                request_id,
                revision,
            } => {
                return result
                    .error_facts()
                    .map(|facts| facts.with_identity(Some(request_id.clone()), *revision))
            }
            Self::StoreError { message } => (RuntimeErrorDomain::Storage, message.clone()),
            Self::DeletePresetResult { ok: false, .. }
            | Self::SaveDefaultResult { ok: false, .. }
            | Self::SaveBackupResult { ok: false }
            | Self::SaveRecoveryResult { ok: false } => {
                (RuntimeErrorDomain::Storage, "operation failed".into())
            }
            Self::MidiStatus {
                ok: false, message, ..
            } => (
                RuntimeErrorDomain::Midi,
                message
                    .clone()
                    .unwrap_or_else(|| "MIDI operation failed".into()),
            ),
            Self::SampleListError { message, .. } => (RuntimeErrorDomain::Sample, message.clone()),
            Self::SamplePreviewError { message } => (RuntimeErrorDomain::Sample, message.clone()),
            Self::DeviceUpdateStatus { ok: false, message } => {
                (RuntimeErrorDomain::Runtime, message.clone())
            }
            Self::SystemInfoError { error } => {
                return Some(RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Runtime,
                    error.code.clone(),
                    RuntimeOperation::SystemInfo,
                    Some(error.message.clone()),
                ));
            }
            _ => return None,
        };
        Some(RuntimeErrorFacts::new(
            domain,
            crate::RuntimeErrorCode::OperationFailed,
            self.operation(),
            Some(message),
        ))
    }

    pub fn success_identity(&self) -> Option<(RuntimeOperation, Option<String>, Option<u64>)> {
        match self {
            Self::Identified {
                result,
                request_id,
                revision,
            } if result.error_facts().is_none() => {
                Some((result.operation(), Some(request_id.clone()), *revision))
            }
            Self::OperationSucceeded {
                operation,
                request_id,
                revision,
            } => Some((operation.clone(), request_id.clone(), *revision)),
            _ => None,
        }
    }
}
