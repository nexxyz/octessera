use playback_runtime::{HostMessage, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult};

#[derive(Clone, Copy)]
pub(crate) enum InitialAudioPrepBoard {
    #[cfg(any(not(feature = "hardware-orange-pi-zero-2w"), test))]
    Pi,
    #[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
    Orange,
}

impl InitialAudioPrepBoard {
    fn failure_context(self) -> &'static str {
        match self {
            #[cfg(any(not(feature = "hardware-orange-pi-zero-2w"), test))]
            Self::Pi => "initial Pi audio preparation failed",
            #[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
            Self::Orange => "initial Orange audio preparation failed",
        }
    }
}

pub(crate) fn interpret_initial_audio_prep(
    message: &HostMessage,
    expected_revision: u64,
    board: InitialAudioPrepBoard,
) -> Option<Result<(), String>> {
    let HostMessage::RuntimeResult { result } = message else {
        return None;
    };
    match result {
        RuntimeStoreResult::Identified {
            result,
            request_id,
            revision,
        } if result.operation() == RuntimeOperation::AudioCommand => match result.error_facts() {
            Some(facts) if revision.is_none() || *revision == Some(expected_revision) => {
                Some(Err(format_initial_audio_prep_failure(
                    board,
                    format!("request {request_id}, revision {revision:?}"),
                    &facts,
                )))
            }
            Some(_) => None,
            None if *revision == Some(expected_revision) => Some(Ok(())),
            None => None,
        },
        RuntimeStoreResult::OperationSucceeded { revision, .. }
            if result.operation() == RuntimeOperation::AudioCommand
                && *revision == Some(expected_revision) =>
        {
            Some(Ok(()))
        }
        RuntimeStoreResult::RuntimeFailure { error }
            if error.operation == RuntimeOperation::AudioCommand =>
        {
            Some(Err(format_initial_audio_prep_failure(
                board,
                format!(
                    "request {:?}, revision {:?}",
                    error.request_id, error.revision
                ),
                error,
            )))
        }
        _ => None,
    }
}

fn format_initial_audio_prep_failure(
    board: InitialAudioPrepBoard,
    context: String,
    facts: &RuntimeErrorFacts,
) -> String {
    format!(
        "{} ({context}, {:?}/{:?}): {}",
        board.failure_context(),
        facts.domain,
        facts.code,
        facts.message.as_deref().unwrap_or("operation failed")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_result(result: RuntimeStoreResult) -> HostMessage {
        HostMessage::RuntimeResult { result }
    }

    #[test]
    fn accepts_raw_audio_success_at_the_expected_revision() {
        let message = runtime_result(RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: None,
            revision: Some(1),
        });

        assert!(matches!(
            interpret_initial_audio_prep(&message, 1, InitialAudioPrepBoard::Pi),
            Some(Ok(()))
        ));
    }

    #[test]
    fn accepts_identified_audio_success_at_the_expected_revision() {
        let message = runtime_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(1),
            }),
            request_id: "audio-initial".into(),
            revision: Some(1),
        });

        assert!(matches!(
            interpret_initial_audio_prep(&message, 1, InitialAudioPrepBoard::Orange),
            Some(Ok(()))
        ));
    }

    #[test]
    fn returns_typed_raw_and_identified_audio_failures_with_board_context() {
        let raw = runtime_result(RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                playback_runtime::RuntimeErrorDomain::Audio,
                playback_runtime::RuntimeErrorCode::OperationFailed,
                RuntimeOperation::AudioCommand,
                Some("DAC configuration failed".into()),
            ),
        });
        let identified = runtime_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    playback_runtime::RuntimeErrorDomain::Sample,
                    playback_runtime::RuntimeErrorCode::NotFound,
                    RuntimeOperation::AudioCommand,
                    Some("sample not found: samples/kick.wav".into()),
                ),
            }),
            request_id: "audio-initial".into(),
            revision: Some(2),
        });

        let Some(Err(raw_error)) = interpret_initial_audio_prep(&raw, 1, InitialAudioPrepBoard::Pi)
        else {
            panic!("expected raw audio preparation failure");
        };
        let Some(Err(identified_error)) =
            interpret_initial_audio_prep(&identified, 2, InitialAudioPrepBoard::Orange)
        else {
            panic!("expected identified audio preparation failure");
        };
        assert!(raw_error.contains("initial Pi audio preparation failed"));
        assert!(raw_error.contains("Audio/OperationFailed"));
        assert!(raw_error.contains("DAC configuration failed"));
        assert!(identified_error.contains("initial Orange audio preparation failed"));
        assert!(identified_error.contains("audio-initial"));
        assert!(identified_error.contains("sample not found: samples/kick.wav"));
    }

    #[test]
    fn ignores_unrelated_stale_and_unrevisioned_successes() {
        let unrelated = runtime_result(RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::Store,
            request_id: None,
            revision: Some(1),
        });
        let stale = runtime_result(RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: None,
            revision: Some(1),
        });
        let unrevisioned = runtime_result(RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            request_id: None,
            revision: None,
        });
        let identified_unrevisioned = runtime_result(RuntimeStoreResult::Identified {
            result: Box::new(RuntimeStoreResult::OperationSucceeded {
                operation: RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(1),
            }),
            request_id: "audio-initial".into(),
            revision: None,
        });

        for message in [&unrelated, &stale, &unrevisioned, &identified_unrevisioned] {
            assert!(interpret_initial_audio_prep(message, 2, InitialAudioPrepBoard::Pi).is_none());
        }
    }
}
