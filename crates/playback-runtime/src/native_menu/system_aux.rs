use super::binding_picker::parameter_picker_group;
use super::model_search::find_item_by_action;
use super::{
    action_item, group, mark_active, NativeMenuAction, NativeMenuConfig, NativeMenuItem,
    NativeMenuPick, NativeMenuValue,
};

pub(super) fn aux_mappings_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Aux Mappings",
        (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| {
                let binding = config.aux_bindings.get(index).cloned().unwrap_or_default();
                let shift_binding = config
                    .shift_aux_bindings
                    .get(index)
                    .cloned()
                    .unwrap_or_default();
                group(
                    format!("Aux {}", index + 1),
                    vec![
                        parameter_picker_group(
                            "Trn",
                            format!("aux:{index}:turn"),
                            binding.turn.as_ref(),
                            config,
                        ),
                        aux_click_picker_group(index, binding.click.as_ref(), config),
                        parameter_picker_group(
                            "S+Trn",
                            format!("shiftAux:{index}:turn"),
                            shift_binding.turn.as_ref(),
                            config,
                        ),
                        shift_aux_click_picker_group(index, shift_binding.click.as_ref(), config),
                    ],
                )
            })
            .collect(),
    )
}

fn aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    aux_click_picker_group_with(index, current, config, "Clk", false)
}

fn shift_aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    aux_click_picker_group_with(index, current, config, "S+Clk", true)
}

