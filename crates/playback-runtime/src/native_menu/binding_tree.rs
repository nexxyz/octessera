use super::{
    action_item, group, NativeMenuAction, NativeMenuItem, NativeMenuValue, NativeParamBindingSpec,
};

pub(super) fn binding_group_from_items(
    label: &str,
    items: &[NativeMenuItem],
    target: &str,
) -> Option<NativeMenuItem> {
    let children = items
        .iter()
        .filter_map(|item| binding_tree_from_menu_item(item, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(label, children))
    }
}

pub(super) fn binding_tree_from_menu_item(
    item: &NativeMenuItem,
    target: &str,
) -> Option<NativeMenuItem> {
    if let Some(binding) = binding_spec_from_item(item) {
        return Some(binding_action_from_spec(binding, target));
    }
    let children = item
        .children
        .iter()
        .filter_map(|child| binding_tree_from_menu_item(child, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(item.label.clone(), children))
    }
}

pub(super) fn binding_spec_from_item(item: &NativeMenuItem) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?.clone();
    binding_spec_from_leaf(item, key)
}

pub(super) fn binding_spec_from_leaf(
    item: &NativeMenuItem,
    key: String,
) -> Option<NativeParamBindingSpec> {
    if is_excluded_binding_key(&key) {
        return None;
    }
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(
                if let Some((voice, _)) = drum_voice_field(item.key.as_deref().unwrap_or("")) {
                    format!("V{} {}", voice + 1, item.label)
                } else if item.key.as_deref().is_some_and(|key| key.contains(".fm.")) {
                    format!("FM {}", item.label)
                } else {
                    item.label.clone()
                },
            ),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            user_min: None,
            user_max: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}

pub(super) fn is_excluded_binding_key(key: &str) -> bool {
    key == "behaviorId"
        || key == "playMode"
        || key.contains(".fm.")
            && ![
                ".fm.index",
                ".fm.amp.gainPct",
                ".fm.filter.cutoffHz",
                ".fm.filter.resonance",
            ]
            .iter()
            .any(|suffix| key.ends_with(suffix))
        || key.contains(".pluck.")
            && ![
                ".pluck.decayMs",
                ".pluck.brightnessPct",
                ".pluck.pickPositionPct",
                ".pluck.amp.gainPct",
                ".pluck.filter.cutoffHz",
                ".pluck.filter.resonance",
            ]
            .iter()
            .any(|suffix| key.ends_with(suffix))
        || key.contains(".drum.") && !is_bindable_drum_key(key)
        || key.ends_with(".sample.filter.type")
        || key.ends_with(".sample.filter.envAmountPct")
        || key.ends_with(".sample.filter.keyTrackingPct")
        || key.contains(".sample.ampEnv.")
        || key.contains(".sample.filterEnv.")
        || key.ends_with(".name")
        || key.ends_with(".autoName")
        || key.ends_with(".clone")
        || key.ends_with(".reset")
        || key.ends_with(".params.timeMode")
        || key.ends_with(".params.timeNote")
        || key.contains(".mapping.")
        || key.ends_with(".triggerProbability.map")
}

fn drum_voice_field(key: &str) -> Option<(u8, &str)> {
    let (voice, field) = key.split_once(".drum.voices.")?.1.split_once('.')?;
    Some((voice.parse::<u8>().ok().filter(|voice| *voice < 8)?, field))
}

fn is_bindable_drum_key(key: &str) -> bool {
    let Some((_, field)) = key.split_once(".drum.") else {
        return false;
    };
    matches!(
        field,
        "amp.gainPct" | "filter.cutoffHz" | "filter.resonance"
    ) || drum_voice_field(key)
        .is_some_and(|(_, field)| matches!(field, "tuneSemis" | "decayMs" | "tonePct" | "attackMs"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn binding_action(
    label: &str,
    key: &str,
    kind: &str,
    min: Option<i32>,
    max: Option<i32>,
    step: Option<i32>,
    options: Vec<&str>,
    target: &str,
) -> NativeMenuItem {
    binding_action_from_spec(
        NativeParamBindingSpec {
            key: key.into(),
            label: Some(label.into()),
            kind: kind.into(),
            min,
            max,
            step,
            user_min: None,
            user_max: None,
            options: options.into_iter().map(str::to_string).collect(),
            invert: false,
        },
        target,
    )
}

pub(super) fn binding_action_from_spec(
    binding: NativeParamBindingSpec,
    target: &str,
) -> NativeMenuItem {
    action_item(
        binding.label.clone().unwrap_or_else(|| binding.key.clone()),
        format!("{target}.{}", binding.key),
        NativeMenuAction::SetParamBinding {
            target: target.into(),
            binding,
        },
    )
}
