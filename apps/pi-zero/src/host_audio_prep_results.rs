use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult,
};
use rodio_engine_source::EngineEvent;
use std::sync::mpsc::Sender;

pub(super) struct PreparedAudioConfig {
    pub(super) event: EngineEvent,
    pub(super) sample_signature: Option<String>,
}

pub(super) enum AudioPrepError {
    Superseded,
    InvalidConfig(String),
    Sample(crate::audio_config_parse::SampleLoadError),
    Failed(String),
}

pub(super) fn prep_error_message(error: AudioPrepError) -> String {
    match error {
        AudioPrepError::Superseded => "audio preparation superseded".into(),
        AudioPrepError::InvalidConfig(error) | AudioPrepError::Failed(error) => error,
        AudioPrepError::Sample(error) => error.message(),
    }
}

pub(super) fn index_u8(index: usize) -> u8 {
    u8::try_from(index).unwrap_or(u8::MAX)
}

pub(super) fn send_audio_prep_result(result_tx: &Sender<HostMessage>, result: RuntimeStoreResult) {
    let _ = result_tx.send(HostMessage::RuntimeResult { result });
}

pub(super) fn audio_prep_success(revision: u64, request_id: Option<String>) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: None,
            revision: Some(revision),
        },
        request_id,
        revision,
    )
}

pub(super) fn audio_prep_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        request_id,
        revision,
    )
}

pub(super) fn audio_config_failure(
    revision: u64,
    request_id: Option<String>,
    message: String,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::InvalidPayload,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        request_id,
        revision,
    )
}

pub(super) fn sample_failure(
    revision: u64,
    request_id: Option<String>,
    code: RuntimeErrorCode,
    message: String,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Sample,
                code,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        request_id,
        revision,
    )
}

pub(super) fn owner_failure(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    }
}

pub(super) fn owner_sample_failure(code: RuntimeErrorCode, message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Sample,
            code,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    }
}

pub(super) fn audio_queue_failure(message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::AudioCommand,
            Some(message),
        ),
    }
}

pub(super) fn identify_audio_prep_result(
    result: RuntimeStoreResult,
    request_id: Option<String>,
    revision: u64,
) -> RuntimeStoreResult {
    match request_id {
        Some(request_id) => result.with_identity(request_id, Some(revision)),
        None => result,
    }
}
