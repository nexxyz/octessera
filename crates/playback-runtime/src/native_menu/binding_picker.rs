use platform_core::BUS_COUNT as FX_BUS_COUNT;

use super::binding_behavior::behavior_binding_groups;
use super::binding_picker_voice::instrument_binding_groups;
use super::binding_pulses::pulses_binding_group;
use super::binding_tree::{binding_action, binding_group_from_items, binding_tree_from_menu_item};
use super::fx::{fx_buses_group, global_fx_group};
use super::section_labels::{BUILD_LABEL, LINK_LABEL, PLAY_LABEL, SHAPE_LABEL};
use super::sparks::sparks_fx_page_items;
use super::{action_item, group, number_item, NativeMenuAction, NativeMenuConfig, NativeMenuItem};
use super::{NativeMenuValue, NativeParamBindingSpec};

pub(super) fn sparks_fx_targets() -> Vec<String> {
    let mut targets = vec!["master".to_string()];
    targets.extend((1..=FX_BUS_COUNT).map(|index| format!("fx_bus_{index}")));
    targets.extend((1..=8).map(|index| format!("instrument_{index}")));
    targets
}

pub(super) fn axis_binding_label(label: &str, binding: Option<&NativeParamBindingSpec>) -> String {
    binding
        .and_then(|binding| binding.label.as_deref().or(Some(binding.key.as_str())))
        .map(|binding_label| format!("{label}: {binding_label}"))
        .unwrap_or_else(|| format!("{label}: (none)"))
}

pub(super) fn parameter_picker_group(
    label: String,
    target: String,
    current: Option<&NativeParamBindingSpec>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("{target}.none"),
        NativeMenuAction::ClearParamBinding {
            target: target.clone(),
        },
    )];
    children.extend(parameter_tree_groups(&target, config));
    if let Some(binding) = current {
        if binding.kind == "number"
            && !target.starts_with("aux:")
            && !target.starts_with("shiftAux:")
        {
            children.insert(1, range_item("Range Max", &target, "rangeMax", binding));
            children.insert(1, range_item("Range Min", &target, "rangeMin", binding));
        }
        children.insert(
            1,
            action_item(
                format!(
                    "Current: {}",
                    binding.label.as_deref().unwrap_or(&binding.key)
                ),
                format!("{target}.current"),
                NativeMenuAction::SetParamBinding {
                    target: target.clone(),
                    binding: binding.clone(),
                },
            ),
        );
    }
    NativeMenuItem {
        label,
        key: Some(target),
        value: NativeMenuValue::Group,
        children,
    }
}

pub(super) fn parameter_picker_group_numeric(
    label: String,
    target: String,
    current: Option<&NativeParamBindingSpec>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut item = parameter_picker_group(
        label,
        target.clone(),
        current.filter(|b| b.kind == "number"),
        config,
    );
    item.children = item
        .children
        .into_iter()
        .filter_map(|item| keep_numeric_binding_item(item, &target))
        .collect();
    item
}

fn keep_numeric_binding_item(mut item: NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) = &item.value
    {
        let allowed = binding.kind == "number"
            && !binding.key.contains(".linkLfo.")
            && (!target.ends_with(".linkLfo.target")
                || crate::native_runner::is_live_link_lfo_target_for_picker(&binding.key));
        return allowed.then_some(item);
    }
    item.children = item
        .children
        .into_iter()
        .filter_map(|item| keep_numeric_binding_item(item, target))
        .collect();
    match item.value {
        NativeMenuValue::Group if item.children.is_empty() => None,
        _ => Some(item),
    }
}

fn range_item(
    label: &str,
    target: &str,
    suffix: &str,
    binding: &NativeParamBindingSpec,
) -> NativeMenuItem {
    let min = binding.min.unwrap_or(0);
    let max = binding.max.unwrap_or(127);
    let low = min.min(max);
    let high = min.max(max);
    let value = match suffix {
        "rangeMin" => binding.user_min.unwrap_or(min),
        "rangeMax" => binding.user_max.unwrap_or(max),
        _ => min,
    }
    .clamp(low, high);
    number_item(
        label,
        format!("{target}.{suffix}"),
        value,
        low,
        high,
        binding.step.unwrap_or(1).max(1),
    )
}

pub(super) fn parameter_tree_groups(
    target: &str,
    config: &NativeMenuConfig,
) -> Vec<NativeMenuItem> {
    let mut groups = Vec::new();

    if let Some(behavior_group) = behavior_binding_groups(config, target) {
        groups.push(group(BUILD_LABEL, behavior_group.children));
    }

    let pulses_groups = config
        .layer_labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| pulses_binding_group(index, label, config, target))
        .collect::<Vec<_>>();
    if !pulses_groups.is_empty() {
        groups.push(group(LINK_LABEL, pulses_groups));
    }

    let instrument_groups = instrument_binding_groups(config, target);
    let mut voice_children = Vec::new();
    if !instrument_groups.is_empty() {
        voice_children.push(group("Instruments", instrument_groups));
    }
    if let Some(item) =
        binding_tree_from_menu_item(&fx_buses_group(&config.fx_buses, config.bpm), target)
    {
        voice_children.push(item);
    }
    if let Some(item) = binding_tree_from_menu_item(
        &global_fx_group(
            &config.global_fx_slots,
            &config.global_fx_params,
            config.bpm,
        ),
        target,
    ) {
        voice_children.push(item);
    }
    if !voice_children.is_empty() {
        groups.push(group(SHAPE_LABEL, voice_children));
    }

    if let Some(item) = binding_group_from_items("Play FX", &sparks_fx_page_items(config), target) {
        groups.push(group(PLAY_LABEL, vec![item]));
    }

    groups.push(group(
        "System",
        vec![group("Sound", sound_binding_items(target))],
    ));

    groups
}

fn sound_binding_items(target: &str) -> Vec<NativeMenuItem> {
    vec![
        binding_action(
            "Note Length",
            "sound.noteLengthMs",
            "number",
            Some(30),
            Some(2000),
            Some(10),
            vec![],
            target,
        ),
        binding_action(
            "Velocity Scale",
            "sound.velocityScalePct",
            "number",
            Some(0),
            Some(200),
            Some(1),
            vec![],
            target,
        ),
        binding_action(
            "Voice Limit",
            "sound.voiceStealingMode",
            "enum",
            None,
            None,
            None,
            vec![
                "fixed12",
                "fixed16",
                "auto-soft",
                "auto-balanced",
                "auto-hard",
                "none",
            ],
            target,
        ),
    ]
}
