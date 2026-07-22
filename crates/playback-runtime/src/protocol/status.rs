use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorDomain {
    Runtime,
    Storage,
    Midi,
    Sample,
    Audio,
    Serialization,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorCode {
    OperationFailed,
    Unavailable,
    InvalidPayload,
    NotFound,
    Unsupported,
    SerializationFailed,
    AudioThreadFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperation {
    RuntimeDispatch,
    DeviceInput,
    Transport,
    MusicalEvent,
    MidiEvent,
    MidiMessage,
    AudioCommand,
    AudioThread,
    Snapshot,
    TransportStop,
    Store,
    StoreListPresets,
    StoreLoadPreset,
    StoreSavePreset,
    StoreDeletePreset,
    StoreLoadDefault,
    StoreSaveDefault,
    StoreSaveBackup,
    StoreSaveRecovery,
    RuntimeEmission,
    Persistence,
    MidiListOutputs,
    MidiListInputs,
    MidiStatus,
    SampleList,
    SamplePreview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRecovery {
    Retry,
    RetainLastGood,
    StopAndSilence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeErrorMetadata {
    pub domain: RuntimeErrorDomain,
    pub code: RuntimeErrorCode,
    pub operation: RuntimeOperation,
    pub recovery: RuntimeRecovery,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RuntimeErrorMetadata {
    pub fn new(
        domain: RuntimeErrorDomain,
        code: RuntimeErrorCode,
        operation: RuntimeOperation,
        recovery: RuntimeRecovery,
        message: Option<String>,
    ) -> Self {
        Self {
            domain,
            code,
            operation,
            recovery,
            request_id: None,
            revision: None,
            message,
        }
    }

    pub fn operation_failed(
        domain: RuntimeErrorDomain,
        operation: RuntimeOperation,
        recovery: RuntimeRecovery,
        message: String,
    ) -> Self {
        Self::new(
            domain,
            RuntimeErrorCode::OperationFailed,
            operation,
            recovery,
            Some(message),
        )
    }

    pub fn with_identity(mut self, request_id: Option<String>, revision: Option<u64>) -> Self {
        self.request_id = request_id;
        self.revision = revision;
        self
    }

    pub fn with_context(
        mut self,
        domain: RuntimeErrorDomain,
        operation: RuntimeOperation,
        recovery: RuntimeRecovery,
        request_id: Option<String>,
        revision: Option<u64>,
    ) -> Self {
        self.domain = domain;
        self.operation = operation;
        self.recovery = recovery;
        if request_id.is_some() {
            self.request_id = request_id;
        }
        if revision.is_some() {
            self.revision = revision;
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeErrorFacts {
    pub domain: RuntimeErrorDomain,
    pub code: RuntimeErrorCode,
    pub operation: RuntimeOperation,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RuntimeErrorFacts {
    pub fn new(
        domain: RuntimeErrorDomain,
        code: RuntimeErrorCode,
        operation: RuntimeOperation,
        message: Option<String>,
    ) -> Self {
        Self {
            domain,
            code,
            operation,
            request_id: None,
            revision: None,
            message,
        }
    }

    pub fn with_identity(mut self, request_id: Option<String>, revision: Option<u64>) -> Self {
        self.request_id = request_id;
        self.revision = revision;
        self
    }

    pub fn with_context(
        mut self,
        domain: RuntimeErrorDomain,
        operation: RuntimeOperation,
        request_id: Option<String>,
        revision: Option<u64>,
    ) -> Self {
        self.domain = domain;
        self.operation = operation;
        if request_id.is_some() {
            self.request_id = request_id;
        }
        if revision.is_some() {
            self.revision = revision;
        }
        self
    }

    pub fn into_metadata(self, recovery: RuntimeRecovery) -> RuntimeErrorMetadata {
        RuntimeErrorMetadata {
            domain: self.domain,
            code: self.code,
            operation: self.operation,
            recovery,
            request_id: self.request_id,
            revision: self.revision,
            message: self.message,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAdapterError {
    pub facts: RuntimeErrorFacts,
}

impl RuntimeAdapterError {
    pub fn operation_failed(message: String) -> Self {
        Self {
            facts: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::RuntimeDispatch,
                Some(message),
            ),
        }
    }

    pub fn from_metadata(metadata: RuntimeErrorMetadata) -> Self {
        Self {
            facts: RuntimeErrorFacts {
                domain: metadata.domain,
                code: metadata.code,
                operation: metadata.operation,
                request_id: metadata.request_id,
                revision: metadata.revision,
                message: metadata.message,
            },
        }
    }

    pub fn from_facts(facts: RuntimeErrorFacts) -> Self {
        Self { facts }
    }

    pub fn into_metadata(
        self,
        domain: RuntimeErrorDomain,
        operation: RuntimeOperation,
        recovery: RuntimeRecovery,
        request_id: Option<String>,
        revision: Option<u64>,
    ) -> RuntimeErrorMetadata {
        self.facts
            .with_context(domain, operation, request_id, revision)
            .into_metadata(recovery)
    }
}

impl Display for RuntimeAdapterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            self.facts
                .message
                .as_deref()
                .unwrap_or("runtime adapter failure"),
        )
    }
}

impl std::error::Error for RuntimeAdapterError {}

impl From<String> for RuntimeAdapterError {
    fn from(message: String) -> Self {
        Self::operation_failed(message)
    }
}

impl From<&str> for RuntimeAdapterError {
    fn from(message: &str) -> Self {
        Self::operation_failed(message.into())
    }
}

impl From<RuntimeAdapterError> for String {
    fn from(error: RuntimeAdapterError) -> Self {
        error.to_string()
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RuntimeErrorMetadata>,
}
