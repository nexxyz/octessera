use crate::protocol::SyncSource;

use super::dance::dance_group;
use super::fx::{fx_buses_group, global_fx_group};
use super::sense::{l2_part_group, l2_root_items};
use super::system::system_group;
use super::voice::{instrument_group, InstrumentMenuConfig};
use super::{NativeMenuConfig, NativeMenuItem, NativeMenuValue};

pub(super) fn build_root(config: NativeMenuConfig) -> NativeMenuItem {
    let sync_index = if config.sync_source == SyncSource::External {
        1
    } else {
        0
    };
    NativeMenuItem {
        label: "Menu".into(),
        key: None,
        value: NativeMenuValue::Group,
        children: vec![
            life_group(&config),
            sense_group(&config),
            voice_group(&config),
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

fn life_group(config: &NativeMenuConfig) -> NativeMenuItem {
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
    }
}

fn sense_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let instrument_options = config.instrument_labels.to_vec();
    NativeMenuItem {
        label: "L2: Sense".into(),
        key: None,
        value: NativeMenuValue::Group,
        children: l2_root_items(config)
            .into_iter()
            .chain(config.part_labels.iter().enumerate().map(|(index, label)| {
                l2_part_group(
                    index,
                    label.clone(),
                    &instrument_options,
                    config.sense_parts.get(index),
                    config,
                )
            }))
            .collect(),
    }
}

fn voice_group(config: &NativeMenuConfig) -> NativeMenuItem {
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
                    .map(|(index, label)| instrument_item(config, index, label))
                    .collect(),
            },
            fx_buses_group(&config.fx_buses),
            global_fx_group(&config.global_fx_slots, &config.global_fx_params),
        ],
    }
}

fn instrument_item(config: &NativeMenuConfig, index: usize, label: &str) -> NativeMenuItem {
    let kind = config
        .instrument_types
        .get(index)
        .map(String::as_str)
        .unwrap_or("synth");
    let route = config
        .instrument_routes
        .get(index)
        .map(String::as_str)
        .unwrap_or("direct");
    let sample_slot = config
        .instrument_sample_slots
        .get(index)
        .copied()
        .unwrap_or(0);
    let midi_channel = config
        .instrument_midi_channels
        .get(index)
        .copied()
        .unwrap_or(1);
    instrument_group(InstrumentMenuConfig {
        index,
        label: instrument_overview_label(label, kind, route, midi_channel),
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
        route,
        volume: config.instrument_volumes.get(index).copied().unwrap_or(100),
        pan_pos: config
            .instrument_pan_positions
            .get(index)
            .copied()
            .unwrap_or(16),
        sample_slot,
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
        sample_filter_env: config.instrument_sample_filter_envs.get(index),
        sample_favourite_dirs: &config.sample_favourite_dirs,
        sample_builtin_favourite_dirs: &config.sample_builtin_favourite_dirs,
        midi_enabled: config
            .instrument_midi_enabled
            .get(index)
            .copied()
            .unwrap_or(false),
        midi_channel,
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
}

fn instrument_overview_label(
    base_label: &str,
    kind: &str,
    route: &str,
    midi_channel: u8,
) -> String {
    let prefix = base_label.split_whitespace().next().unwrap_or(base_label);
    match kind {
        "sampler" => format!("{prefix} samp {route}"),
        "midi" => format!("{prefix} midi ch{midi_channel}"),
        "none" => format!("{prefix} none"),
        _ => format!("{prefix} synth {route}"),
    }
}
