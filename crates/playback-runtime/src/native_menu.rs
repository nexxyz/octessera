use crate::protocol::SyncSource;
use bindings::{axis_binding_label, parameter_picker_group, xy_pad_items};
use dance::dance_group;
#[cfg(test)]
use fx::default_fx_bus_config;
use fx::{fx_buses_group, global_fx_group};
#[cfg(test)]
use options::{FX_BUS_SLOT_OPTIONS, GLOBAL_FX_SLOT_OPTIONS};
#[cfg(test)]
use platform_core::{BUS_COUNT as FX_BUS_COUNT, GLOBAL_FX_SLOT_COUNT};
#[cfg(test)]
use sense::default_sense_part_config;
use sense::l2_part_group;
use system::{aux_mappings_group, system_group};
use voice::{instrument_group, InstrumentMenuConfig};

mod binding_behavior;
mod binding_sense;
mod binding_tree;
mod bindings;
mod dance;
mod format;
mod fx;
mod help;
mod model;
mod model_binding_specs;
mod model_edit;
mod model_search;
mod model_snapshot;
mod options;
mod sample_browser_menu;
mod sense;
mod synth_preset_items;
mod system;
mod types;
mod voice;
mod voice_config_read;
mod voice_env_groups;

pub use model::NativeMenuPressResult;
pub(crate) use options::{is_valid_fx_bus_slot_type, is_valid_global_fx_slot_type};
pub use types::*;

fn build_root(config: NativeMenuConfig) -> NativeMenuItem {
    let sync_index = if config.sync_source == SyncSource::External {
        1
    } else {
        0
    };
    let instrument_options = config.instrument_labels.to_vec();
    NativeMenuItem {
        label: "Menu".into(),
        key: None,
        value: NativeMenuValue::Group,
        children: vec![
            NativeMenuItem {
                label: "L1: Life".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: config
                    .part_labels
                    .iter()
                    .map(|label| NativeMenuItem {
                        label: label.clone(),
                        key: None,
                        value: NativeMenuValue::Group,
                        children: config.l1_items.clone(),
                    })
                    .collect(),
            },
            NativeMenuItem {
                label: "L2: Sense".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: std::iter::once(aux_mappings_group(&config))
                    .chain(config.part_labels.iter().enumerate().map(|(index, label)| {
                        l2_part_group(
                            index,
                            label.clone(),
                            &instrument_options,
                            config.sense_parts.get(index),
                            &config,
                        )
                    }))
                    .collect(),
            },
            NativeMenuItem {
                label: "L3: Voice".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: vec![
                    NativeMenuItem {
                        label: "Instruments".into(),
                        key: None,
                        value: NativeMenuValue::Group,
                        children: config
                            .instrument_labels
                            .iter()
                            .enumerate()
                            .map(|(index, label)| {
                                let kind = config
                                    .instrument_types
                                    .get(index)
                                    .map(String::as_str)
                                    .unwrap_or("synth");
                                instrument_group(InstrumentMenuConfig {
                                    index,
                                    label: label.clone(),
                                    name: config
                                        .instrument_names
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or(kind),
                                    kind,
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
                                    volume: config
                                        .instrument_volumes
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
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
                                        .unwrap_or(80),
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
                                        .unwrap_or(120),
                                    sample_velocity_medium: config
                                        .instrument_sample_velocity_medium
                                        .get(index)
                                        .copied()
                                        .unwrap_or(85),
                                    sample_velocity_low: config
                                        .instrument_sample_velocity_low
                                        .get(index)
                                        .copied()
                                        .unwrap_or(45),
                                    sample_amp_env: config.instrument_sample_amp_envs.get(index),
                                    sample_filter: config.instrument_sample_filters.get(index),
                                    sample_filter_env: config
                                        .instrument_sample_filter_envs
                                        .get(index),
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
                                        .unwrap_or(120),
                                    sample_browser: config.sample_browser.as_ref(),
                                })
                            })
                            .collect(),
                    },
                    fx_buses_group(&config.fx_buses),
                    global_fx_group(&config.global_fx_slots, &config.global_fx_params),
                ],
            },
            dance_group(&config),
            NativeMenuItem {
                label: "".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: vec![],
            },
            system_group(&config, sync_index),
        ],
    }
}

fn group(label: impl Into<String>, children: Vec<NativeMenuItem>) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: None,
        value: NativeMenuValue::Group,
        children,
    }
}

fn enum_item(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<&str>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum {
            options: options.into_iter().map(String::from).collect(),
            selected,
        },
        children: vec![],
    }
}

fn number_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Number {
            value,
            min,
            max,
            step,
        },
        children: vec![],
    }
}

fn bool_item(label: impl Into<String>, key: impl Into<String>, value: bool) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Bool { value },
        children: vec![],
    }
}

fn text_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: impl Into<String>,
    max_len: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Text {
            value: value.into(),
            max_len,
            cursor: 0,
        },
        children: vec![],
    }
}

fn action_item(
    label: impl Into<String>,
    key: impl Into<String>,
    action: NativeMenuAction,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Action(action),
        children: vec![],
    }
}

fn selected_index(options: &[&str], value: &str) -> usize {
    options
        .iter()
        .position(|option| *option == value)
        .unwrap_or(0)
}

fn slot_option_selected(slot: usize, option_count: usize) -> usize {
    if slot == usize::MAX {
        0
    } else {
        (slot + 1).min(option_count.saturating_sub(1))
    }
}

fn enum_item_from_strings(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<String>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum { options, selected },
        children: vec![],
    }
}

#[cfg(test)]
mod tests;
