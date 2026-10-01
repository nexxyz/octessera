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
        let (title, detail) =
            if let Some(detail) = self.preset_store_confirmation_detail(action_type) {
                detail
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
                let (title, kind, rest) = [
                    ("Confirm Synth", "synth preset", "synth.preset:"),
                    ("Confirm FM", "FM preset", "fm.preset:"),
                    ("Confirm Plucked", "Plucked preset", "pluck.preset:"),
                    ("Confirm Sampler", "Sampler kit", "sample.kit:"),
                    ("Confirm Drum", "Drum kit", "drum.kit:"),
                ]
                .into_iter()
                .find_map(|(title, kind, prefix)| {
                    action_type
                        .strip_prefix(prefix)
                        .map(|rest| (title, kind, rest))
                })?;
                let (slot, choice) = rest.split_once(':')?;
                let slot = slot.parse::<usize>().ok()?.checked_add(1)?;
                (title, format!("Load I{slot} {kind} {choice}?"))
            };
        let options = vec!["Cancel".into(), "Confirm".into()];
        let detail_width = if matches!(action_type.as_str(), "system.save" | "system.load") {
            19
        } else {
            28
        };
        Some(NativeConfirmDialog {
            title: title.into(),
            lines: wrap_help_text(&detail, detail_width),
            options,
            cursor: 0,
            action: action.clone(),
            cancel_toast: Some("Cancelled".into()),
            confirm_before_execute: false,
        })
    }

    fn preset_store_confirmation_detail(
        &self,
        action_type: &str,
    ) -> Option<(&'static str, String)> {
        match action_type {
            "preset.saveAs" => Some((
                "Confirm Save",
                format!(
                    "Save preset {}?",
                    clean_preset_name(&self.preset_draft_name)
                ),
            )),
            "preset.saveCurrent" => Some((
                "Confirm Save",
                format!("Overwrite preset {}?", self.current_preset_name.as_ref()?),
            )),
            "preset.renameApply" => {
                let from = self.preset_rename_source.as_ref()?;
                Some((
                    "Confirm Rename",
                    format!(
                        "Rename {from} to {}?",
                        clean_preset_name(&self.preset_draft_name)
                    ),
                ))
            }
            "default.save" => Some(("Confirm Patch", "Save current patch?".into())),
            "default.load" => Some(("Confirm Patch", "Load saved patch?".into())),
            "system.save" => Some(("Confirm System", "Save System settings?".into())),
            "system.load" => Some(("Confirm System", "Load System settings?".into())),
            _ if action_type.starts_with("preset.load:") => Some((
                "Confirm Load",
                format!("Load preset {}?", action_type.strip_prefix("preset.load:")?),
            )),
            _ if action_type.starts_with("preset.delete:") => Some((
                "Confirm Delete",
                format!(
                    "Delete preset {}?",
                    action_type.strip_prefix("preset.delete:")?
                ),
            )),
            _ => None,
        }
    }
}
