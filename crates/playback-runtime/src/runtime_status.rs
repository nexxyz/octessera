use super::{CoreRunner, HostAdapter, PlaybackRuntime, RuntimeIngest};
use crate::protocol::{
    HostMessage, RunnerMessage, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery, RuntimeStatusState, RuntimeStoreResult,
};
use serde_json::Value;

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
        if let HostMessage::RuntimeResult { result } = message {
            self.apply_runtime_result(result, operation);
        }
    }

    pub(super) fn apply_runtime_result(
        &mut self,
        result: &RuntimeStoreResult,
        operation: Option<RuntimeOperation>,
    ) {
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
                    | RuntimeOperation::StoreSaveBackup
                    | RuntimeOperation::StoreSaveRecovery
            ) {
                self.clear_error(RuntimeOperation::Store);
            }
            if matches!(
                operation.clone(),
                RuntimeOperation::StoreSavePreset
                    | RuntimeOperation::StoreSaveDefault
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

    pub(super) fn refresh_presentations(&mut self) {
        self.refresh_presented_status();
        self.refresh_presented_snapshot();
    }

    pub(super) fn append_snapshot(&self, output: &mut RuntimeIngest) {
        if let Some(snapshot) = self.presented_snapshot.clone() {
            output.messages.push(RunnerMessage::Snapshot { snapshot });
        }
    }

    pub(super) fn append_status(&self, output: &mut RuntimeIngest) {
        if let Some(status) = self.presented_status.clone() {
            output
                .messages
                .push(RunnerMessage::RuntimeStatus { status });
        }
    }

    pub(super) fn append_presentations(&self, output: &mut RuntimeIngest) {
        self.append_snapshot(output);
        self.append_status(output);
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
                self.best_effort_host_silence(host, output);
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
        match runner.send(HostMessage::TransportStop) {
            Ok(messages) => {
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
        self.best_effort_host_silence(host, &mut output);
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
    ) {
        if let Err(error) = host.silence_internal_audio() {
            self.latch_error(error.into_metadata(
                RuntimeErrorDomain::Audio,
                RuntimeOperation::AudioCommand,
                RuntimeRecovery::StopAndSilence,
                None,
                None,
            ));
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

    fn refresh_presented_status(&mut self) {
        let Some(raw_status) = self.last_good_status.as_ref() else {
            self.presented_status = None;
            return;
        };
        let mut status = raw_status.clone();
        status.error = self.latched_errors.last().cloned();
        if status.error.is_some() {
            status.state = RuntimeStatusState::Error;
        }
        self.presented_status = Some(status);
    }

    pub(super) fn refresh_presented_snapshot(&mut self) {
        let Some(raw_snapshot) = self.last_good_snapshot.as_ref() else {
            self.presented_snapshot = None;
            return;
        };
        let presented = if let Some(error) = self.latched_errors.last() {
            let Some(mut snapshot) = raw_snapshot.as_object().cloned() else {
                self.presented_snapshot = None;
                return;
            };
            let Ok(error) = serde_json::to_value(error) else {
                self.presented_snapshot = None;
                return;
            };
            snapshot.insert("runtimeError".into(), error);
            Value::Object(snapshot)
        } else {
            raw_snapshot.clone()
        };
        if self.presented_snapshot.as_ref() != Some(&presented) {
            self.last_snapshot_revision = self.last_snapshot_revision.wrapping_add(1);
        }
        self.presented_snapshot = Some(presented);
    }

    pub(super) fn snapshot_failure() -> RuntimeErrorMetadata {
        RuntimeErrorMetadata::new(
            RuntimeErrorDomain::Serialization,
            crate::RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::Snapshot,
            RuntimeRecovery::StopAndSilence,
            Some("runtime snapshot must be a JSON object".into()),
        )
    }
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
