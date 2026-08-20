use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};

use super::{
    clean_preset_name, native_factory_payload, portable_patch_payload_for_save, NativeRunner,
    NativeToast,
};

impl NativeRunner {
    pub(super) fn apply_factory_payload(&mut self) -> Result<(), String> {
        self.apply_config_payload(native_factory_payload())?;
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
            "default.load" => Some(RuntimePlatformEffect::StoreLoadDefault),
            "default.save" => Some(RuntimePlatformEffect::StoreSaveDefault {
                payload: self.config_payload(),
                mode: None,
            }),
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
            action if action.starts_with("preset.load:") => action
                .strip_prefix("preset.load:")
                .map(|name| RuntimePlatformEffect::StoreLoadPreset { name: name.into() }),
            action if action.starts_with("preset.delete:") => action
                .strip_prefix("preset.delete:")
                .map(|name| RuntimePlatformEffect::StoreDeletePreset { name: name.into() }),
            "midi.panic" => Some(RuntimePlatformEffect::MidiPanic),
            "system.reboot" => Some(RuntimePlatformEffect::StoreSaveRecovery {
                payload: self.config_payload(),
            }),
            "system.shutdown" => Some(RuntimePlatformEffect::StoreSaveRecovery {
                payload: self.config_payload(),
            }),
            "audio.applyReboot" | "usb.applyReboot" => {
                Some(RuntimePlatformEffect::ApplyDeviceConfigReboot {
                    payload: self.config_payload(),
                })
            }
            "usb.sdTransferStart" => Some(RuntimePlatformEffect::UsbSdTransferStart),
            "usb.sdTransferStop" => Some(RuntimePlatformEffect::UsbSdTransferStop),
            "recording.startAudio" => Some(RuntimePlatformEffect::RecordingStartAudio {
                max_minutes: self.recording_max_minutes,
            }),
            "recording.stop" => Some(RuntimePlatformEffect::RecordingStop),
            "system.hardwareTest" => Some(RuntimePlatformEffect::HardwareTest),
            "system.info" => Some(RuntimePlatformEffect::SystemInfoRequest),
            "system.configureWifi" => Some(RuntimePlatformEffect::SetupPortalOpen),
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
            _ => None,
        };
        Ok(effect)
    }

    pub(super) fn apply_store_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
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
                let operation = result.operation();
                let succeeded = result.error_facts().is_none();
                let is_save_operation = matches!(
                    &operation,
                    crate::protocol::RuntimeOperation::StoreSavePreset
                        | crate::protocol::RuntimeOperation::StoreSaveDefault
                );
                let apply_result = self.apply_store_result(*result);
                if let Err(error) = apply_result {
                    if operation == crate::protocol::RuntimeOperation::StoreLoadDefault
                        && self.restore_rehydration_pending()
                    {
                        self.fail_restore_rehydration();
                    }
                    return Err(error);
                }
                if !succeeded && is_save_operation {
                    self.retry_config_save_after_restore_failure();
                }
                if succeeded && is_save_operation {
                    self.acknowledge_config_save(revision);
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
            result @ (RuntimeStoreResult::ListPresetsResult { .. }
            | RuntimeStoreResult::LoadPresetResult { .. }
            | RuntimeStoreResult::SavePresetResult { .. }
            | RuntimeStoreResult::DeletePresetResult { .. }
            | RuntimeStoreResult::LoadDefaultResult { .. }
            | RuntimeStoreResult::SaveDefaultResult { .. }
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
            result @ (RuntimeStoreResult::StoreError { .. }
            | RuntimeStoreResult::DeviceUpdateStatus { .. }
            | RuntimeStoreResult::UsbSdTransferStatus { .. }
            | RuntimeStoreResult::RuntimeFailure { .. }) => {
                self.apply_error_presentation_result(result)
            }
            RuntimeStoreResult::Identified { .. }
            | RuntimeStoreResult::OperationSucceeded { .. }
            | RuntimeStoreResult::SamplePreviewError { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
