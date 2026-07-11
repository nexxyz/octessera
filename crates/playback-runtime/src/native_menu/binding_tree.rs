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
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
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
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}

pub(super) fn is_excluded_binding_key(key: &str) -> bool {
    key == "behaviorId"
        || key == "sparksMode"
        || key.ends_with(".name")
        || key.ends_with(".autoName")
        || key.ends_with(".clone")
        || key.ends_with(".reset")
        || key.contains(".mapping.")
        || key.ends_with(".triggerProbability.map")
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