fn aux_click_picker_group_with(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
    label_prefix: &str,
    shifted: bool,
) -> NativeMenuItem {
    let key_prefix = click_key_prefix(shifted);
    let mut children = vec![action_item(
        "(none)",
        format!("{key_prefix}{}.click.none", index + 1),
        aux_click_action(index, None, shifted),
    )];
    let behavior_actions = config
        .build_items
        .iter()
        .filter_map(|item| match &item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(action)) => Some(action_item(
                item.label.clone(),
                format!("{key_prefix}{}.click.behavior.{action}", index + 1),
                aux_click_action(
                    index,
                    Some(Box::new(NativeMenuAction::BehaviorAction(action.clone()))),
                    shifted,
                ),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !behavior_actions.is_empty() {
        children.push(group("Behavior", behavior_actions));
    }
    children.push(group(
        "Sample Assign",
        config
            .instrument_labels
            .iter()
            .enumerate()
            .map(|(instrument, label)| {
                action_item(
                    label.clone(),
                    format!("{key_prefix}{}.click.sample.{instrument}", index + 1),
                    aux_click_action(
                        index,
                        Some(Box::new(NativeMenuAction::PlatformEffect(format!(
                            "sample.assign:{instrument}:0"
                        )))),
                        shifted,
                    ),
                )
            })
            .collect(),
    ));
    children.push(group(
        "Actions",
        vec![
            action_item(
                "Map FX",
                format!("{key_prefix}{}.click.fx_map", index + 1),
                aux_click_action(
                    index,
                    Some(Box::new(NativeMenuAction::PlatformEffect(
                        "play.fx.map".into(),
                    ))),
                    shifted,
                ),
            ),
            action_item(
                "Reset Behavior",
                format!("{key_prefix}{}.click.reset", index + 1),
                aux_click_action(
                    index,
                    Some(Box::new(NativeMenuAction::ResetBehavior)),
                    shifted,
                ),
            ),
        ],
    ));
    let mut picker = NativeMenuItem {
        pick: NativeMenuPick::Value,
        ..group(label_prefix, children)
    };
    show_click_binding(&mut picker, index, shifted, current);
    picker
}

/// The key of a Clk picker's `(none)` row, which finds the picker.
pub(super) fn click_none_key(index: usize, shifted: bool) -> String {
    format!("{}{}.click.none", click_key_prefix(shifted), index + 1)
}

fn click_key_prefix(shifted: bool) -> &'static str {
    if shifted {
        "shift_aux"
    } else {
        "aux"
    }
}

/// Shows a Clk picker's binding: its Current row, its pointer and the action's name in its
/// label; `mapped` until the whole menu names an action its list does not hold.
pub(super) fn show_click_binding(
    picker: &mut NativeMenuItem,
    index: usize,
    shifted: bool,
    current: Option<&NativeMenuAction>,
) {
    let current_key = format!("{}{}.click.current", click_key_prefix(shifted), index + 1);
    picker
        .children
        .retain(|item| item.key.as_deref() != Some(current_key.as_str()));
    if let Some(action) = current {
        let at = 1.min(picker.children.len());
        picker.children.insert(
            at,
            action_item(
                "Current",
                current_key.clone(),
                aux_click_action(index, Some(Box::new(action.clone())), shifted),
            ),
        );
    }
    let picked = aux_click_action(index, current.cloned().map(Box::new), shifted);
    mark_active(&mut picker.children, &|item| {
        matches!(&item.value, NativeMenuValue::Action(action) if *action == picked)
            && item.key.as_deref() != Some(current_key.as_str())
    });
    let name = match current {
        Some(_) => entry_label_for_action(&picker.children, &picked, &current_key)
            .unwrap_or_else(|| "mapped".into()),
        None => "(none)".into(),
    };
    let prefix = if shifted { "S+Clk" } else { "Clk" };
    picker.label = format!("{prefix}: {name}");
}

fn entry_label_for_action(
    items: &[NativeMenuItem],
    action: &NativeMenuAction,
    current_key: &str,
) -> Option<String> {
    items.iter().find_map(|item| match &item.value {
        NativeMenuValue::Action(entry) if entry == action => {
            (item.key.as_deref() != Some(current_key)).then(|| item.label.clone())
        }
        _ => entry_label_for_action(&item.children, action, current_key),
    })
}

/// A Clk picker whose list does not hold its bound action (one bound with Fn + click on another
/// menu row) takes that row's label from the whole menu.
pub(super) fn name_click_pickers_from_menu(root: &mut NativeMenuItem) {
    let mut unnamed = Vec::new();
    collect_unnamed_click_pickers(root, &mut Vec::new(), &mut unnamed);
    for (path, action) in unnamed {
        let Some(name) = find_item_by_action(root, &action).map(|item| item.label.clone()) else {
            continue;
        };
        let picker = path
            .iter()
            .fold(&mut *root, |node, index| &mut node.children[*index]);
        if let Some((prefix, _)) = picker.label.split_once(": ") {
            picker.label = format!("{prefix}: {name}");
        }
    }
}

fn collect_unnamed_click_pickers(
    node: &NativeMenuItem,
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, NativeMenuAction)>,
) {
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        if let Some(action) = unnamed_click_action(child) {
            out.push((path.clone(), action));
        } else if child.pick != NativeMenuPick::Value {
            collect_unnamed_click_pickers(child, path, out);
        }
        path.pop();
    }
}

/// The bound action of a Clk picker that points at none of its entries.
fn unnamed_click_action(picker: &NativeMenuItem) -> Option<NativeMenuAction> {
    if picker.pick != NativeMenuPick::Value
        || picker
            .children
            .iter()
            .any(|child| child.pick == NativeMenuPick::Active)
    {
        return None;
    }
    picker.children.iter().find_map(|child| match &child.value {
        NativeMenuValue::Action(
            NativeMenuAction::SetAuxClick {
                action: Some(action),
                ..
            }
            | NativeMenuAction::SetShiftAuxClick {
                action: Some(action),
                ..
            },
        ) => Some((**action).clone()),
        _ => None,
    })
}

fn aux_click_action(
    index: usize,
    action: Option<Box<NativeMenuAction>>,
    shifted: bool,
) -> NativeMenuAction {
    if shifted {
        NativeMenuAction::SetShiftAuxClick { index, action }
    } else {
        NativeMenuAction::SetAuxClick { index, action }
    }
}
