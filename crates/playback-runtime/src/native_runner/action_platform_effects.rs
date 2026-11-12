use crate::native_menu::PlatformAction;
use crate::protocol::{RuntimeBluetoothDeviceAction, RuntimePlatformEffect};

use super::restart_settings::DefaultSaveScope;
use super::{clean_preset_name, portable_patch_payload_for_save, NativeRunner};

impl NativeRunner {
    /// The host effect of a platform action, `None` for actions the runner handles itself;
    /// `execute_platform_action` decides which actions reach it.
    pub(super) fn platform_effect_for_action(
        &mut self,
        action: &PlatformAction,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        let effect = match action {
            PlatformAction::PresetRefresh => Some(RuntimePlatformEffect::StoreListPresets),
            PlatformAction::DefaultLoad => {
                self.admit_patch_load(RuntimePlatformEffect::StoreLoadDefault)
            }
            PlatformAction::DefaultSave => {
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
            PlatformAction::SystemSave => {
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
            PlatformAction::SystemLoad => {
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
            PlatformAction::PresetSaveAs => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.patch_store.preset_draft_name),
                payload: portable_patch_payload_for_save(&self.config_payload())?,
                mode: None,
            }),
            PlatformAction::PresetRenameApply => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.patch_store.preset_draft_name),
                payload: portable_patch_payload_for_save(&self.config_payload())?,
                mode: None,
            }),
            PlatformAction::PresetSaveCurrent => match self.patch_store.current_preset_name.clone()
            {
                Some(name) => Some(RuntimePlatformEffect::StoreSavePreset {
                    name,
                    payload: portable_patch_payload_for_save(&self.config_payload())?,
                    mode: Some("overwrite".into()),
                }),
                None => None,
            },
            PlatformAction::PresetLoad(name) => {
                self.admit_patch_load(RuntimePlatformEffect::StoreLoadPreset { name: name.clone() })
            }
            PlatformAction::PresetDelete(name) => {
                Some(RuntimePlatformEffect::StoreDeletePreset { name: name.clone() })
            }
            PlatformAction::MidiPanic => Some(RuntimePlatformEffect::MidiPanic),
            PlatformAction::UsbSdTransferStart => Some(RuntimePlatformEffect::UsbSdTransferStart),
            PlatformAction::UsbSdTransferStop => Some(RuntimePlatformEffect::UsbSdTransferStop),
            PlatformAction::MaintenanceHold => {
                Some(RuntimePlatformEffect::MaintenanceHold { active: true })
            }
            PlatformAction::MaintenanceRelease => {
                Some(RuntimePlatformEffect::MaintenanceHold { active: false })
            }
            PlatformAction::RecordingStartAudio => {
                Some(RuntimePlatformEffect::RecordingStartAudio {
                    max_minutes: self.audio_io.recording_max_minutes,
                })
            }
            PlatformAction::RecordingStartAudioOled => {
                Some(RuntimePlatformEffect::RecordingStartAudioOled {
                    max_minutes: self.audio_io.recording_max_minutes,
                })
            }
            PlatformAction::RecordingStop => Some(RuntimePlatformEffect::RecordingStop),
            PlatformAction::SystemInfo => Some(RuntimePlatformEffect::SystemInfoRequest),
            PlatformAction::SystemBackupRestore => {
                Some(RuntimePlatformEffect::UserDataTransferOpen)
            }
            PlatformAction::SystemUpdateCheck => Some(RuntimePlatformEffect::UpdateCheck),
            PlatformAction::SystemUpdateApply => Some(RuntimePlatformEffect::UpdateApply),
            PlatformAction::SystemRollback => Some(RuntimePlatformEffect::Rollback),
            PlatformAction::MidiOutput(id) => {
                Some(RuntimePlatformEffect::MidiSelectOutput { id: id.clone() })
            }
            PlatformAction::MidiInput(id) => {
                Some(RuntimePlatformEffect::MidiSelectInput { id: id.clone() })
            }
            PlatformAction::BluetoothScan => Some(self.start_bluetooth_scan()),
            PlatformAction::BluetoothToggle(address) => {
                Some(RuntimePlatformEffect::BluetoothDevice {
                    address: address.clone(),
                    action: self.bluetooth_toggle_action(address),
                })
            }
            PlatformAction::BluetoothPair(address) => {
                Some(RuntimePlatformEffect::BluetoothDevice {
                    address: address.clone(),
                    action: RuntimeBluetoothDeviceAction::Pair,
                })
            }
            PlatformAction::BluetoothForget(address) => {
                Some(RuntimePlatformEffect::BluetoothDevice {
                    address: address.clone(),
                    action: RuntimeBluetoothDeviceAction::Forget,
                })
            }
            _ => None,
        };
        Ok(effect)
    }

    /// The recovery save of the patch that precedes a reboot, shutdown or app restart.
    pub(super) fn recovery_save_effect(&mut self) -> Result<Option<RuntimePlatformEffect>, String> {
        match super::system_persistence::SystemPersistenceState::patch_document(self) {
            Ok(payload) => Ok(Some(RuntimePlatformEffect::StoreSaveRecovery { payload })),
            Err(error) => {
                self.present_patch_persistence_error(
                    crate::RuntimeOperation::StoreSaveRecovery,
                    error,
                )?;
                Ok(None)
            }
        }
    }
}
