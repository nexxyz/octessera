use super::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeIngest};
use crate::protocol::{HostMessage, RuntimeOperation, RuntimePlatformEffect, RuntimeStoreResult};

impl PlaybackRuntime {
    /// Hands one runner effect to the host as an identified request and routes what comes back:
    /// follow-up results, or an adapter failure latched with the request's identity.
    pub(super) fn forward_platform_effect<H: HostAdapter>(
        &mut self,
        effect: RuntimePlatformEffect,
        runner: &mut Option<&mut dyn CoreRunner>,
        host: &mut H,
        output: &mut RuntimeIngest,
    ) {
        if matches!(
            effect,
            RuntimePlatformEffect::StoreSaveDefault { mode: None, .. }
        ) {
            // A patch save run by hand latches its own failure again.
            self.forget_shown_save_failure(&RuntimeOperation::StoreSaveDefault);
        }
        let request = self.next_platform_request(effect);
        if let Some(runner) = runner.as_deref_mut() {
            runner.register_platform_request(&request);
        }
        match host.handle_platform_effect(&request) {
            Ok(messages) => {
                for follow_up in messages {
                    let follow_up = self.identify_result(follow_up, &request);
                    output.follow_ups.push(follow_up);
                }
            }
            Err(error) => {
                let recording_finalization_failed = error.facts.operation
                    == RuntimeOperation::Recording
                    && !matches!(
                        &request.effect,
                        RuntimePlatformEffect::RecordingStartAudio { .. }
                            | RuntimePlatformEffect::RecordingStartAudioOled { .. }
                    );
                let recording_failure = recording_finalization_failed
                    .then(|| error.facts.clone())
                    .map(|facts| {
                        facts.with_identity(Some(request.request_id.clone()), request.revision)
                    });
                let message = error.to_string();
                let mut error = error;
                if error.facts.preset_name.is_none() {
                    error.facts.preset_name = request.effect.preset_name();
                }
                self.latch_adapter_failure(
                    error,
                    request.error_domain(),
                    request.operation(),
                    Some(request.request_id.clone()),
                    request.revision,
                );
                if matches!(
                    request.operation(),
                    RuntimeOperation::StoreSaveDefault
                        | RuntimeOperation::StoreSaveSystem
                        | RuntimeOperation::Recording
                ) || recording_finalization_failed
                {
                    let failure_facts =
                        recording_failure.unwrap_or_else(|| request.effect.failure_facts(message));
                    output.follow_ups.push(HostMessage::RuntimeResult {
                        result: RuntimeStoreResult::RuntimeFailure {
                            error: failure_facts,
                        }
                        .with_identity(request.request_id.clone(), request.revision),
                    });
                }
                self.append_presentations(output);
            }
        }
    }
}
