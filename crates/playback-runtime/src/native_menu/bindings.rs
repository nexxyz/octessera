use platform_core::BUS_COUNT as FX_BUS_COUNT;

use super::binding_behavior::behavior_binding_groups;
use super::binding_sense::sense_binding_group;
use super::binding_tree::{binding_action, binding_group_from_items, binding_tree_from_menu_item};
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
                sample_favourite_dirs: &config.sample_favourite_dirs,
                sample_builtin_favourite_dirs: &config.sample_builtin_favourite_dirs,
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
