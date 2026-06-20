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
            if part.starts_with('P') && part.contains(':') {
                "P*: *".into()
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
        NativeMenuAction::BehaviorAction(action_type) => {
            format!("action:behavior_action:{action_type}")
        }
        NativeMenuAction::SetParamBinding { .. } => "action:param_bind".into(),
        NativeMenuAction::ClearParamBinding { .. } => "action:param_clear".into(),
        NativeMenuAction::SetAuxClick { .. } => "action:aux_click_set_target".into(),
        NativeMenuAction::CloneInstrument { .. } => "action:instrument_clone".into(),
        NativeMenuAction::ResetInstrument { .. } => "action:instrument_reset".into(),
        NativeMenuAction::PlatformEffect(effect) => match effect.as_str() {
            "preset.saveAs" => "action:preset_save".into(),
            "preset.saveCurrent" => "action:preset_save_current".into(),
            "preset.refresh" => "action:refresh_presets".into(),
            "preset.renameApply" => "action:preset_rename_apply".into(),
            "default.save" => "action:default_save".into(),
            "default.load" => "action:default_load".into(),
            "factory.load" => "action:factory_load".into(),
            "system.shutdown" => "action:system_shutdown".into(),
            "midi.panic" => "action:midi_panic".into(),
            "dance.fx.map" => "action:fx_assign_enter".into(),
            value if value.starts_with("preset.load:") => "action:preset_load:*".into(),
            value if value.starts_with("preset.delete:") => "action:preset_delete:*".into(),
            value if value.starts_with("preset.renamePick:") => {
                "action:preset_rename_pick:*".into()
            }
            value if value.starts_with("midi.out:") || value.starts_with("midi.output:") => {
                if value == "midi.out:" || value == "midi.output:" {
                    "action:midi_select_output:null".into()
                } else {
                    "action:midi_select_output:*".into()
                }
            }
            value if value.starts_with("midi.in:") || value.starts_with("midi.input:") => {
                if value == "midi.in:" || value == "midi.input:" {
                    "action:midi_select_input:null".into()
                } else {
                    "action:midi_select_input:*".into()
                }
            }
            value if value.starts_with("sample.open:") => "action:sample_browser_open".into(),
            value if value.starts_with("sample.up:") => "action:sample_browser_up".into(),
            value if value.starts_with("sample.pick:") => "action:sample_browser_pick".into(),
            value if value.starts_with("sample.preview:") => "action:sample_preview".into(),
            value if value.starts_with("sample.assign:") => "action:sample_assign_enter".into(),
            value if value.starts_with("synth.preset:") => "action:synth_preset_load".into(),
            value if value.starts_with("trigger.probability.assign:") => {
                "action:trigger_probability_assign_enter".into()
            }
            value => format!("action:{value}"),
        },
    }
}
