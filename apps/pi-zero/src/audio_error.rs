use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};

pub(crate) fn audio_queue_error(message: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}
