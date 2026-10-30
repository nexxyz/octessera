use super::binding_picker::{
    is_lfo_target_picker, keep_numeric_binding_item, play_fx_binding_group,
    INSTRUMENTS_BINDING_GROUP, PLAY_FX_BINDING_GROUP, SYSTEM_BINDING_GROUP,
};
use super::binding_picker_voice::instrument_binding_source;
use super::binding_tree::binding_tree_from_menu_item;
use super::fx::{FX_BUSES_LABEL, GLOBAL_FX_LABEL};
use super::play::play_fx_page_items;
use super::{keyed_group, NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuValue};

/// Applies `update` to the children of every parameter picker below `node`; the pickers repeat
/// the bindable menu, so rows that change there change in each picker too.
fn update_parameter_pickers(
    node: &mut NativeMenuItem,
    update: &mut dyn FnMut(&str, &mut Vec<NativeMenuItem>),
) {
    for child in &mut node.children {
        let target = match child.children.first().map(|first| &first.value) {
            Some(NativeMenuValue::Action(NativeMenuAction::ClearParamBinding { target })) => {
                Some(target.clone())
            }
            _ => None,
        };
        match target {
            Some(target) => update(&target, &mut child.children),
            None => update_parameter_pickers(child, update),
        }
    }
}

pub(super) fn picker_rows(item: NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
    if is_lfo_target_picker(target) {
        keep_numeric_binding_item(item, target)
    } else {
        Some(item)
    }
}

pub(super) fn update_play_fx_bindings(root: &mut NativeMenuItem, config: &NativeMenuConfig) {
    let items = play_fx_page_items(config);
    update_parameter_pickers(root, &mut |target, children| {
        children.retain(|child| child.key.as_deref() != Some(PLAY_FX_BINDING_GROUP));
        let Some(group) =
            play_fx_binding_group(&items, target).and_then(|group| picker_rows(group, target))
        else {
            return;
        };
        let at = children
            .iter()
            .position(|child| child.key.as_deref() == Some(SYSTEM_BINDING_GROUP))
            .unwrap_or(children.len());
        children.insert(at, group);
    });
}

pub(super) fn update_instrument_bindings(
    root: &mut NativeMenuItem,
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
    update_parameter_pickers(root, &mut |target, children| {
        let Some(shape) = shape_binding_group(children) else {
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
            replace_binding_tree(&mut instruments.children, &source, target, &rank);
        }
        shape
            .children
            .retain(|child| !is_instruments(child) || !child.children.is_empty());
    });
}

/// `source` is a Shape group such as FX Buses or Global FX, already updated in the menu.
pub(super) fn update_shape_bindings(root: &mut NativeMenuItem, source: &NativeMenuItem) {
    let rank = |label: &str| {
        ["Instruments", FX_BUSES_LABEL, GLOBAL_FX_LABEL]
            .iter()
            .position(|known| *known == label)
    };
    update_parameter_pickers(root, &mut |target, children| {
        if let Some(shape) = shape_binding_group(children) {
            replace_binding_tree(&mut shape.children, source, target, &rank);
        }
    });
}

fn shape_binding_group(children: &mut [NativeMenuItem]) -> Option<&mut NativeMenuItem> {
    child_with_key(children, "binding.group.shape")
}

fn child_with_key<'a>(
    children: &'a mut [NativeMenuItem],
    key: &str,
) -> Option<&'a mut NativeMenuItem> {
    children
        .iter_mut()
        .find(|child| child.key.as_deref() == Some(key))
}

/// Picker groups keep their sources' order, so `rank` places the new tree, and the one it
/// replaces may have had another label.
fn replace_binding_tree(
    children: &mut Vec<NativeMenuItem>,
    source: &NativeMenuItem,
    target: &str,
    rank: &dyn Fn(&str) -> Option<usize>,
) {
    let own = rank(&source.label);
    children.retain(|child| rank(&child.label) != own);
    if let Some(tree) =
        binding_tree_from_menu_item(source, target).and_then(|tree| picker_rows(tree, target))
    {
        let at = children
            .iter()
            .take_while(|child| rank(&child.label) < own)
            .count();
        children.insert(at, tree);
    }
}
