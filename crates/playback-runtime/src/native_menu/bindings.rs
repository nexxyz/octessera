use platform_core::BUS_COUNT as FX_BUS_COUNT;

use super::dance::dance_fx_page_items;
use super::fx::{fx_buses_group, global_fx_group};
use super::voice::{instrument_group, InstrumentMenuConfig};
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
    NativeMenuItem {
        label,
        key: Some(target),
        value: NativeMenuValue::Group,
        children,
    }
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

    if let Some(behavior_group) = behavior_binding_groups(config, target) {
        groups.push(behavior_group);
    }

    let sense_groups = config
        .part_labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| sense_binding_group(index, label, config, target))
        .collect::<Vec<_>>();
    if !sense_groups.is_empty() {
        groups.push(group("Sense", sense_groups));
    }

    let instrument_groups = config
        .instrument_labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let item = instrument_group(InstrumentMenuConfig {
                index,
                label: label.clone(),
                name: config
                    .instrument_names
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or(label),
                kind: config
                    .instrument_types
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("none"),
                auto_name: config
                    .instrument_auto_names
                    .get(index)
                    .copied()
                    .unwrap_or(true),
                note_behavior: config
                    .instrument_note_behaviors
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("oneshot"),
                route: config
                    .instrument_routes
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("direct"),
                volume: config.instrument_volumes.get(index).copied().unwrap_or(100),
                pan_pos: config
                    .instrument_pan_positions
                    .get(index)
                    .copied()
                    .unwrap_or(16),
                sample_slot: config
                    .instrument_sample_slots
                    .get(index)
                    .copied()
                    .unwrap_or(0),
                synth_config: config.instrument_synth_configs.get(index),
                synth_osc1_waveform: config
                    .instrument_synth_osc1_waveforms
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("saw"),
                synth_osc2_waveform: config
                    .instrument_synth_osc2_waveforms
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("square"),
                synth_filter_type: config
                    .instrument_synth_filter_types
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("lowpass"),
                synth_filter_cutoff: config
                    .instrument_synth_filter_cutoffs
                    .get(index)
                    .copied()
                    .unwrap_or(8000),
                synth_gain_pct: config
                    .instrument_synth_gain_pct
                    .get(index)
                    .copied()
                    .unwrap_or(70),
                synth_filter_resonance: config
                    .instrument_synth_filter_resonance
                    .get(index)
                    .copied()
                    .unwrap_or(20),
                sample_tune_semis: config
                    .instrument_sample_tune_semis
                    .get(index)
                    .copied()
                    .unwrap_or(0),
                sample_gain_pct: config
                    .instrument_sample_gain_pct
                    .get(index)
                    .copied()
                    .unwrap_or(100),
                sample_base_velocity: config
                    .instrument_sample_base_velocity
                    .get(index)
                    .copied()
                    .unwrap_or(100),
                sample_amp_velocity_sensitivity_pct: config
                    .instrument_sample_amp_velocity_sensitivity_pct
                    .get(index)
                    .copied()
                    .unwrap_or(100),
                sample_velocity_levels_enabled: config
                    .instrument_sample_velocity_levels_enabled
                    .get(index)
                    .copied()
                    .unwrap_or(false),
                sample_velocity_high: config
                    .instrument_sample_velocity_high
                    .get(index)
                    .copied()
                    .unwrap_or(127),
                sample_velocity_medium: config
                    .instrument_sample_velocity_medium
                    .get(index)
                    .copied()
                    .unwrap_or(96),
                sample_velocity_low: config
                    .instrument_sample_velocity_low
                    .get(index)
                    .copied()
                    .unwrap_or(64),
                sample_amp_env: config.instrument_sample_amp_envs.get(index),
                sample_filter: config.instrument_sample_filters.get(index),
                sample_filter_env: config.instrument_sample_filter_envs.get(index),
                midi_enabled: config
                    .instrument_midi_enabled
                    .get(index)
                    .copied()
                    .unwrap_or(false),
                midi_channel: config
                    .instrument_midi_channels
                    .get(index)
                    .copied()
                    .unwrap_or(1),
                midi_velocity: config
                    .instrument_midi_velocity
                    .get(index)
                    .copied()
                    .unwrap_or(100),
                midi_duration_ms: config
                    .instrument_midi_duration_ms
                    .get(index)
                    .copied()
                    .unwrap_or(250),
                sample_browser: config
                    .sample_browser
                    .as_ref()
                    .filter(|browser| browser.instrument_slot == index),
            });
            binding_tree_from_menu_item(&item, target)
        })
        .collect::<Vec<_>>();
    if !instrument_groups.is_empty() {
        groups.push(group("Instruments", instrument_groups));
    }

    if let Some(item) = binding_tree_from_menu_item(&fx_buses_group(&config.fx_buses), target) {
        groups.push(item);
    }
    if let Some(item) = binding_tree_from_menu_item(
        &global_fx_group(&config.global_fx_slots, &config.global_fx_params),
        target,
    ) {
        groups.push(item);
    }
    if let Some(item) = binding_group_from_items("Dance FX", &dance_fx_page_items(config), target) {
        groups.push(item);
    }

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

