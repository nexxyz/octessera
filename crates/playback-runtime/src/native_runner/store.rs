use crate::native_menu::NativeMenuAction;
use crate::protocol::{RuntimePlatformEffect, RuntimeStoreResult};

use super::{
    clean_preset_name, native_factory_payload, wrap_help_text, NativeConfirmDialog, NativeRunner,
    NativeSampleBrowser, NativeToast,
};

impl NativeRunner {
    pub(super) fn apply_factory_payload(&mut self) -> Result<(), String> {
        self.apply_config_payload(native_factory_payload())?;
        self.toast = Some(NativeToast {
            message: "Factory loaded".into(),
            offset: 0,
        });
        Ok(())
    }

    pub(super) fn platform_effect_for_action(&self, action: &str) -> Option<RuntimePlatformEffect> {
        match action {
            "preset.refresh" => Some(RuntimePlatformEffect::StoreListPresets),
            "default.load" => Some(RuntimePlatformEffect::StoreLoadDefault),
            "default.save" => Some(RuntimePlatformEffect::StoreSaveDefault {
                payload: self.config_payload(),
                mode: None,
            }),
            "preset.saveAs" => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.preset_draft_name),
                payload: self.config_payload(),
                mode: None,
            }),
            "preset.renameApply" => Some(RuntimePlatformEffect::StoreSavePreset {
                name: clean_preset_name(&self.preset_draft_name),
                payload: self.config_payload(),
                mode: None,
            }),
            "preset.saveCurrent" => self.current_preset_name.as_ref().map(|name| {
                RuntimePlatformEffect::StoreSavePreset {
                    name: name.clone(),
                    payload: self.config_payload(),
                    mode: Some("overwrite".into()),
                }
            }),
            action if action.starts_with("preset.load:") => action
                .strip_prefix("preset.load:")
                .map(|name| RuntimePlatformEffect::StoreLoadPreset { name: name.into() }),
            action if action.starts_with("preset.delete:") => action
                .strip_prefix("preset.delete:")
                .map(|name| RuntimePlatformEffect::StoreDeletePreset { name: name.into() }),
            "midi.panic" => Some(RuntimePlatformEffect::MidiPanic),
            "system.reboot" => Some(RuntimePlatformEffect::Reboot),
            "system.shutdown" => Some(RuntimePlatformEffect::Shutdown),
            "system.hardwareTest" => Some(RuntimePlatformEffect::HardwareTest),
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
        }
    }

    pub(super) fn confirmation_for_action(
        &self,
        action: &NativeMenuAction,
    ) -> Option<NativeConfirmDialog> {
        let instrument_detail = match action {
            NativeMenuAction::CloneInstrument { index } => {
                Some(("Confirm Clone", format!("Clone instrument I{}?", index + 1)))
            }
            NativeMenuAction::ResetInstrument { index } => {
                Some(("Confirm Reset", format!("Reset instrument I{}?", index + 1)))
            }
            _ => None,
        };
        if let Some((title, detail)) = instrument_detail {
            return Some(NativeConfirmDialog {
                title: title.into(),
                lines: wrap_help_text(&detail, 28),
                options: vec!["Cancel".into(), "Confirm".into()],
                cursor: 0,
                action: action.clone(),
            });
        }
        let NativeMenuAction::PlatformEffect(action_type) = action else {
            return None;
        };
        let (title, detail) = if action_type == "preset.saveAs" {
            (
                "Confirm Save",
                format!(
                    "Save preset {}?",
                    clean_preset_name(&self.preset_draft_name)
                ),
            )
        } else if action_type == "preset.saveCurrent" {
            let name = self.current_preset_name.as_ref()?;
            ("Confirm Save", format!("Overwrite preset {name}?"))
        } else if action_type == "preset.renameApply" {
            let from = self.preset_rename_source.as_ref()?;
            (
                "Confirm Rename",
                format!(
                    "Rename {from} to {}?",
                    clean_preset_name(&self.preset_draft_name)
                ),
            )
        } else if let Some(name) = action_type.strip_prefix("preset.load:") {
            ("Confirm Load", format!("Load preset {name}?"))
        } else if let Some(name) = action_type.strip_prefix("preset.delete:") {
            ("Confirm Delete", format!("Delete preset {name}?"))
        } else if action_type == "default.save" {
            ("Confirm Default", "Save current default?".into())
        } else if action_type == "default.load" {
            ("Confirm Default", "Load saved default?".into())
        } else if action_type == "factory.load" {
            ("Confirm Factory", "Load factory settings?".into())
        } else if action_type == "midi.panic" {
            ("Confirm MIDI", "Send MIDI panic?".into())
        } else if action_type == "system.reboot" {
            ("Confirm Reboot", "Reboot cellSymphony?".into())
        } else if action_type == "system.shutdown" {
            ("Confirm Shutdown", "Shut down cellSymphony?".into())
        } else if action_type == "system.hardwareTest" {
            ("Confirm Hardware Test", "Run the hardware test?".into())
        } else if action_type == "system.updateApply" {
            ("Confirm Update", "Apply the update now?".into())
        } else if action_type == "system.rollback" {
            (
                "Confirm Rollback",
                "Rollback to the previous release?".into(),
            )
        } else if let Some(rest) = action_type.strip_prefix("synth.preset:") {
            let preset = rest.split(':').nth(1).unwrap_or("preset");
            ("Confirm Synth", format!("Load synth preset {preset}?"))
        } else {
            return None;
        };
        Some(NativeConfirmDialog {
            title: title.into(),
            lines: wrap_help_text(&detail, 28),
            options: vec!["Cancel".into(), "Confirm".into()],
            cursor: 0,
            action: action.clone(),
        })
    }

    pub(super) fn apply_store_result(&mut self, result: RuntimeStoreResult) -> Result<(), String> {
        match result {
            RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            } => {
                self.apply_config_payload(payload)?;
                self.toast = Some(NativeToast {
                    message: "Default loaded".into(),
                    offset: 0,
                });
            }
            RuntimeStoreResult::LoadPresetResult { name, payload } => {
                if let Some(payload) = payload {
                    self.apply_config_payload(payload)?;
                }
                self.toast = Some(NativeToast {
                    message: format!("Loaded {name}"),
                    offset: 0,
                });
                self.current_preset_name = Some(name);
            }
            RuntimeStoreResult::SavePresetResult { name, .. } => {
                if let Some(source) = self.preset_rename_source.take() {
                    if source != name {
                        self.outbox.push_platform_effect(
                            RuntimePlatformEffect::StoreDeletePreset { name: source },
                        );
                    }
                }
                self.toast = Some(NativeToast {
                    message: format!("Saved {name}"),
                    offset: 0,
                });
                self.current_preset_name = Some(name);
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::DeletePresetResult { name, ok } if ok => {
                if self.current_preset_name.as_deref() == Some(name.as_str()) {
                    self.current_preset_name = None;
                }
                self.toast = Some(NativeToast {
                    message: format!("Deleted {name}"),
                    offset: 0,
                });
            }
            RuntimeStoreResult::SaveDefaultResult { ok, is_auto: _ } if ok => {
                self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
                self.auto_save_flash_pulses_remaining = 8;
                self.toast = Some(NativeToast {
                    message: "Saved default".into(),
                    offset: 0,
                });
            }
            RuntimeStoreResult::StoreError { message } => {
                self.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::ListPresetsResult { names } => {
                self.preset_names = names;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiListOutputsResult { outputs } => {
                self.midi_outputs = outputs;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiListInputsResult { inputs } => {
                self.midi_inputs = inputs;
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::MidiStatus {
                ok,
                message,
                selected_out_id,
                selected_in_id,
            } => {
                self.midi_status = Some(if ok {
                    "MIDI ok".into()
                } else {
                    message.unwrap_or_else(|| "MIDI error".into())
                });
                self.selected_midi_output_id = selected_out_id;
                self.selected_midi_input_id = selected_in_id;
            }
            RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries,
            } if self.sample_browser_matches(instrument_slot, sample_slot, &dir) => {
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries,
                });
                self.menu.rebuild(self.menu_config());
            }
            RuntimeStoreResult::SampleListError {
                instrument_slot,
                sample_slot,
                dir,
                message,
            } if self.sample_browser_matches(instrument_slot, sample_slot, &dir) => {
                self.sample_browser = Some(NativeSampleBrowser {
                    instrument_slot,
                    sample_slot,
                    dir,
                    entries: vec![],
                });
                self.toast = Some(NativeToast { message, offset: 0 });
                self.menu.rebuild(self.menu_config());
            }
            _ => {}
        }
        Ok(())
    }

    fn sample_browser_matches(
        &self,
        instrument_slot: usize,
        sample_slot: usize,
        dir: &str,
    ) -> bool {
        self.sample_browser.as_ref().is_some_and(|browser| {
            browser.instrument_slot == instrument_slot
                && browser.sample_slot == sample_slot
                && browser.dir == dir
        })
    }
}
