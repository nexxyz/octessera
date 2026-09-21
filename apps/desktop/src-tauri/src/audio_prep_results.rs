use super::audio_prep_config::AudioPrepError;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimeStoreResult,
};
use rodio_engine_source::QueueSendError;
use std::sync::mpsc::Sender;

pub(super) fn send_audio_prep_result(result_tx: &Sender<HostMessage>, result: RuntimeStoreResult) {
    let _ = result_tx.send(HostMessage::RuntimeResult { result });
}

pub(super) fn full_prep_success(
    revision: u64,
    request_id: Option<String>,
    _generation: u64,
) -> RuntimeStoreResult {
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

pub(super) fn full_prep_failure(
    revision: u64,
    request_id: Option<String>,
    error: AudioPrepError,
) -> RuntimeStoreResult {
    let result = match error {
        AudioPrepError::Sample(error) => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Sample,
                error.code(),
                RuntimeOperation::AudioCommand,
                Some(error.message()),
            ),
        },
        AudioPrepError::InvalidConfig(message) => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::InvalidPayload,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        AudioPrepError::Failed(message) => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::AudioCommand,
                Some(message),
            ),
        },
        AudioPrepError::Superseded => {
            return RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(revision),
            }
        }
    };
    identify_audio_prep_result(result, request_id, revision)
}

pub(super) fn full_prep_failure_from_queue(
    revision: u64,
    request_id: Option<String>,
    error: QueueSendError,
) -> RuntimeStoreResult {
    identify_audio_prep_result(
        queue_failure(RuntimeOperation::AudioCommand, error),
        request_id,
        revision,
    )
}

pub(super) fn owner_prep_failure(error: AudioPrepError) -> RuntimeStoreResult {
    full_prep_failure(0, None, error)
}

pub(super) fn owner_prep_failure_from_queue(error: QueueSendError) -> RuntimeStoreResult {
    queue_failure(RuntimeOperation::AudioCommand, error)
}

pub(super) fn sample_preview_failure(error: AudioPrepError) -> RuntimeStoreResult {
    match error {
        AudioPrepError::Sample(error) => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Sample,
                error.code(),
                RuntimeOperation::SamplePreview,
                Some(error.message()),
            ),
        },
        AudioPrepError::Failed(message) | AudioPrepError::InvalidConfig(message) => {
            RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Audio,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::SamplePreview,
                    Some(message),
                ),
            }
        }
        AudioPrepError::Superseded => RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Sample,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::SamplePreview,
                Some("sample preview superseded".into()),
            ),
        },
    }
}

pub(super) fn sample_preview_queue_failure(error: QueueSendError) -> RuntimeStoreResult {
    queue_failure(RuntimeOperation::SamplePreview, error)
}

fn queue_failure(operation: RuntimeOperation, error: QueueSendError) -> RuntimeStoreResult {
    let code = match error {
        QueueSendError::Full { .. } => RuntimeErrorCode::OperationFailed,
        QueueSendError::Disconnected { .. } => RuntimeErrorCode::AudioThreadFailed,
    };
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Audio,
            code,
            operation,
            Some(error.to_string()),
        ),
    }
}

fn identify_audio_prep_result(
    result: RuntimeStoreResult,
    request_id: Option<String>,
    revision: u64,
) -> RuntimeStoreResult {
    match request_id {
        Some(request_id) => result.with_identity(request_id, Some(revision)),
        None => result,
    }
}
