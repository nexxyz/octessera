use super::binding_picker::parameter_picker_group;
use super::binding_picker_rows::picker_entry_for_action;
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
                        ),
                        aux_click_picker_group(index, binding.click.as_ref(), config),
                        parameter_picker_group(
                            "S+Trn",
                            format!("shiftAux:{index}:turn"),
                            shift_binding.turn.as_ref(),
                        ),
                        shift_aux_click_picker_group(index, shift_binding.click.as_ref(), config),
                    ],
                )
            })
            .collect(),
    )
}

/// What a click picker binds: one side of an Aux encoder's click, or one side of the Play
/// Faders Tile Map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClickPickerSlot {
    Aux { index: usize, shifted: bool },
    FaderTile { shifted: bool },
}

impl ClickPickerSlot {
    /// The prefix of every row key in the picker.
    fn key_root(self) -> String {
        match self {
            Self::Aux { index, shifted } => {
                format!("{}{}.click", click_key_prefix(shifted), index + 1)
            }
            Self::FaderTile { shifted: false } => "play.faders.tile.action".into(),
            Self::FaderTile { shifted: true } => "play.faders.tile.shiftAction".into(),
        }
    }

    fn label_prefix(self) -> &'static str {
        match self {
            Self::Aux { shifted: false, .. } => "Clk",
            Self::Aux { shifted: true, .. } => "S+Clk",
            Self::FaderTile { shifted: false } => "Action",
            Self::FaderTile { shifted: true } => "S+Action",
        }
    }

    fn action(self, action: Option<Box<NativeMenuAction>>) -> NativeMenuAction {
        match self {
            Self::Aux { index, shifted } => aux_click_action(index, action, shifted),
            Self::FaderTile { shifted } => NativeMenuAction::SetFaderTileAction { shifted, action },
        }
    }
}

fn aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let slot = ClickPickerSlot::Aux {
        index,
        shifted: false,
    };
    click_picker_group(slot, current, config)
}

fn shift_aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let slot = ClickPickerSlot::Aux {
        index,
        shifted: true,
    };
    click_picker_group(slot, current, config)
}

/// The actions an Aux click can run, as a picker that binds them to `slot`.
pub(super) fn click_picker_group(
    slot: ClickPickerSlot,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let root = slot.key_root();
    let mut children = vec![action_item(
        "(none)",
        format!("{root}.none"),
        slot.action(None),
    )];
    let behavior_actions = config
        .build_items
        .iter()
        .filter_map(|item| match &item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(action)) => Some(action_item(
                item.label.clone(),
                format!("{root}.behavior.{action}"),
                slot.action(Some(Box::new(NativeMenuAction::BehaviorAction(
                    action.clone(),
                )))),
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
                    format!("{root}.sample.{instrument}"),
                    slot.action(Some(Box::new(NativeMenuAction::PlatformEffect(format!(
                        "sample.assign:{instrument}:0"
                    ))))),
                )
            })
            .collect(),
    ));
    children.push(group(
        "Actions",
        vec![
            action_item(
                "Map FX",
                format!("{root}.fx_map"),
                slot.action(Some(Box::new(NativeMenuAction::PlatformEffect(
                    "play.fx.map".into(),
                )))),
            ),
            action_item(
                "Reset Behavior",
                format!("{root}.reset"),
                slot.action(Some(Box::new(NativeMenuAction::ResetBehavior))),
            ),
        ],
    ));
    let mut picker = NativeMenuItem {
        pick: NativeMenuPick::Value,
        ..group(slot.label_prefix(), children)
    };
    show_click_binding(&mut picker, slot, current);
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
    slot: ClickPickerSlot,
    current: Option<&NativeMenuAction>,
) {
    let current_key = format!("{}.current", slot.key_root());
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
                slot.action(Some(Box::new(action.clone()))),
            ),
        );
    }
    let picked = slot.action(current.cloned().map(Box::new));
    mark_active(&mut picker.children, &|item| {
        matches!(&item.value, NativeMenuValue::Action(action) if *action == picked)
            && item.key.as_deref() != Some(current_key.as_str())
    });
    let name = match current {
        Some(_) => entry_label_for_action(&picker.children, &picked, &current_key)
            .unwrap_or_else(|| "mapped".into()),
        None => "(none)".into(),
    };
    picker.label = format!("{}: {name}", slot.label_prefix());
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
pub(super) fn name_click_pickers_from_menu(root: &mut NativeMenuItem, tree: &[NativeMenuItem]) {
    let mut unnamed = Vec::new();
    collect_unnamed_click_pickers(root, &mut Vec::new(), &mut unnamed);
    for (path, action) in unnamed {
        let Some(name) = find_item_by_action(root, &action)
            .or_else(|| picker_entry_for_action(root, tree, &action))
            .map(|item| item.label.clone())
        else {
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
