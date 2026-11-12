use crate::native_menu::{FactoryLoadKind, NativeMenuAction, PlatformAction};

use super::{clean_preset_name, wrap_help_text, NativeConfirmDialog, NativeRunner};

impl NativeRunner {
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
                lines: wrap_help_text(&detail),
                options: vec!["Cancel".into(), "Confirm".into()],
                cursor: 0,
                action: action.clone(),
                cancel_toast: Some("Canceled".into()),
                confirm_before_execute: false,
            });
        }
        let NativeMenuAction::PlatformEffect(platform_action) = action else {
            return None;
        };
        let (title, detail) = if let Some(detail) =
            self.preset_store_confirmation_detail(platform_action)
        {
            detail
        } else {
            match platform_action {
                PlatformAction::FactoryLoad => ("Confirm Factory", "Load factory settings?".into()),
                PlatformAction::SystemClearAll => {
                    ("Confirm Load Empty", "Load empty patch state?".into())
                }
                PlatformAction::MidiPanic => ("Confirm MIDI", "Send MIDI panic?".into()),
                PlatformAction::SystemReboot => ("Confirm Reboot", "Reboot Octessera?".into()),
                PlatformAction::SystemShutdown => {
                    ("Confirm Shutdown", "Shut down Octessera?".into())
                }
                PlatformAction::UsbSdTransferStart => (
                    "Confirm SD2 Transfer",
                    "USB audio/MIDI disconnect. Host owns OLED SD2 until stopped.".into(),
                ),
                PlatformAction::UsbSdTransferStop => (
                    "Confirm SD2 Transfer",
                    "Eject OLED SD2 on the host first, then stop transfer.".into(),
                ),
                PlatformAction::SystemHardwareTest => {
                    ("Confirm Hardware Test", "Run the hardware test?".into())
                }
                PlatformAction::SystemConfigureWifi => {
                    return Some(NativeConfirmDialog {
                        title: "Open Wi-Fi Setup".into(),
                        lines: vec![
                            "Playback stops.".into(),
                            "Wi-Fi disconnects.".into(),
                            "Setup may change:".into(),
                            "SSH, hostname,".into(),
                            "and login.".into(),
                        ],
                        options: vec!["Cancel".into(), "Open Portal".into()],
                        cursor: 0,
                        action: action.clone(),
                        cancel_toast: Some("Canceled".into()),
                        confirm_before_execute: false,
                    })
                }
                PlatformAction::MaintenanceHold => {
                    return Some(NativeConfirmDialog {
                        title: "Live Show Mode".into(),
                        lines: vec![
                            "48 h: Wi-Fi off,".into(),
                            "so no SSH, updates".into(),
                            "or Wi-Fi setup.".into(),
                            "Housekeeping waits.".into(),
                            "End brings it back.".into(),
                        ],
                        options: vec!["Cancel".into(), "Start".into()],
                        cursor: 0,
                        action: action.clone(),
                        cancel_toast: Some("Canceled".into()),
                        confirm_before_execute: false,
                    })
                }
                PlatformAction::SystemUpdateApply => {
                    ("Confirm Update", "Apply the update now?".into())
                }
                PlatformAction::SystemRollback => (
                    "Confirm Rollback",
                    "Rollback to the previous release?".into(),
                ),
                PlatformAction::FactoryInstrumentLoad { kind, slot, id } => {
                    let (title, family) = match kind {
                        FactoryLoadKind::SynthPreset => ("Confirm Synth", "Synth preset"),
                        FactoryLoadKind::FmPreset => ("Confirm FM", "FM preset"),
                        FactoryLoadKind::PluckPreset => ("Confirm Plucked", "Plucked preset"),
                        FactoryLoadKind::SampleKit => ("Confirm Sampler", "Sampler kit"),
                        FactoryLoadKind::DrumKit => ("Confirm Drum", "Drum kit"),
                    };
                    let name = self.menu.action_label(action).unwrap_or_else(|| id.clone());
                    (title, format!("Load I{} {family} {name}?", slot + 1))
                }
                _ => return None,
            }
        };
        let options = vec!["Cancel".into(), "Confirm".into()];
        Some(NativeConfirmDialog {
            title: title.into(),
            lines: wrap_help_text(&detail),
            options,
            cursor: 0,
            action: action.clone(),
            cancel_toast: Some("Canceled".into()),
            confirm_before_execute: false,
        })
    }

    fn preset_store_confirmation_detail(
        &self,
        action: &PlatformAction,
    ) -> Option<(&'static str, String)> {
        match action {
            PlatformAction::PresetSaveAs => Some((
                "Confirm Save",
                format!(
                    "Save preset {}?",
                    clean_preset_name(&self.patch_store.preset_draft_name)
                ),
            )),
            PlatformAction::PresetSaveCurrent => Some((
                "Confirm Save",
                format!(
                    "Overwrite preset {}?",
                    self.patch_store.current_preset_name.as_ref()?
                ),
            )),
            PlatformAction::PresetRenameApply => {
                let from = self.patch_store.preset_rename_source.as_ref()?;
                Some((
                    "Confirm Rename",
                    format!(
                        "Rename {from} to {}?",
                        clean_preset_name(&self.patch_store.preset_draft_name)
                    ),
                ))
            }
            PlatformAction::DefaultSave => Some(("Confirm Patch", "Save current patch?".into())),
            PlatformAction::DefaultLoad => Some(("Confirm Patch", "Load saved patch?".into())),
            PlatformAction::SystemSave => Some(("Confirm System", "Save System settings?".into())),
            PlatformAction::SystemLoad => Some(("Confirm System", "Load System settings?".into())),
            PlatformAction::PresetLoad(name) => {
                Some(("Confirm Load", format!("Load preset {name}?")))
            }
            PlatformAction::PresetDelete(name) => {
                Some(("Confirm Delete", format!("Delete preset {name}?")))
            }
            _ => None,
        }
    }
}
