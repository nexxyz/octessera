use super::binding_picker::{
    axis_binding_label, current_binding_rows, is_lfo_target_picker, keep_numeric_binding_item,
    labelled_binding, mark_active_binding, play_fx_binding_group, INSTRUMENTS_BINDING_GROUP,
    PLAY_FX_BINDING_GROUP, SYSTEM_BINDING_GROUP,
};
use super::binding_picker_voice::instrument_binding_source;
use super::binding_tree::binding_tree_from_menu_item;
use super::fx::{FX_BUSES_LABEL, GLOBAL_FX_LABEL};
use super::model_search::{find_item_by_action, find_item_by_key};
use super::play::play_fx_page_items;
use super::{
    keyed_group, NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuValue,
    NativeParamBindingSpec,
};

/// The target of a parameter picker, whose first row clears its binding.
pub(super) fn picker_target(item: &NativeMenuItem) -> Option<&str> {
    match item.children.first().map(|first| &first.value) {
        Some(NativeMenuValue::Action(NativeMenuAction::ClearParamBinding { target })) => {
            Some(target)
        }
        _ => None,
    }
}

/// Shows a parameter picker's binding: its label and its Current and Range rows.
pub(super) fn show_picker_binding(
    picker: &mut NativeMenuItem,
    target: &str,
    binding: Option<&NativeParamBindingSpec>,
) {
    if let Some((prefix, _)) = picker.label.split_once(": ") {
        picker.label = axis_binding_label(prefix, binding);
    }
    let rows = binding
        .map(|binding| current_binding_rows(target, binding))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| picker_rows(row, target))
        .collect::<Vec<_>>();
    let row_keys = ["current", "rangeMin", "rangeMax"].map(|row| format!("{target}.{row}"));
    picker
        .children
        .retain(|item| !item.key.as_ref().is_some_and(|key| row_keys.contains(key)));
    let at = 1.min(picker.children.len());
    picker.children.splice(at..at, rows);
    mark_active_binding(
        &mut picker.children,
        binding.map(|binding| binding.key.as_str()),
    );
}

/// Names the pickers whose binding kept only its key from their entries in the shared `tree`.
pub(super) fn name_parameter_pickers(node: &mut NativeMenuItem, tree: &[NativeMenuItem]) {
    for child in &mut node.children {
        let Some(target) = picker_target(child).map(str::to_owned) else {
            name_parameter_pickers(child, tree);
            continue;
        };
        let unnamed = child.children.iter().find_map(|row| match &row.value {
            NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. })
                if binding.label.is_none() =>
            {
                Some(labelled_binding(binding, tree))
            }
            _ => None,
        });
        if let Some(binding) = unnamed.filter(|binding| binding.label.is_some()) {
            show_picker_binding(child, &target, Some(&binding));
        }
    }
}

/// The top groups of a picker's list; its own `(none)`, Current and Range rows are no groups.
fn is_list_group(item: &NativeMenuItem) -> bool {
    item.value == NativeMenuValue::Group
}

/// Copies the shared parameter `tree` into the picker, with the picker's target in each entry.
pub(super) fn open_picker(picker: &mut NativeMenuItem, tree: &[NativeMenuItem]) {
    let Some(target) = picker_target(picker).map(str::to_owned) else {
        return;
    };
    if picker.children.iter().any(is_list_group) {
        return;
    }
    let bound = bound_key(picker).map(str::to_owned);
    let head = picker.children.len();
    picker.children.extend(tree.iter().filter_map(|group| {
        let mut group = group.clone();
        retarget(&mut group, &target);
        picker_rows(group, &target)
    }));
    mark_active_binding(&mut picker.children[head..], bound.as_deref());
}

/// The entry of a closed picker's list that binds `binding_key`, retargeted and marked as
/// `open_picker` shows it, without copying the rest of the list.
pub(super) fn picker_list_entry(
    picker: &NativeMenuItem,
    tree: &[NativeMenuItem],
    binding_key: &str,
) -> Option<NativeMenuItem> {
    let target = picker_target(picker)?;
    let mut entry = tree.iter().find_map(|group| {
        let mut entry = find_binding_entry(group, binding_key)?.clone();
        retarget(&mut entry, target);
        picker_rows(entry, target)
    })?;
    mark_active_binding(std::slice::from_mut(&mut entry), bound_key(picker));
    Some(entry)
}

fn bound_key(picker: &NativeMenuItem) -> Option<&str> {
    picker.children.iter().find_map(|row| match &row.value {
        NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) => {
            Some(binding.key.as_str())
        }
        _ => None,
    })
}

