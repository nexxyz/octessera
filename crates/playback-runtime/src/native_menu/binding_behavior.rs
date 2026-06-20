use super::binding_tree::{
    binding_action_from_spec, binding_spec_from_item, binding_spec_from_leaf,
};
use super::{group, NativeMenuConfig, NativeMenuItem, NativeParamBindingSpec};

pub(super) fn behavior_binding_groups(
    config: &NativeMenuConfig,
    target: &str,
) -> Option<NativeMenuItem> {
    let children = config
        .part_labels
        .iter()
        .enumerate()
        .filter_map(|(part_index, label)| {
            binding_group_from_behavior_items(
                label,
                &config.l1_items,
                target,
                config.active_part_index,
                part_index,
            )
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group("Behavior", children))
    }
}

fn binding_group_from_behavior_items(
    label: &str,
    items: &[NativeMenuItem],
    target: &str,
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeMenuItem> {
    let children = items
        .iter()
        .filter_map(|item| {
            binding_tree_from_behavior_item(item, target, active_part_index, target_part_index)
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(label, children))
    }
}

fn binding_tree_from_behavior_item(
    item: &NativeMenuItem,
    target: &str,
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeMenuItem> {
    if let Some(binding) =
        binding_spec_from_behavior_item(item, active_part_index, target_part_index)
    {
        return Some(binding_action_from_spec(binding, target));
    }
    let children = item
        .children
        .iter()
        .filter_map(|child| {
            binding_tree_from_behavior_item(child, target, active_part_index, target_part_index)
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(item.label.clone(), children))
    }
}

fn binding_spec_from_behavior_item(
    item: &NativeMenuItem,
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?;
    if let Some(field) = key.strip_prefix("behavior.") {
        let rewritten = format!("parts.{target_part_index}.l1.behaviorConfig.{field}");
        return binding_spec_from_leaf(item, rewritten);
    }
    if let Some(field) = key.strip_prefix(&format!("parts.{active_part_index}.l1.behaviorConfig."))
    {
        let rewritten = format!("parts.{target_part_index}.l1.behaviorConfig.{field}");
        return binding_spec_from_leaf(item, rewritten);
    }
    binding_spec_from_item(item)
}
