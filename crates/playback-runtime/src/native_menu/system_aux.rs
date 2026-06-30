use super::bindings::{axis_binding_label, parameter_picker_group};
use super::{
    action_item, bool_item, group, NativeMenuAction, NativeMenuConfig, NativeMenuItem,
    NativeMenuValue,
};

pub(super) fn aux_mappings_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Aux Mappings",
        std::iter::once(bool_item(
            "Auto Map",
            "auxAutoMapEnabled",
            config.aux_auto_map_enabled,
        ))
        .chain((0..platform_core::AUX_ENCODER_COUNT).map(|index| {
            let binding = config.aux_bindings.get(index).cloned().unwrap_or_default();
            group(
                format!("Aux {}", index + 1),
                vec![
                    parameter_picker_group(
                        axis_binding_label("Turn", binding.turn.as_ref()),
                        format!("aux:{index}:turn"),
                        binding.turn.as_ref(),
                        config,
                    ),
                    aux_click_picker_group(index, binding.click.as_ref(), config),
                ],
            )
        }))
        .collect(),
    )
}

fn aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("aux{}.click.none", index + 1),
        NativeMenuAction::SetAuxClick {
            index,
            action: None,
        },
    )];
    if let Some(action) = current {
        children.push(action_item(
            "Current",
            format!("aux{}.click.current", index + 1),
            NativeMenuAction::SetAuxClick {
                index,
                action: Some(Box::new(action.clone())),
            },
        ));
    }
    let behavior_actions = config
        .l1_items
        .iter()
        .filter_map(|item| match &item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(action)) => Some(action_item(
                item.label.clone(),
                format!("aux{}.click.behavior.{action}", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::BehaviorAction(action.clone()))),
                },
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
                    format!("aux{}.click.sample.{instrument}", index + 1),
                    NativeMenuAction::SetAuxClick {
                        index,
                        action: Some(Box::new(NativeMenuAction::PlatformEffect(format!(
                            "sample.assign:{instrument}:0"
                        )))),
                    },
                )
            })
            .collect(),
    ));
    children.push(group(
        "Actions",
        vec![
            action_item(
                "Map FX",
                format!("aux{}.click.fx_map", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::PlatformEffect(
                        "dance.fx.map".into(),
                    ))),
                },
            ),
            action_item(
                "Reset Behavior",
                format!("aux{}.click.reset", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::ResetBehavior)),
                },
            ),
        ],
    ));
    let label = current
        .map(|_| "Click: mapped".to_string())
        .unwrap_or_else(|| "Click: (none)".into());
    group(label, children)
}
