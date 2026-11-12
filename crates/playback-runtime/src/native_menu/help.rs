use super::{
    FactoryLoadKind, NativeMenuAction, NativeMenuHelpTarget, NativeMenuItem, NativeMenuValue,
    PlatformAction,
};

#[cfg(test)]
pub(super) fn collect_help_targets(
    item: &NativeMenuItem,
    path: String,
    targets: &mut Vec<NativeMenuHelpTarget>,
) {
    for child in &item.children {
        if child.label.is_empty() {
            continue;
        }
        let child_path = canonicalize_help_path(&format!("{path} > {}", child.label));
        targets.push(menu_help_target(&child_path, child));
        if !child.children.is_empty() {
            collect_help_targets(child, child_path, targets);
        }
    }
}

pub(super) fn menu_help_target(path: &str, item: &NativeMenuItem) -> NativeMenuHelpTarget {
    let (key, kind) = match &item.value {
        NativeMenuValue::Group => keyed_item_help_target(item, "group"),
        NativeMenuValue::Enum { .. } => keyed_item_help_target(item, "enum"),
        NativeMenuValue::Number { .. } => keyed_item_help_target(item, "number"),
        NativeMenuValue::Bool { .. } => keyed_item_help_target(item, "bool"),
        NativeMenuValue::Text { .. } => keyed_item_help_target(item, "text"),
        NativeMenuValue::Action(action) => menu_action_help_target(action),
    };
    NativeMenuHelpTarget {
        path: path.to_string(),
        key,
        kind,
        label: item.label.clone(),
    }
}

fn keyed_item_help_target(item: &NativeMenuItem, kind: &str) -> (String, String) {
    (item_key_help_target(item), kind.into())
}

fn item_key_help_target(item: &NativeMenuItem) -> String {
    item.key
        .as_ref()
        .map(|key| format!("key:{}", canonicalize_help_key(key)))
        .unwrap_or_default()
}

