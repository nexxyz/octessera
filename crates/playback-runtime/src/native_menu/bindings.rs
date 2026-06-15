use platform_core::BUS_COUNT as FX_BUS_COUNT;

use super::{
    action_item, bool_item, enum_item, group, selected_index, NativeMenuAction, NativeMenuConfig,
    NativeMenuItem, NativeMenuValue, NativeParamBindingSpec,
};

pub(super) fn dance_fx_targets() -> Vec<String> {
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
    group(label, children)
}

pub(super) fn parameter_tree_groups(
    target: &str,
    config: &NativeMenuConfig,
) -> Vec<NativeMenuItem> {
    let mut groups = vec![group(
        "Sound",
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
                "Voice Stealing",
                "sound.voiceStealingMode",
                "enum",
                None,
                None,
                None,
                vec!["off", "lenient", "balanced", "aggressive"],
                target,
            ),
        ],
    )];

    let behavior_params = config
        .l1_items
        .iter()
        .filter_map(|item| {
            binding_from_menu_item(
                item,
                &format!("parts.{}.l1.behaviorConfig", config.active_part_index),
            )
        })
        .map(|binding| binding_action_from_spec(binding, target))
        .collect::<Vec<_>>();
    if !behavior_params.is_empty() {
        groups.push(group("Behavior", behavior_params));
    }

    let instrument_groups = config
        .instrument_labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            group(
                label.clone(),
                vec![
                    group(
                        "Mixer",
                        vec![
                            binding_action(
                                "Volume",
                                &format!("instruments.{index}.mixer.volume"),
                                "number",
                                Some(0),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Pan",
                                &format!("instruments.{index}.mixer.panPos"),
                                "number",
                                Some(0),
                                Some(32),
                                Some(1),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                    group(
                        "Synth",
                        vec![binding_action(
                            "Gain",
                            &format!("instruments.{index}.synth.amp.gainPct"),
                            "number",
                            Some(0),
                            Some(100),
                            Some(1),
                            vec![],
                            target,
                        )],
                    ),
                    group(
                        "Sample",
                        vec![
                            binding_action(
                                "Base Velocity",
                                &format!("instruments.{index}.sample.baseVelocity"),
                                "number",
                                Some(1),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Tune",
                                &format!("instruments.{index}.sample.tuneSemis"),
                                "number",
                                Some(-24),
                                Some(24),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Gain",
                                &format!("instruments.{index}.sample.amp.gainPct"),
                                "number",
                                Some(0),
                                Some(100),
                                Some(1),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                    group(
                        "MIDI",
                        vec![
                            binding_action(
                                "Enabled",
                                &format!("instruments.{index}.midi.enabled"),
                                "bool",
                                None,
                                None,
                                None,
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Velocity",
                                &format!("instruments.{index}.midi.velocity"),
                                "number",
                                Some(1),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Duration",
                                &format!("instruments.{index}.midi.durationMs"),
                                "number",
                                Some(10),
                                Some(5000),
                                Some(10),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                ],
            )
        })
        .collect::<Vec<_>>();
    groups.push(group("Instruments", instrument_groups));
    groups
}

pub(super) fn xy_pad_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        parameter_picker_group(
            axis_binding_label("X Axis", config.xy_x_binding.as_ref()),
            "xy:x".into(),
            config.xy_x_binding.as_ref(),
            config,
        ),
        parameter_picker_group(
            axis_binding_label("Y Axis", config.xy_y_binding.as_ref()),
            "xy:y".into(),
            config.xy_y_binding.as_ref(),
            config,
        ),
        bool_item("Invert X", "dance.xy.invertX", config.xy_invert_x),
        bool_item("Invert Y", "dance.xy.invertY", config.xy_invert_y),
        enum_item(
            "Release",
            "dance.xy.release",
            vec!["sample-hold", "reset-center"],
            selected_index(&["sample-hold", "reset-center"], &config.xy_release),
        ),
    ]
}

fn binding_from_menu_item(
    item: &NativeMenuItem,
    behavior_prefix: &str,
) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?.strip_prefix("behavior.")?;
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn binding_action(
    label: &str,
    key: &str,
    kind: &str,
    min: Option<i32>,
    max: Option<i32>,
    step: Option<i32>,
    options: Vec<&str>,
    target: &str,
) -> NativeMenuItem {
    binding_action_from_spec(
        NativeParamBindingSpec {
            key: key.into(),
            label: Some(label.into()),
            kind: kind.into(),
            min,
            max,
            step,
            options: options.into_iter().map(str::to_string).collect(),
            invert: false,
        },
        target,
    )
}

fn binding_action_from_spec(binding: NativeParamBindingSpec, target: &str) -> NativeMenuItem {
    action_item(
        binding.label.clone().unwrap_or_else(|| binding.key.clone()),
        format!("{target}.{}", binding.key),
        NativeMenuAction::SetParamBinding {
            target: target.into(),
            binding,
        },
    )
}
