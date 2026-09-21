use crate::audio_sink_registry::AudioBroadcastError;
use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
use rodio_engine_source::QueueSendError;

pub(crate) fn audio_queue_error(error: AudioBroadcastError) -> RuntimeAdapterError {
    let message = error.to_string();
    let (code, message) = match error {
        AudioBroadcastError::Queue(error) => (
            match error {
                QueueSendError::Full { .. } => RuntimeErrorCode::OperationFailed,
                QueueSendError::Disconnected { .. } => RuntimeErrorCode::AudioThreadFailed,
            },
            message,
        ),
        AudioBroadcastError::Registry(_) => (RuntimeErrorCode::AudioThreadFailed, message),
    };
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        code,
        RuntimeOperation::AudioCommand,
        Some(message),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback_runtime::RuntimeErrorCode;
    use rodio_engine_source::{QueueKind, QueueSendError};

    #[test]
    fn queue_error_preserves_full_and_disconnected_runtime_codes() {
        let full = audio_queue_error(AudioBroadcastError::Queue(QueueSendError::Full {
            queue: QueueKind::Latest,
        }));
        assert_eq!(full.facts.code, RuntimeErrorCode::OperationFailed);

        let disconnected =
            audio_queue_error(AudioBroadcastError::Queue(QueueSendError::Disconnected {
                queue: QueueKind::Latest,
            }));
        assert_eq!(disconnected.facts.code, RuntimeErrorCode::AudioThreadFailed);
    }
}
