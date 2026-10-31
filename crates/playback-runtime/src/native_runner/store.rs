use crate::protocol::RuntimeStoreResult;

use super::preset_native_completion::NativePresetResultAction;
use super::{native_factory_payload, portable_patch_payload_for_save, NativeRunner, NativeToast};

impl NativeRunner {
    pub(super) fn apply_factory_payload(&mut self) -> Result<(), String> {
        let patch = portable_patch_payload_for_save(&native_factory_payload())?;
        self.apply_patch_payload_preserving_device(patch)?;
        self.stop_for_config_load();
        self.display.toast = Some(NativeToast {
            message: "Factory loaded".into(),
            offset: 0,
        });
        Ok(())
    }

    pub(super) fn apply_store_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
        if is_system_store_operation(&result.operation()) {
            self.apply_system_store_result(result)?;
            return Ok(());
        }
        match result {
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            } => {
                if let RuntimeStoreResult::SetupPortalStatus { status } = result.as_ref() {
                    self.apply_setup_portal_status(status.clone(), Some(request_id), revision);
                    return Ok(());
                }
                if let RuntimeStoreResult::UserDataRestoreStatus { status } = result.as_ref() {
                    self.apply_user_data_restore_status(status.clone(), Some(request_id), revision);
                    return Ok(());
                }
                if let RuntimeStoreResult::UserDataTransferStatus { status } = result.as_ref() {
                    self.apply_user_data_transfer_status(
                        status.clone(),
                        Some(request_id),
                        revision,
                    );
                    return Ok(());
                }
                let operation = result.operation();
                let succeeded = result.error_facts().is_none();
                let is_default_save_success = matches!(
                    result.as_ref(),
                    RuntimeStoreResult::SaveDefaultResult { ok: true, .. }
                );
                let native_preset_action =
                    if operation == crate::protocol::RuntimeOperation::StoreSavePreset {
                        self.resolve_native_preset_result(&request_id, revision, result.as_ref())
                    } else {
                        None
                    };
                let native_preset_ignored = matches!(
                    &native_preset_action,
                    Some(
                        NativePresetResultAction::Ignore | NativePresetResultAction::MissingCatalog
                    )
                );
                let pending_default_revision = self.pending_default_write_revision();
                let restart_completion =
                    if operation == crate::protocol::RuntimeOperation::StoreSaveDefault {
                        self.apply_restart_default_save_result(&result, &request_id, revision)
                    } else {
                        None
                    };
                let is_save_operation = matches!(
                    &operation,
                    crate::protocol::RuntimeOperation::StoreSavePreset
                        | crate::protocol::RuntimeOperation::StoreSaveDefault
                );
                let apply_result = match native_preset_action {
                    Some(NativePresetResultAction::Complete(write)) => {
                        self.finish_native_preset_write(write);
                        Ok(())
                    }
                    Some(NativePresetResultAction::MissingCatalog) => self
                        .apply_error_presentation_result(RuntimeStoreResult::RuntimeFailure {
                            error: crate::RuntimeErrorFacts::new(
                                crate::RuntimeErrorDomain::Storage,
                                crate::RuntimeErrorCode::OperationFailed,
                                crate::RuntimeOperation::StoreSavePreset,
                                Some("native preset catalog unavailable".into()),
                            ),
                        }),
                    Some(NativePresetResultAction::Ignore) => Ok(()),
                    None if operation == crate::protocol::RuntimeOperation::StoreSaveDefault
                        && is_default_save_success =>
                    {
                        self.apply_store_persistence_result_with_default_feedback(
                            *result,
                            restart_completion
                                .as_ref()
                                .is_some_and(|completion| completion.show_saved_feedback),
                        )
                    }
                    None => self.apply_store_result(*result),
                };
                if let Err(error) = apply_result {
                    if operation == crate::protocol::RuntimeOperation::StoreLoadDefault
                        && self.restore_rehydration_pending()
                    {
                        self.fail_restore_rehydration();
                    }
                    return Err(error);
                }
                if !succeeded
                    && is_save_operation
                    && restart_completion.is_none()
                    && operation != crate::protocol::RuntimeOperation::StoreSaveDefault
                    && pending_default_revision != revision
                    && !native_preset_ignored
                {
                    self.retry_config_save_after_restore_failure();
                }
                if succeeded
                    && is_save_operation
                    && restart_completion.is_none()
                    && operation != crate::protocol::RuntimeOperation::StoreSaveDefault
                    && pending_default_revision != revision
                    && !native_preset_ignored
                {
                    self.acknowledge_config_save(revision);
                }
                if let Some(completion) = restart_completion {
                    if completion.succeeded && completion.scope.is_patch() {
                        self.pending.saved_patch_baseline = completion.saved_patch_payload;
                        self.acknowledge_patch_save(completion.captured_patch_dirty_revision);
                    }
                }
            }
            result => {
                let operation = result.operation();
                let failed = result.error_facts().is_some();
                let is_save_operation = matches!(
                    &operation,
                    crate::protocol::RuntimeOperation::StoreSavePreset
                        | crate::protocol::RuntimeOperation::StoreSaveDefault
                );
                if failed && is_save_operation {
                    self.retry_config_save_after_restore_failure();
                }
                if failed
                    && operation == crate::protocol::RuntimeOperation::StoreLoadDefault
                    && self.restore_rehydration_pending()
                {
                    self.fail_restore_rehydration();
                }
                let apply_result = self.apply_unidentified_store_result(result);
                if let Err(error) = apply_result {
                    if operation == crate::protocol::RuntimeOperation::StoreLoadDefault
                        && self.restore_rehydration_pending()
                    {
                        self.fail_restore_rehydration();
                    }
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn apply_unidentified_store_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        match result {
            RuntimeStoreResult::SavePresetResult { .. }
                if self.pending.native_preset_write.is_some() =>
            {
                Ok(())
            }
            result @ RuntimeStoreResult::SaveDefaultResult { .. } => self
                .apply_store_persistence_result_with_default_feedback(
                    result,
                    !self.restart_settings.has_pending_native_write(),
                ),
            result @ (RuntimeStoreResult::ListPresetsResult { .. }
            | RuntimeStoreResult::LoadPresetResult { .. }
            | RuntimeStoreResult::SavePresetResult { .. }
            | RuntimeStoreResult::DeletePresetResult { .. }
            | RuntimeStoreResult::LoadDefaultResult { .. }
            | RuntimeStoreResult::SaveBackupResult { .. }
            | RuntimeStoreResult::SaveRecoveryResult { .. }) => {
                self.apply_store_persistence_result(result)
            }
            result @ (RuntimeStoreResult::MidiListOutputsResult { .. }
            | RuntimeStoreResult::MidiListInputsResult { .. }
            | RuntimeStoreResult::MidiStatus { .. }) => self.apply_midi_result(result),
            result @ (RuntimeStoreResult::SampleListResult { .. }
            | RuntimeStoreResult::SampleListError { .. }
            | RuntimeStoreResult::SamplePreviewStarted { .. }) => {
                self.apply_sample_browser_result(result)
            }
            result @ (RuntimeStoreResult::SystemInfoResult { .. }
            | RuntimeStoreResult::SystemInfoError { .. }
            | RuntimeStoreResult::SetupPortalStatus { .. }) => {
                self.apply_setup_system_result(result)
            }
            result @ RuntimeStoreResult::UserDataRestoreStatus { .. } => {
                self.apply_user_data_restore_result(result)
            }
            result @ RuntimeStoreResult::UserDataTransferStatus { .. } => {
                self.apply_user_data_transfer_result(result)
            }
            result @ RuntimeStoreResult::BluetoothStatus { .. } => {
                self.apply_bluetooth_result(result);
                Ok(())
            }
            result @ (RuntimeStoreResult::StoreError { .. }
            | RuntimeStoreResult::DeviceUpdateStatus { .. }
            | RuntimeStoreResult::DeviceConfigStatus { .. }
            | RuntimeStoreResult::RecordingStatus { .. }
            | RuntimeStoreResult::UsbSdTransferStatus { .. }
            | RuntimeStoreResult::RuntimeFailure { .. }) => {
                self.apply_error_presentation_result(result)
            }
            RuntimeStoreResult::SamplesMissing { paths } => {
                self.apply_missing_samples(&paths);
                Ok(())
            }
            RuntimeStoreResult::Identified { .. }
            | RuntimeStoreResult::OperationSucceeded { .. }
            | RuntimeStoreResult::SamplePreviewError { .. } => Ok(()),
            result @ (RuntimeStoreResult::LoadSystemResult { .. }
            | RuntimeStoreResult::SaveSystemResult { .. }) => {
                self.apply_system_store_result(result).map(|_| ())
            }
        }
    }
}

pub(super) fn is_system_store_operation(operation: &crate::RuntimeOperation) -> bool {
    matches!(
        operation,
        crate::RuntimeOperation::StoreLoadSystem | crate::RuntimeOperation::StoreSaveSystem
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_runner::NativeRuntimeErrorPresentation;
    use crate::NativeRunnerConfig;

    #[test]
    fn device_update_status_is_presented_as_a_toast() {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner
            .apply_store_result(RuntimeStoreResult::DeviceUpdateStatus {
                ok: false,
                message: "helper failed".into(),
            })
            .unwrap();

        assert_eq!(
            runner
                .display
                .toast
                .as_ref()
                .map(|toast| toast.message.as_str()),
            Some("helper failed")
        );
    }

    #[test]
    fn recording_status_is_presented_without_clearing_an_existing_error() {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
            title: "update failed".into(),
            lines: vec!["update failed".into()],
        });

        runner
            .apply_store_result(RuntimeStoreResult::RecordingStatus {
                ok: false,
                message: "Recording incomplete".into(),
                active: false,
            })
            .unwrap();

        assert_eq!(
            runner
                .display
                .toast
                .as_ref()
                .map(|toast| toast.message.as_str()),
            Some("Recording incomplete")
        );
        assert_eq!(
            runner
                .display
                .runtime_error_presentation
                .as_ref()
                .map(|error| error.title.as_str()),
            Some("update failed")
        );
    }
}
