use crate::protocol::RuntimePlatformEffect;

use super::restart_settings::DefaultSaveScope;
use super::{
    clean_preset_name, portable_patch_payload_for_save, NativeManualSaveRequest, NativeRunner,
};

impl NativeRunner {
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
            "system.reboot" | "system.shutdown" | "system.restartApp" => {
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
            "maintenance.hold" => Some(RuntimePlatformEffect::MaintenanceHold { active: true }),
            "maintenance.release" => Some(RuntimePlatformEffect::MaintenanceHold { active: false }),
            "recording.startAudio" => Some(RuntimePlatformEffect::RecordingStartAudio {
                max_minutes: self.recording_max_minutes,
            }),
            "recording.startAudioOled" => Some(RuntimePlatformEffect::RecordingStartAudioOled {
                max_minutes: self.recording_max_minutes,
            }),
            "recording.stop" => Some(RuntimePlatformEffect::RecordingStop),
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
}
