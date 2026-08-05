use super::{RuntimeAudioCommand, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation};
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
    SetupPortalOpen,
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
            Self::SetupPortalOpen => RuntimeOperation::SetupPortal,
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
            Self::SetupPortalOpen => RuntimeErrorDomain::Runtime,
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
