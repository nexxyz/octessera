use super::{NativeMenuAction, NativeMenuHelpTarget, NativeMenuItem, NativeMenuValue};

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
        NativeMenuValue::Group => (String::new(), "group"),
        NativeMenuValue::Enum { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "enum",
        ),
        NativeMenuValue::Number { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "number",
        ),
        NativeMenuValue::Bool { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "bool",
        ),
        NativeMenuValue::Text { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "text",
        ),
        NativeMenuValue::Action(action) => (menu_action_help_key(action), "action"),
    };
    NativeMenuHelpTarget {
        path: path.to_string(),
        key,
        kind: kind.into(),
        label: item.label.clone(),
    }
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

fn menu_action_help_key(action: &NativeMenuAction) -> String {
    match action {
        NativeMenuAction::ResetBehavior => "action:reset_behavior".into(),
        NativeMenuAction::SelectBehavior(_) => "action:behavior_select".into(),
        NativeMenuAction::NavigateBack => "action:navigate_back".into(),
        NativeMenuAction::BehaviorAction(action_type) => {
            format!("action:behavior_action:{action_type}")
        }
        NativeMenuAction::SetParamBinding { .. } => "action:param_bind".into(),
        NativeMenuAction::ClearParamBinding { .. } => "action:param_clear".into(),
        NativeMenuAction::SetAuxClick { .. } => "action:aux_click_set_target".into(),
        NativeMenuAction::CloneInstrument { .. } => "action:instrument_clone".into(),
        NativeMenuAction::ResetInstrument { .. } => "action:instrument_reset".into(),
        NativeMenuAction::PlatformEffect(effect) => platform_effect_help_key(effect),
    }
}

fn platform_effect_help_key(effect: &str) -> String {
    preset_effect_help_key(effect)
        .or_else(|| default_system_effect_help_key(effect))
        .or_else(|| midi_effect_help_key(effect))
        .or_else(|| sample_effect_help_key(effect))
        .or_else(|| synth_effect_help_key(effect))
        .or_else(|| trigger_probability_effect_help_key(effect))
        .unwrap_or_else(|| format!("action:{effect}"))
}

fn preset_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        "preset.saveAs" => Some("action:preset_save".into()),
        "preset.saveCurrent" => Some("action:preset_save_current".into()),
        "preset.refresh" => Some("action:refresh_presets".into()),
        "preset.renameApply" => Some("action:preset_rename_apply".into()),
        value if value.starts_with("preset.load:") => Some("action:preset_load:*".into()),
        value if value.starts_with("preset.delete:") => Some("action:preset_delete:*".into()),
        value if value.starts_with("preset.renamePick:") => {
            Some("action:preset_rename_pick:*".into())
        }
        _ => None,
    }
}

fn default_system_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        "default.save" => Some("action:default_save".into()),
        "default.load" => Some("action:default_load".into()),
        "factory.load" => Some("action:factory_load".into()),
        "system.clearAll" => Some("action:system_clear_all".into()),
        "system.reboot" => Some("action:system_reboot".into()),
        "system.shutdown" => Some("action:system_shutdown".into()),
        "system.hardwareTest" => Some("action:system_hardware_test".into()),
        "system.updateCheck" => Some("action:system_update_check".into()),
        "system.updateApply" => Some("action:system_update_apply".into()),
        "system.rollback" => Some("action:system_rollback".into()),
        "sparks.fx.map" => Some("action:fx_assign_enter".into()),
        _ => None,
    }
}

fn midi_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        "midi.panic" => Some("action:midi_panic".into()),
        value if value.starts_with("midi.out:") || value.starts_with("midi.output:") => {
            Some(midi_output_effect_help_key(value))
        }
        value if value.starts_with("midi.in:") || value.starts_with("midi.input:") => {
            Some(midi_input_effect_help_key(value))
        }
        _ => None,
    }
}

fn midi_output_effect_help_key(effect: &str) -> String {
    if effect == "midi.out:" || effect == "midi.output:" {
        "action:midi_select_output:null".into()
    } else {
        "action:midi_select_output:*".into()
    }
}

fn midi_input_effect_help_key(effect: &str) -> String {
    if effect == "midi.in:" || effect == "midi.input:" {
        "action:midi_select_input:null".into()
    } else {
        "action:midi_select_input:*".into()
    }
}

fn sample_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        value if value.starts_with("sample.open:") => Some("action:sample.open".into()),
        value if value.starts_with("sample.up:") => Some("action:sample.up".into()),
        value if value.starts_with("sample.enter:") => Some("action:sample.enter".into()),
        value if value.starts_with("sample.pick:") => Some("action:sample.pick".into()),
        value if value.starts_with("sample.preview:") => Some("action:sample_preview".into()),
        value if value.starts_with("sample.assign:") => Some("action:sample_assign_enter".into()),
        value if value.starts_with("sample.favorite.set:") => {
            Some("action:sample_favourite_set".into())
        }
        value if value.starts_with("sample.favorite.remove:") => {
            Some("action:sample_favourite_remove".into())
        }
        _ => None,
    }
}

fn synth_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        value if value.starts_with("synth.preset:") => Some("action:synth_preset_load".into()),
        _ => None,
    }
}

fn trigger_probability_effect_help_key(effect: &str) -> Option<String> {
    match effect {
        value if value.starts_with("trigger.probability.assign:") => {
            Some("action:trigger_probability_assign_enter".into())
        }
        _ => None,
    }
}
