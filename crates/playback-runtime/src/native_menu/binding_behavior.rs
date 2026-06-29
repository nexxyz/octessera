use super::binding_tree::{binding_action_from_spec, binding_spec_from_item};
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
                config
                    .behavior_target_items
                    .get(part_index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                target,
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
) -> Option<NativeMenuItem> {
    let children = items
        .iter()
        .filter_map(|item| binding_tree_from_behavior_item(item, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(label, children))
    }
}

fn binding_tree_from_behavior_item(item: &NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
    if let Some(binding) = binding_spec_from_behavior_item(item) {
        return Some(binding_action_from_spec(binding, target));
    }
    let children = item
        .children
        .iter()
        .filter_map(|child| binding_tree_from_behavior_item(child, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(item.label.clone(), children))
    }
}

fn binding_spec_from_behavior_item(item: &NativeMenuItem) -> Option<NativeParamBindingSpec> {
    item.key.as_ref()?;
    binding_spec_from_item(item)
}
