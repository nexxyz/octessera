use super::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeIngest};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery, RuntimeStoreResult,
};

impl PlaybackRuntime {
    pub fn latch_facts(&mut self, facts: RuntimeErrorFacts) {
        let recovery = recovery_for_facts(&facts);
        self.latch_error(facts.into_metadata(recovery));
    }

    pub fn recover_from_facts<R: CoreRunner, H: HostAdapter>(
        &mut self,
        facts: RuntimeErrorFacts,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let recovery = recovery_for_facts(&facts);
        self.recover_from_error(facts.into_metadata(recovery), runner, host)
    }

    pub fn recover_from_error<R: CoreRunner, H: HostAdapter>(
        &mut self,
        error: RuntimeErrorMetadata,
        runner: &mut R,
        host: &mut H,
    ) -> Result<RuntimeIngest, String> {
        let recovery = error.recovery.clone();
        self.latch_error(error);
        if recovery == RuntimeRecovery::StopAndSilence {
            return Ok(self.best_effort_stop_and_silence(runner, host));
        }
        let mut output = RuntimeIngest::default();
        self.append_presentations(&mut output);
        Ok(output)
    }

    pub(super) fn observe_host_message(
        &mut self,
        message: &HostMessage,
        operation: Option<RuntimeOperation>,
    ) {
        if matches!(
            message,
            HostMessage::MidiRealtimeStart
                | HostMessage::MidiRealtimeStop
                | HostMessage::TransportStop
        ) {
            self.reset_pulse_phase();
        }
        if let HostMessage::RuntimeResult { result } = message {
            self.apply_runtime_result(result, operation);
        }
    }

    pub(super) fn apply_runtime_result(
        &mut self,
        result: &RuntimeStoreResult,
        operation: Option<RuntimeOperation>,
    ) {
        if let Some(facts) = setup_portal_lifecycle_error_facts(result) {
            self.clear_error_with_identity(
                RuntimeOperation::SetupPortal,
                facts.request_id.as_deref(),
                facts.revision,
            );
            return;
        }
        if let Some(facts) = user_data_transfer_lifecycle_error_facts(result) {
            self.clear_error_with_identity(
                RuntimeOperation::UserDataTransfer,
                facts.request_id.as_deref(),
                facts.revision,
            );
            return;
        }
        if let Some(mut facts) = result.error_facts() {
            if !self.remember_result(&facts, false) {
                return;
            }
            if facts.operation == RuntimeOperation::Store {
                if let Some(operation) = operation {
                    facts.operation = operation;
                }
            }
            self.latch_error(facts.into_metadata(RuntimeRecovery::RetainLastGood));
        } else {
            let operation = result.operation();
            if let Some((_, request_id, revision)) = result.success_identity() {
                let key = crate::RuntimeErrorFacts::new(
                    crate::RuntimeErrorDomain::Runtime,
                    crate::RuntimeErrorCode::OperationFailed,
                    operation.clone(),
                    None,
                )
                .with_identity(request_id.clone(), revision);
                if !self.remember_result(&key, true) {
                    return;
                }
            }
            if let Some((operation, request_id, revision)) = result.success_identity() {
                self.clear_error_with_identity(operation, request_id.as_deref(), revision);
            } else {
                self.clear_error(operation.clone());
            }
            if matches!(
                operation.clone(),
                RuntimeOperation::StoreListPresets
                    | RuntimeOperation::StoreLoadPreset
                    | RuntimeOperation::StoreSavePreset
                    | RuntimeOperation::StoreDeletePreset
                    | RuntimeOperation::StoreLoadDefault
                    | RuntimeOperation::StoreSaveDefault
                    | RuntimeOperation::StoreLoadSystem
                    | RuntimeOperation::StoreSaveSystem
                    | RuntimeOperation::StoreSaveBackup
                    | RuntimeOperation::StoreSaveRecovery
            ) {
                self.clear_error(RuntimeOperation::Store);
            }
            if matches!(
                operation.clone(),
                RuntimeOperation::StoreSavePreset
                    | RuntimeOperation::StoreSaveDefault
                    | RuntimeOperation::StoreSaveSystem
                    | RuntimeOperation::StoreSaveBackup
                    | RuntimeOperation::StoreSaveRecovery
            ) {
                self.clear_error(RuntimeOperation::Persistence);
            }
        }
    }

    fn remember_result(&mut self, error: &crate::RuntimeErrorFacts, failed: bool) -> bool {
        if error.request_id.is_none() && error.revision.is_none() {
            return true;
        }
        let key = (
            failed,
            error.operation.clone(),
            error.request_id.clone(),
            error.revision,
        );
        if self.processed_result_keys.contains(&key) {
            return false;
        }
        self.processed_result_keys.push(key);
        if self.processed_result_keys.len() > 128 {
            self.processed_result_keys.drain(..64);
        }
        true
    }

