use crate::native_menu::NativeMenuAction;

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
                lines: wrap_help_text(&detail, 28),
                options: vec!["Cancel".into(), "Confirm".into()],
                cursor: 0,
                action: action.clone(),
                cancel_toast: Some("Cancelled".into()),
                confirm_before_execute: false,
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
        } else if action_type == "system.clearAll" {
            ("Confirm Load Empty", "Load empty patch state?".into())
        } else if action_type == "midi.panic" {
            ("Confirm MIDI", "Send MIDI panic?".into())
        } else if action_type == "system.reboot" {
            ("Confirm Reboot", "Reboot Octessera?".into())
        } else if action_type == "system.shutdown" {
            ("Confirm Shutdown", "Shut down Octessera?".into())
        } else if action_type == "audio.applyReboot" || action_type == "usb.applyReboot" {
            ("Confirm Audio", "Save audio settings and reboot?".into())
        } else if action_type == "usb.sdTransferStart" {
            (
                "Confirm SD2 Transfer",
                "USB audio/MIDI disconnect. Host owns OLED SD2 until stopped.".into(),
            )
        } else if action_type == "usb.sdTransferStop" {
            (
                "Confirm SD2 Transfer",
                "Eject OLED SD2 on the host first, then stop transfer.".into(),
            )
        } else if action_type == "system.hardwareTest" {
            ("Confirm Hardware Test", "Run the hardware test?".into())
        } else if action_type == "system.configureWifi" {
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
                cancel_toast: Some("Cancelled".into()),
                confirm_before_execute: false,
            });
        } else if action_type == "system.updateApply" {
            ("Confirm Update", "Apply the update now?".into())
        } else if action_type == "system.rollback" {
            (
                "Confirm Rollback",
                "Rollback to the previous release?".into(),
            )
        } else {
            let rest = action_type.strip_prefix("synth.preset:")?;
            let preset = rest.split(':').nth(1).unwrap_or("preset");
            ("Confirm Synth", format!("Load synth preset {preset}?"))
        };
        let options = if action_type == "audio.applyReboot" || action_type == "usb.applyReboot" {
            vec!["Cancel".into(), "Save & Reboot".into()]
        } else {
            vec!["Cancel".into(), "Confirm".into()]
        };
        Some(NativeConfirmDialog {
            title: title.into(),
            lines: wrap_help_text(&detail, 28),
            options,
            cursor: 0,
            action: action.clone(),
            cancel_toast: Some("Cancelled".into()),
            confirm_before_execute: false,
        })
    }
}
