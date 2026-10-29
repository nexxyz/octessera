use super::PlaybackRuntime;
use crate::protocol::{
    HostMessage, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::Value;

impl PlaybackRuntime {
    pub(super) fn next_platform_request(
        &mut self,
        effect: RuntimePlatformEffect,
    ) -> RuntimePlatformRequest {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let revision = match &effect {
            RuntimePlatformEffect::AudioCommand {
                command: RuntimeAudioCommand::SetAudioConfig { revision, .. },
            } => Some(*revision),
            RuntimePlatformEffect::StoreSavePreset { payload, .. }
            | RuntimePlatformEffect::StoreSaveDefault { payload, .. }
            | RuntimePlatformEffect::StoreSaveBackup { payload }
            | RuntimePlatformEffect::StoreSaveRecovery { payload }
            | RuntimePlatformEffect::ApplyDeviceConfig { payload } => {
                payload.get("revision").and_then(Value::as_u64)
            }
            RuntimePlatformEffect::StoreSaveSystem { .. } => None,
            _ => None,
        };
        RuntimePlatformRequest::new(
            effect,
            format!("platform-{}", self.next_request_id),
            revision,
        )
    }

    pub(super) fn identify_audio_command(
        &mut self,
        command: RuntimeAudioCommand,
    ) -> RuntimeAudioCommand {
        let RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id: None,
            generation,
            config,
        } = command
        else {
            return command;
        };
        self.next_request_id = self.next_request_id.saturating_add(1);
        RuntimeAudioCommand::SetAudioConfig {
            revision,
            request_id: Some(format!("audio-{}", self.next_request_id)),
            generation,
            config,
        }
    }

    pub(super) fn identify_result(
        &self,
        message: HostMessage,
        request: &RuntimePlatformRequest,
    ) -> HostMessage {
        match message {
            HostMessage::RuntimeResult { result } => {
                let result = match result {
                    RuntimeStoreResult::StoreError { message } => {
                        RuntimeStoreResult::RuntimeFailure {
                            error: request.effect.failure_facts(message),
                        }
                    }
                    RuntimeStoreResult::Identified {
                        result,
                        request_id,
                        revision,
                    } if request_id == request.request_id
                        && revision == request.revision
                        && !matches!(result.as_ref(), RuntimeStoreResult::Identified { .. }) =>
                    {
                        return HostMessage::RuntimeResult {
                            result: RuntimeStoreResult::Identified {
                                result,
                                request_id,
                                revision,
                            },
                        };
                    }
                    RuntimeStoreResult::Identified { .. } => invalid_identified_result(request),
                    result => result,
                };
                HostMessage::RuntimeResult {
                    result: result.with_identity(request.request_id.clone(), request.revision),
                }
            }
            other => other,
        }
    }
}

fn invalid_identified_result(request: &RuntimePlatformRequest) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Serialization,
            RuntimeErrorCode::InvalidPayload,
            request.operation(),
            Some("runtime result identity is invalid".into()),
        ),
    }
}

#[cfg(test)]
mod tests;
