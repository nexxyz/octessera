use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};

use super::preset_native_completion::NativePresetResultAction;
use super::restart_settings::DefaultSaveScope;
use super::{
    clean_preset_name, native_factory_payload, portable_patch_payload_for_save,
    NativeManualSaveRequest, NativeRunner, NativeToast,
};

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

    pub(super) fn platform_effect_for_action(
        &mut self,
        action: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        let effect = match action {
            "preset.refresh" => Some(RuntimePlatformEffect::StoreListPresets),
            "default.load" => {
                if self.reject_patch_load_while_save_pending() {
                    None
                } else {
                    Some(RuntimePlatformEffect::StoreLoadDefault)
                }
            }
            "default.save" => {
                if self.restart_settings.has_pending_patch_write() {
                    self.show_toast("Save in progress");
                    return Ok(None);
                }
                let payload =
                    match super::system_persistence::SystemPersistenceState::patch_document(self) {
                        Ok(payload) => payload,
                        Err(error) => {
                            self.present_patch_persistence_error(
                                crate::RuntimeOperation::StoreSaveDefault,
                                error,
                            )?;
                            return Ok(None);
                        }
                    };
                if !self.register_default_write(payload.clone(), DefaultSaveScope::Ordinary) {
                    self.show_toast("Save in progress");
                    return Ok(None);
                }
                Some(RuntimePlatformEffect::StoreSaveDefault {
                    payload,
                    mode: None,
                })
            }
            "system.save" => {
                if self.pending.system_persistence.has_pending_request()
                    || self.pending.system_persistence.has_completed_auto_save()
                    || self.system_apply_pending()
                {
                    self.show_toast("System save pending, try again");
                    None
                } else {
                    Some(RuntimePlatformEffect::StoreSaveSystem {
                        payload:
                            super::system_persistence::SystemPersistenceState::document_for_ordinary_save(
                                self,
                            )?,
                    })
                }
            }
            "system.load" => {
                if self.pending.system_persistence.has_pending_request()
                    || self.pending.system_persistence.has_completed_auto_save()
                    || self.system_apply_pending()
                {
                    self.show_toast("System operation pending, try again");
                    None
                } else {
                    Some(RuntimePlatformEffect::StoreLoadSystem)
                }
            }
            "preset.saveAs" => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.preset_draft_name),
                payload: portable_patch_payload_for_save(&self.config_payload())?,
                mode: None,
            }),
            "preset.renameApply" => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.preset_draft_name),
                payload: portable_patch_payload_for_save(&self.config_payload())?,
                mode: None,
            }),
            "preset.saveCurrent" => match self.current_preset_name.clone() {
                Some(name) => Some(RuntimePlatformEffect::StoreSavePreset {
                    name,
                    payload: portable_patch_payload_for_save(&self.config_payload())?,
                    mode: Some("overwrite".into()),
                }),
                None => None,
            },
            action if action.starts_with("preset.load:") => {
                if self.reject_patch_load_while_save_pending() {
                    None
                } else {
                    action
                        .strip_prefix("preset.load:")
                        .map(|name| RuntimePlatformEffect::StoreLoadPreset { name: name.into() })
                }
            }
            action if action.starts_with("preset.delete:") => action
                .strip_prefix("preset.delete:")
                .map(|name| RuntimePlatformEffect::StoreDeletePreset { name: name.into() }),
            "midi.panic" => Some(RuntimePlatformEffect::MidiPanic),
            "system.reboot" | "system.shutdown" => {
                match super::system_persistence::SystemPersistenceState::patch_document(self) {
                    Ok(payload) => Some(RuntimePlatformEffect::StoreSaveRecovery { payload }),
                    Err(error) => {
                        self.present_patch_persistence_error(
                            crate::RuntimeOperation::StoreSaveRecovery,
                            error,
                        )?;
                        None
                    }
                }
            }
            "usb.sdTransferStart" => Some(RuntimePlatformEffect::UsbSdTransferStart),
            "usb.sdTransferStop" => Some(RuntimePlatformEffect::UsbSdTransferStop),
            "recording.startAudio" => Some(RuntimePlatformEffect::RecordingStartAudio {
                max_minutes: self.recording_max_minutes,
            }),
            "recording.startAudioOled" => Some(RuntimePlatformEffect::RecordingStartAudioOled {
                max_minutes: self.recording_max_minutes,
            }),
            "recording.stop" => Some(RuntimePlatformEffect::RecordingStop),
            "system.hardwareTest" => Some(RuntimePlatformEffect::HardwareTest),
            "system.info" => Some(RuntimePlatformEffect::SystemInfoRequest),
            "system.configureWifi" => Some(RuntimePlatformEffect::SetupPortalOpen),
            "system.backupRestore" => Some(RuntimePlatformEffect::UserDataTransferOpen),
            "system.updateCheck" => Some(RuntimePlatformEffect::UpdateCheck),
            "system.updateApply" => Some(RuntimePlatformEffect::UpdateApply),
            "system.rollback" => Some(RuntimePlatformEffect::Rollback),
            action if action.starts_with("midi.output:") => {
                let id = action.strip_prefix("midi.output:").unwrap_or_default();
                Some(RuntimePlatformEffect::MidiSelectOutput {
                    id: if id.is_empty() { None } else { Some(id.into()) },
                })
            }
            action if action.starts_with("midi.input:") => {
                let id = action.strip_prefix("midi.input:").unwrap_or_default();
                Some(RuntimePlatformEffect::MidiSelectInput {
                    id: if id.is_empty() { None } else { Some(id.into()) },
                })
            }
            action => self.bluetooth_effect_for_action(action),
        };
        Ok(effect)
    }

    pub(super) fn reject_patch_load_while_save_pending(&mut self) -> bool {
        let save_pending = self.restart_settings.has_pending_patch_write()
            || self.pending.pending_save_revision.is_some()
            || matches!(
                &self.pending.manual_save_request,
                Some(NativeManualSaveRequest::Default)
            )
            || (self.auto_save_default
                && (self.pending.pending_autosave_payload_due_at.is_some() || self.config_dirty));
        if save_pending {
            self.show_toast("Save pending, try again");
        }
        save_pending
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
            | RuntimeStoreResult::SampleListError { .. }) => {
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
            | RuntimeStoreResult::RecordingStatus { .. }
            | RuntimeStoreResult::UsbSdTransferStatus { .. }
            | RuntimeStoreResult::RuntimeFailure { .. }) => {
                self.apply_error_presentation_result(result)
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