fn binding_group_from_items(
    label: &str,
    items: &[NativeMenuItem],
    target: &str,
) -> Option<NativeMenuItem> {
    let children = items
        .iter()
        .filter_map(|item| binding_tree_from_menu_item(item, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(label, children))
    }
}

fn behavior_binding_groups(config: &NativeMenuConfig, target: &str) -> Option<NativeMenuItem> {
    let children = config
        .part_labels
        .iter()
        .enumerate()
        .filter_map(|(part_index, label)| {
            binding_group_from_behavior_items(
                label,
                &config.l1_items,
                target,
                config.active_part_index,
                part_index,
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
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeMenuItem> {
    let children = items
        .iter()
        .filter_map(|item| {
            binding_tree_from_behavior_item(item, target, active_part_index, target_part_index)
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(label, children))
    }
}

fn binding_tree_from_behavior_item(
    item: &NativeMenuItem,
    target: &str,
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeMenuItem> {
    if let Some(binding) =
        binding_spec_from_behavior_item(item, active_part_index, target_part_index)
    {
        return Some(binding_action_from_spec(binding, target));
    }
    let children = item
        .children
        .iter()
        .filter_map(|child| {
            binding_tree_from_behavior_item(child, target, active_part_index, target_part_index)
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(item.label.clone(), children))
    }
}

fn binding_tree_from_menu_item(item: &NativeMenuItem, target: &str) -> Option<NativeMenuItem> {
    if let Some(binding) = binding_spec_from_item(item) {
        return Some(binding_action_from_spec(binding, target));
    }
    let children = item
        .children
        .iter()
        .filter_map(|child| binding_tree_from_menu_item(child, target))
        .collect::<Vec<_>>();
    if children.is_empty() {
        None
    } else {
        Some(group(item.label.clone(), children))
    }
}

fn binding_spec_from_item(item: &NativeMenuItem) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?.clone();
    if is_excluded_binding_key(&key) {
        return None;
    }
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key,
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

fn binding_spec_from_behavior_item(
    item: &NativeMenuItem,
    active_part_index: usize,
    target_part_index: usize,
) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?;
    if let Some(field) = key.strip_prefix("behavior.") {
        let rewritten = format!("parts.{target_part_index}.l1.behaviorConfig.{field}");
        return binding_spec_from_leaf(item, rewritten);
    }
    if let Some(field) = key.strip_prefix(&format!("parts.{active_part_index}.l1.behaviorConfig."))
    {
        let rewritten = format!("parts.{target_part_index}.l1.behaviorConfig.{field}");
        return binding_spec_from_leaf(item, rewritten);
    }
    binding_spec_from_item(item)
}

fn binding_spec_from_leaf(item: &NativeMenuItem, key: String) -> Option<NativeParamBindingSpec> {
    if is_excluded_binding_key(&key) {
        return None;
    }
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key,
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

fn is_excluded_binding_key(key: &str) -> bool {
    key == "behaviorId"
        || key == "danceMode"
        || key.ends_with(".name")
        || key.ends_with(".autoName")
        || key.ends_with(".clone")
        || key.ends_with(".reset")
        || key.contains(".mapping.")
        || key.ends_with(".triggerProbability.map")
}

fn sense_binding_group(
    index: usize,
    label: &str,
    config: &NativeMenuConfig,
    target: &str,
) -> Option<NativeMenuItem> {
    let sense = config.sense_parts.get(index)?;
    let prefix = format!("parts.{index}.l2");
    Some(group(
        label,
        vec![
            group(
                "Scanning",
                vec![
                    binding_action(
                        "Scan Mode",
                        &format!("{prefix}.scanMode"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["immediate", "scanning"],
                        target,
                    ),
                    binding_action(
                        "Scan Axis",
                        &format!("{prefix}.scanAxis"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["rows", "columns"],
                        target,
                    ),
                    binding_action(
                        "Scan Unit",
                        &format!("{prefix}.scanUnit"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["1/16", "1/8", "1/4", "1/2", "1/1"],
                        target,
                    ),
                    binding_action(
                        "Scan Direction",
                        &format!("{prefix}.scanDirection"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["forward", "reverse"],
                        target,
                    ),
                    binding_action(
                        "Sections",
                        &format!("{prefix}.scanSections"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["1", "2", "4", "8"],
                        target,
                    ),
                ],
            ),
            group(
                "Events",
                vec![
                    binding_action(
                        "Event Triggers",
                        &format!("{prefix}.eventEnabled"),
                        "bool",
                        None,
                        None,
                        None,
                        vec![],
                        target,
                    ),
                    binding_action(
                        "State Notes",
                        &format!("{prefix}.stateNotesEnabled"),
                        "bool",
                        None,
                        None,
                        None,
                        vec![],
                        target,
                    ),
                ],
            ),
            group(
                "Trigger Prob.",
                vec![
                    binding_action(
                        "Mode",
                        &format!("{prefix}.triggerProbabilityMode"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["zero", "custom", "full"],
                        target,
                    ),
                    binding_action(
                        "Low Prob",
                        &format!("{prefix}.triggerProbabilityLowPct"),
                        "number",
                        Some(0),
                        Some(100),
                        Some(1),
                        vec![],
                        target,
                    ),
                    binding_action(
                        "High Prob",
                        &format!("{prefix}.triggerProbabilityHighPct"),
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
                "Note Mapping",
                vec![
                    binding_action(
                        "Lowest Note",
                        &format!("{prefix}.pitch.lowestNote"),
                        "number",
                        Some(0),
                        Some(127),
                        Some(1),
                        vec![],
                        target,
                    ),
                    binding_action(
                        "Highest Note",
                        &format!("{prefix}.pitch.highestNote"),
                        "number",
                        Some(0),
                        Some(127),
                        Some(1),
                        vec![],
                        target,
                    ),
                    binding_action(
                        "Starting Note",
                        &format!("{prefix}.pitch.startingNote"),
                        "number",
                        Some(0),
                        Some(127),
                        Some(1),
                        vec![],
                        target,
                    ),
                    binding_action(
                        "Scale",
                        &format!("{prefix}.pitch.scale"),
                        "enum",
                        None,
                        None,
                        None,
                        vec![
                            "chromatic",
                            "major",
                            "natural_minor",
                            "dorian",
                            "mixolydian",
                            "major_pentatonic",
                            "minor_pentatonic",
                            "harmonic_minor",
                        ],
                        target,
                    ),
                    binding_action(
                        "Root",
                        &format!("{prefix}.pitch.root"),
                        "enum",
                        None,
                        None,
                        None,
                        vec![
                            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                        ],
                        target,
                    ),
                    binding_action(
                        "Out of Range",
                        &format!("{prefix}.pitch.outOfRange"),
                        "enum",
                        None,
                        None,
                        None,
                        vec!["clamp", "wrap"],
                        target,
                    ),
                ],
            ),
            sense_axis_binding_group(
                &format!("{prefix}.x"),
                "X Axis",
                sense.x_pitch_enabled,
                sense.x_pitch_steps,
                sense.x_pitch_restart_each_section,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.x.velocity"),
                "Velocity",
                &sense.x_velocity,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.x.filterCutoff"),
                "Filter Cutoff",
                &sense.x_filter_cutoff,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.x.filterResonance"),
                "Filter Resonance",
                &sense.x_filter_resonance,
                target,
            ),
            sense_axis_binding_group(
                &format!("{prefix}.y"),
                "Y Axis",
                sense.y_pitch_enabled,
                sense.y_pitch_steps,
                sense.y_pitch_restart_each_section,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.y.velocity"),
                "Velocity",
                &sense.y_velocity,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.y.filterCutoff"),
                "Filter Cutoff",
                &sense.y_filter_cutoff,
                target,
            ),
            sense_axis_lane_binding_group(
                &format!("{prefix}.y.filterResonance"),
                "Filter Resonance",
                &sense.y_filter_resonance,
                target,
            ),
        ],
    ))
}

fn sense_axis_binding_group(
    prefix: &str,
    label: &str,
    _enabled: bool,
    _steps: i32,
    _restart_each_section: bool,
    target: &str,
) -> NativeMenuItem {
    group(
        label,
        vec![group(
            "Pitch Steps",
            vec![
                binding_action(
                    "Enabled",
                    &format!("{prefix}.pitch.enabled"),
                    "bool",
                    None,
                    None,
                    None,
                    vec![],
                    target,
                ),
                binding_action(
                    "Steps",
                    &format!("{prefix}.pitch.steps"),
                    "number",
                    Some(-16),
                    Some(16),
                    Some(1),
                    vec![],
                    target,
                ),
                binding_action(
                    "Restart Section",
                    &format!("{prefix}.pitch.restartEachSection"),
                    "bool",
                    None,
                    None,
                    None,
                    vec![],
                    target,
                ),
            ],
        )],
    )
}

fn sense_axis_lane_binding_group(
    prefix: &str,
    label: &str,
    lane: &super::NativeValueLaneConfig,
    target: &str,
) -> NativeMenuItem {
    let _ = lane;
    group(
        label,
        vec![
            binding_action(
                "Enabled",
                &format!("{prefix}.enabled"),
                "bool",
                None,
                None,
                None,
                vec![],
                target,
            ),
            binding_action(
                "From",
                &format!("{prefix}.from"),
                "number",
                Some(0),
                Some(127),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "To",
                &format!("{prefix}.to"),
                "number",
                Some(0),
                Some(127),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "Grid Offset",
                &format!("{prefix}.gridOffset"),
                "number",
                Some(-7),
                Some(7),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "Curve",
                &format!("{prefix}.curve"),
                "enum",
                None,
                None,
                None,
                vec!["linear", "exp", "log"],
                target,
            ),
        ],
    )
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