pub(super) fn canonicalize_help_path(path: &str) -> String {
    let parts = path
        .split(" > ")
        .map(|part| {
            if part.starts_with('L') && part.contains(':') {
                "L*: *".into()
            } else if part.starts_with('I') && part.contains(':') {
                if part == path.rsplit(" > ").next().unwrap_or(part) {
                    let number = part
                        .chars()
                        .skip(1)
                        .take_while(|ch| ch.is_ascii_digit())
                        .collect::<String>();
                    format!("Instrument {number}")
                } else {
                    "Instrument *".into()
                }
            } else if part.starts_with('B') && part.contains(':') {
                "B*: *".into()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>();
    parts.join(" > ")
}

fn canonicalize_help_key(key: &str) -> String {
    let parts = key.split('.').collect::<Vec<_>>();
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            if part.chars().all(|ch| ch.is_ascii_digit())
                && !(index >= 4
                    && parts.get(index - 2) == Some(&"paramMods")
                    && matches!(parts.get(index - 1), Some(&"x") | Some(&"y")))
            {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn menu_action_help_target(action: &NativeMenuAction) -> (String, String) {
    match action {
        NativeMenuAction::ResetBehavior => ("action:reset_behavior".into(), "action".into()),
        NativeMenuAction::SelectBehavior(behavior_id) => (
            format!("action:behavior_select:{behavior_id}"),
            "action".into(),
        ),
        NativeMenuAction::SelectLayerBehavior { behavior_id, .. } => (
            format!("action:behavior_select:{behavior_id}"),
            "action".into(),
        ),
        NativeMenuAction::SelectNoteSet { note_set_id, .. } => (
            format!("action:note_set_select:{note_set_id}"),
            "action".into(),
        ),
        NativeMenuAction::NavigateBack => ("action:navigate_back".into(), "action".into()),
        NativeMenuAction::BehaviorAction(action_type)
        | NativeMenuAction::LayerBehaviorAction { action_type, .. } => (
            format!("action:behavior_action:{action_type}"),
            "action".into(),
        ),
        NativeMenuAction::SetParamBinding { binding, .. } => (
            format!("key:{}", canonicalize_help_key(&binding.key)),
            binding.kind.clone(),
        ),
        NativeMenuAction::ClearParamBinding { .. } => {
            ("action:param_clear".into(), "action".into())
        }
        NativeMenuAction::SetAuxClick {
            action: Some(action),
            ..
        }
        | NativeMenuAction::SetShiftAuxClick {
            action: Some(action),
            ..
        }
        | NativeMenuAction::SetFaderTileAction {
            action: Some(action),
            ..
        } => menu_action_help_target(action),
        NativeMenuAction::SetAuxClick { .. } | NativeMenuAction::SetShiftAuxClick { .. } => {
            ("action:aux_click_set_target".into(), "action".into())
        }
        NativeMenuAction::SetFaderTileAction { .. } => {
            ("action:fader_tile_set_action".into(), "action".into())
        }
        NativeMenuAction::CloneInstrument { .. } => {
            ("action:instrument_clone".into(), "action".into())
        }
        NativeMenuAction::ResetInstrument { .. } => {
            ("action:instrument_reset".into(), "action".into())
        }
        NativeMenuAction::PlatformEffect(effect) => {
            (platform_action_help_key(effect), "action".into())
        }
    }
}

/// The help key of a platform action. Most name their row explicitly; the rest use their wire
/// form, which help rows match with `*` for any slot, voice or port.
fn platform_action_help_key(action: &PlatformAction) -> String {
    let key = match action {
        PlatformAction::PresetSaveAs => "preset_save",
        PlatformAction::PresetSaveCurrent => "preset_save_current",
        PlatformAction::PresetRefresh => "refresh_presets",
        PlatformAction::PresetRenameApply => "preset_rename_apply",
        PlatformAction::PresetLoad(_) => "preset_load:*",
        PlatformAction::PresetDelete(_) => "preset_delete:*",
        PlatformAction::PresetRenamePick(_) => "preset_rename_pick:*",
        PlatformAction::DefaultSave => "default_save",
        PlatformAction::DefaultLoad => "default_load",
        PlatformAction::SystemSave => "system_save",
        PlatformAction::SystemLoad => "system_load",
        PlatformAction::FactoryLoad => "factory_load",
        PlatformAction::SystemClearAll => "system_clear_all",
        PlatformAction::SystemReboot => "system_reboot",
        PlatformAction::SystemShutdown => "system_shutdown",
        PlatformAction::SystemHardwareTest => "system_hardware_test",
        PlatformAction::SystemConfigureWifi => "system_configure_wifi",
        PlatformAction::SystemBackupRestore => "system_backup_restore",
        PlatformAction::SystemUpdateCheck => "system_update_check",
        PlatformAction::SystemUpdateApply => "system_update_apply",
        PlatformAction::SystemRollback => "system_rollback",
        PlatformAction::PlayFxMap => "fx_assign_enter",
        PlatformAction::FaderTileMap => "fader_tile_assign_enter",
        PlatformAction::Signal(_) => "signal_view",
        PlatformAction::MidiPanic => "midi_panic",
        PlatformAction::MidiOutput(None) => "midi_select_output:null",
        PlatformAction::MidiOutput(Some(_)) => "midi_select_output:*",
        PlatformAction::MidiInput(None) => "midi_select_input:null",
        PlatformAction::MidiInput(Some(_)) => "midi_select_input:*",
        PlatformAction::BluetoothScan => "bluetooth_scan",
        PlatformAction::BluetoothToggle(_) => "bluetooth_toggle:*",
        PlatformAction::BluetoothPair(_) => "bluetooth_pair:*",
        PlatformAction::BluetoothForget(_) => "bluetooth_forget:*",
        PlatformAction::TriggerProbabilityAssign(_) => "trigger_probability_assign_enter",
        PlatformAction::SampleAssign { .. } => "sample_assign_enter",
        PlatformAction::SampleOpen { .. } => "sample.open",
        PlatformAction::SampleEnter { .. } => "sample.enter",
        PlatformAction::SampleUp { .. } => "sample.up",
        PlatformAction::SamplePick { .. } => "sample.pick",
        PlatformAction::SampleFavoriteSet { .. } => "sample_favourite_set",
        PlatformAction::SampleFavoriteRemove { .. } => "sample_favourite_remove",
        PlatformAction::FactoryInstrumentLoad { kind, id, .. } => {
            let family = match kind {
                FactoryLoadKind::SampleKit => return "action:sample_kit_load".into(),
                FactoryLoadKind::SynthPreset => "synth_preset_load",
                FactoryLoadKind::FmPreset => "fm_preset_load",
                FactoryLoadKind::PluckPreset => "pluck_preset_load",
                FactoryLoadKind::DrumKit => "drum_kit_load",
            };
            return format!("action:{family}:{id}");
        }
        PlatformAction::SystemControlsHelp
        | PlatformAction::SystemInfo
        | PlatformAction::MaintenanceHold
        | PlatformAction::MaintenanceRelease
        | PlatformAction::UsbSdTransferStart
        | PlatformAction::UsbSdTransferStop
        | PlatformAction::RecordingStartAudio
        | PlatformAction::RecordingStartAudioOled
        | PlatformAction::RecordingStop
        | PlatformAction::RestartSaveChoice
        | PlatformAction::RestartSaving
        | PlatformAction::RestartApp
        | PlatformAction::MidiCcLearn(_)
        | PlatformAction::DrumAssign { .. }
        | PlatformAction::DrumPreview { .. }
        | PlatformAction::MidiDrumAssign(_) => return format!("action:{}", action.as_wire()),
    };
    format!("action:{key}")
}