    pub(super) fn apply_recovery<H: HostAdapter>(
        &mut self,
        recovery: RuntimeRecovery,
        runner: &mut Option<&mut dyn CoreRunner>,
        host: &mut H,
        output: &mut RuntimeIngest,
    ) {
        if recovery == RuntimeRecovery::StopAndSilence {
            if let Some(runner) = runner.take() {
                output.merge(self.best_effort_stop_and_silence(runner, host));
            } else {
                self.best_effort_host_silence(host, output, true);
                output.follow_ups.push(HostMessage::TransportStop);
            }
        }
    }

    pub fn best_effort_stop_and_silence<H: HostAdapter>(
        &mut self,
        runner: &mut dyn CoreRunner,
        host: &mut H,
    ) -> RuntimeIngest {
        let mut output = RuntimeIngest::default();
        self.reset_pulse_phase();
        let mut internal_silence_attempted = false;
        match runner.send(HostMessage::TransportStop) {
            Ok(messages) => {
                internal_silence_attempted = self.last_good_status.as_ref().is_some_and(|previous| {
                    messages.iter().any(|message| {
                        matches!(
                            message,
                            RunnerMessage::RuntimeStatus { status }
                                if super::midi::transport_requires_note_cleanup(Some(previous), status)
                        )
                    })
                });
                if let Ok(ingest) =
                    self.ingest_core_messages_with_runner(messages, Some(runner), host)
                {
                    output.merge(ingest);
                }
            }
            Err(message) => self.latch_error(RuntimeErrorMetadata::operation_failed(
                RuntimeErrorDomain::Runtime,
                RuntimeOperation::TransportStop,
                RuntimeRecovery::StopAndSilence,
                message,
            )),
        }
        self.best_effort_host_silence(host, &mut output, !internal_silence_attempted);
        if let Some(status) = self.last_good_status.as_mut() {
            status.transport = crate::RuntimeTransportState::Stopped;
        }
        self.refresh_presentations();
        self.append_presentations(&mut output);
        output
    }

    pub(super) fn best_effort_host_silence<H: HostAdapter>(
        &mut self,
        host: &mut H,
        output: &mut RuntimeIngest,
        silence_internal_audio: bool,
    ) {
        if silence_internal_audio {
            if let Err(error) = host.silence_internal_audio() {
                self.latch_error(error.into_metadata(
                    RuntimeErrorDomain::Audio,
                    RuntimeOperation::AudioCommand,
                    RuntimeRecovery::StopAndSilence,
                    None,
                    None,
                ));
            }
        }
        if let Err(error) = host.panic_external_midi() {
            self.latch_error(error.into_metadata(
                RuntimeErrorDomain::Midi,
                RuntimeOperation::MidiMessage,
                RuntimeRecovery::StopAndSilence,
                None,
                None,
            ));
        }
        self.append_presentations(output);
    }
}

fn setup_portal_lifecycle_error_facts(result: &RuntimeStoreResult) -> Option<RuntimeErrorFacts> {
    let phase = match result {
        RuntimeStoreResult::SetupPortalStatus { status } => &status.phase,
        RuntimeStoreResult::Identified { result, .. } => match result.as_ref() {
            RuntimeStoreResult::SetupPortalStatus { status } => &status.phase,
            _ => return None,
        },
        _ => return None,
    };
    if matches!(
        phase,
        crate::RuntimeSetupPortalPhase::Failed
            | crate::RuntimeSetupPortalPhase::TimedOut
            | crate::RuntimeSetupPortalPhase::Unsupported
    ) {
        result.error_facts()
    } else {
        None
    }
}

fn user_data_transfer_lifecycle_error_facts(
    result: &RuntimeStoreResult,
) -> Option<RuntimeErrorFacts> {
    let status = match result {
        RuntimeStoreResult::UserDataTransferStatus { status } => status,
        RuntimeStoreResult::Identified { result, .. } => match result.as_ref() {
            RuntimeStoreResult::UserDataTransferStatus { status } => status,
            _ => return None,
        },
        _ => return None,
    };
    (status.phase == crate::RuntimeUserDataTransferPhase::Unsupported).then(|| {
        result.error_facts().unwrap_or_else(|| {
            RuntimeErrorFacts::new(
                RuntimeErrorDomain::Runtime,
                crate::RuntimeErrorCode::Unsupported,
                RuntimeOperation::UserDataTransfer,
                None,
            )
        })
    })
}

fn recovery_for_facts(facts: &RuntimeErrorFacts) -> RuntimeRecovery {
    match facts.operation {
        RuntimeOperation::AudioThread
        | RuntimeOperation::RuntimeDispatch
        | RuntimeOperation::Snapshot
        | RuntimeOperation::TransportStop => RuntimeRecovery::StopAndSilence,
        RuntimeOperation::Persistence => RuntimeRecovery::Retry,
        _ => RuntimeRecovery::RetainLastGood,
    }
}
