use super::binding_picker::{axis_binding_label, parameter_picker_group};
use super::{
    action_item, group, NativeMenuAction, NativeMenuConfig, NativeMenuItem, NativeMenuValue,
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
                            axis_binding_label("Trn", binding.turn.as_ref()),
                            format!("aux:{index}:turn"),
                            binding.turn.as_ref(),
                            config,
                        ),
                        aux_click_picker_group(index, binding.click.as_ref(), config),
                        parameter_picker_group(
                            axis_binding_label("S+Trn", shift_binding.turn.as_ref()),
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
    aux_click_picker_group_with(index, current, config, "Clk", "aux", false)
}

fn shift_aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    aux_click_picker_group_with(index, current, config, "S+Clk", "shift_aux", true)
}

fn aux_click_picker_group_with(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
    label_prefix: &str,
    key_prefix: &str,
    shifted: bool,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("{key_prefix}{}.click.none", index + 1),
        aux_click_action(index, None, shifted),
    )];
    if let Some(action) = current {
        children.push(action_item(
            "Current",
            format!("{key_prefix}{}.click.current", index + 1),
            aux_click_action(index, Some(Box::new(action.clone())), shifted),
        ));
    }
    let behavior_actions = config
        .worlds_items
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
                        "sparks.fx.map".into(),
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
    let label = current
        .map(|_| format!("{label_prefix}: mapped"))
        .unwrap_or_else(|| format!("{label_prefix}: (none)"));
    group(label, children)
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
