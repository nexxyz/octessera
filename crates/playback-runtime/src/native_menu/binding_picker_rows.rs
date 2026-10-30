use super::binding_picker::{
    is_lfo_target_picker, keep_numeric_binding_item, play_fx_binding_group, PLAY_FX_BINDING_GROUP,
    SYSTEM_BINDING_GROUP,
};
use super::binding_picker_voice::instrument_binding_source;
use super::binding_tree::binding_tree_from_menu_item;
use super::play::play_fx_page_items;
use super::{NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuValue};

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

fn picker_rows(item: NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
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
    update_parameter_pickers(root, &mut |target, children| {
        let Some(instruments) = children
            .iter_mut()
            .find(|child| child.key.as_deref() == Some("binding.group.shape"))
            .and_then(|shape| {
                shape
                    .children
                    .iter_mut()
                    .find(|child| child.key.as_deref() == Some("binding.group.instruments"))
            })
        else {
            return;
        };
        let Some(at) = instruments
            .children
            .iter()
            .position(|child| child.label == *label)
        else {
            return;
        };
        match binding_tree_from_menu_item(&source, target)
            .and_then(|tree| picker_rows(tree, target))
        {
            Some(tree) => instruments.children[at] = tree,
            None => {
                instruments.children.remove(at);
            }
        }
    });
}