fn find_binding_entry<'a>(node: &'a NativeMenuItem, key: &str) -> Option<&'a NativeMenuItem> {
    match &node.value {
        NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. })
            if binding.key == key =>
        {
            Some(node)
        }
        _ => node
            .children
            .iter()
            .find_map(|child| find_binding_entry(child, key)),
    }
}

pub(super) fn close_picker(picker: &mut NativeMenuItem) {
    picker.children.retain(|row| !is_list_group(row));
}

fn retarget(item: &mut NativeMenuItem, target: &str) {
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding {
        target: row_target,
        binding,
    }) = &mut item.value
    {
        target.clone_into(row_target);
        item.key = Some(format!("{target}.{}", binding.key));
    }
    for child in &mut item.children {
        retarget(child, target);
    }
}

/// The entry of `target`'s picker list that runs `action`, found in the shared `tree`.
pub(super) fn picker_entry_for_action<'a>(
    root: &NativeMenuItem,
    tree: &'a [NativeMenuItem],
    action: &NativeMenuAction,
) -> Option<&'a NativeMenuItem> {
    let NativeMenuAction::SetParamBinding { target, binding } = action else {
        return None;
    };
    find_item_by_key(root, target).filter(|picker| picker_target(picker) == Some(target))?;
    let untargeted = NativeMenuAction::SetParamBinding {
        target: String::new(),
        binding: binding.clone(),
    };
    tree.iter()
        .find_map(|group| find_item_by_action(group, &untargeted))
        .filter(|entry| picker_rows((*entry).clone(), target).is_some())
}

pub(super) fn picker_rows(item: NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
    if is_lfo_target_picker(target) {
        keep_numeric_binding_item(item, target)
    } else {
        Some(item)
    }
}

pub(super) fn update_play_fx_bindings(tree: &mut Vec<NativeMenuItem>, config: &NativeMenuConfig) {
    tree.retain(|child| child.key.as_deref() != Some(PLAY_FX_BINDING_GROUP));
    let Some(group) = play_fx_binding_group(&play_fx_page_items(config), "") else {
        return;
    };
    let at = tree
        .iter()
        .position(|child| child.key.as_deref() == Some(SYSTEM_BINDING_GROUP))
        .unwrap_or(tree.len());
    tree.insert(at, group);
}

pub(super) fn update_instrument_bindings(
    tree: &mut [NativeMenuItem],
    config: &NativeMenuConfig,
    index: usize,
) {
    let Some(label) = config.instrument_labels.get(index) else {
        return;
    };
    let source = instrument_binding_source(config, index, label);
    let rank = |label: &str| {
        label
            .split_once(": ")
            .and_then(|(slot, _)| slot.strip_prefix('I')?.parse::<usize>().ok())
    };
    let is_instruments =
        |child: &NativeMenuItem| child.key.as_deref() == Some(INSTRUMENTS_BINDING_GROUP);
    let Some(shape) = shape_binding_group(tree) else {
        return;
    };
    if !shape.children.iter().any(is_instruments) {
        shape.children.insert(
            0,
            keyed_group("Instruments", INSTRUMENTS_BINDING_GROUP, Vec::new()),
        );
    }
    if let Some(instruments) = shape
        .children
        .iter_mut()
        .find(|child| is_instruments(child))
    {
        replace_binding_tree(&mut instruments.children, &source, &rank);
    }
    shape
        .children
        .retain(|child| !is_instruments(child) || !child.children.is_empty());
}

/// `source` is a Shape group such as FX Buses or Global FX, already updated in the menu.
pub(super) fn update_shape_bindings(tree: &mut [NativeMenuItem], source: &NativeMenuItem) {
    let rank = |label: &str| {
        ["Instruments", FX_BUSES_LABEL, GLOBAL_FX_LABEL]
            .iter()
            .position(|known| *known == label)
    };
    if let Some(shape) = shape_binding_group(tree) {
        replace_binding_tree(&mut shape.children, source, &rank);
    }
}

fn shape_binding_group(children: &mut [NativeMenuItem]) -> Option<&mut NativeMenuItem> {
    children
        .iter_mut()
        .find(|child| child.key.as_deref() == Some("binding.group.shape"))
}

/// Picker groups keep their sources' order, so `rank` places the new tree, and the one it
/// replaces may have had another label.
fn replace_binding_tree(
    children: &mut Vec<NativeMenuItem>,
    source: &NativeMenuItem,
    rank: &dyn Fn(&str) -> Option<usize>,
) {
    let own = rank(&source.label);
    children.retain(|child| rank(&child.label) != own);
    if let Some(tree) = binding_tree_from_menu_item(source, "") {
        let at = children
            .iter()
            .take_while(|child| rank(&child.label) < own)
            .count();
        children.insert(at, tree);
    }
}
