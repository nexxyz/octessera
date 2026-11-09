use crate::native_runner::InstrumentType;
use crate::protocol::SyncSource;
use platform_core::NoteBehavior;

use super::binding_picker_rows::name_parameter_pickers;
use super::format_values::compact_route_value;
use super::fx::{fx_buses_group, global_fx_group};
use super::link::{link_layer_group, link_root_items};
use super::play::play_group;
use super::section_labels::{BUILD_LABEL, LINK_LABEL, SHAPE_LABEL};
use super::system::system_group;
use super::system_aux::name_click_pickers_from_menu;
use super::voice::{instrument_group, InstrumentMenuConfig};
use super::{number_item, NativeMenuConfig, NativeMenuItem, NativeMenuPick, NativeMenuValue};

pub(super) fn build_root(config: NativeMenuConfig, tree: &[NativeMenuItem]) -> NativeMenuItem {
    let sync_index = if config.sync_source == SyncSource::External {
        1
    } else {
        0
    };
    let mut root = NativeMenuItem {
        label: "Menu".into(),
        breadcrumb: None,
        key: None,
        value: NativeMenuValue::Group,
        children: vec![
            build_group(&config),
            link_group(&config),
            shape_group(&config),
            play_group(&config),
            NativeMenuItem {
                label: "".into(),
                breadcrumb: None,
                key: None,
                value: NativeMenuValue::Group,
                children: vec![],
                pick: NativeMenuPick::None,
            },
            system_group(&config, sync_index),
        ],
        pick: NativeMenuPick::None,
    };
    name_parameter_pickers(&mut root, tree);
    name_click_pickers_from_menu(&mut root, tree);
    root
}

fn build_group(config: &NativeMenuConfig) -> NativeMenuItem {
    NativeMenuItem {
        label: BUILD_LABEL.into(),
        breadcrumb: None,
        key: None,
        value: NativeMenuValue::Group,
        children: config
            .layer_labels
            .iter()
            .enumerate()
            .map(|(index, label)| NativeMenuItem {
                label: label.clone(),
                breadcrumb: None,
                key: None,
                value: NativeMenuValue::Group,
                children: config
                    .build_items_by_layer
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| config.build_items.clone()),
                pick: NativeMenuPick::None,
            })
            .chain(std::iter::once(number_item(
                "Seed",
                "randomSeed",
                i32::from(config.random_seed),
                1,
                9999,
                1,
            )))
            .collect(),
        pick: NativeMenuPick::None,
    }
}

fn link_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let instrument_options = config.instrument_labels.to_vec();
    NativeMenuItem {
        label: LINK_LABEL.into(),
        breadcrumb: None,
        key: None,
        value: NativeMenuValue::Group,
        children: [
            number_item("BPM", "transport.bpm", i32::from(config.bpm), 40, 240, 1),
            number_item(
                "Swing A",
                "transport.swingA",
                i32::from(config.swing_a),
                1,
                7,
                1,
            ),
            number_item(
                "Swing B",
                "transport.swingB",
                i32::from(config.swing_b),
                1,
                7,
                1,
            ),
        ]
        .into_iter()
        .chain(link_root_items(config))
        .chain(
            config
                .layer_labels
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    link_layer_group(
                        index,
                        label.clone(),
                        &instrument_options,
                        config.link_layers.get(index),
                        config,
                    )
                }),
        )
        .collect(),
        pick: NativeMenuPick::None,
    }
}

fn shape_group(config: &NativeMenuConfig) -> NativeMenuItem {
    NativeMenuItem {
        label: SHAPE_LABEL.into(),
        breadcrumb: None,
        key: None,
        value: NativeMenuValue::Group,
        children: vec![
            NativeMenuItem {
                label: "Instruments".into(),
                breadcrumb: None,
                key: None,
                value: NativeMenuValue::Group,
                children: config
                    .instrument_labels
                    .iter()
                    .enumerate()
                    .map(|(index, label)| instrument_item(config, index, label))
                    .collect(),
                pick: NativeMenuPick::None,
            },
            fx_buses_group(&config.fx_buses, config.bpm),
            global_fx_group(
                &config.global_fx_slots,
                &config.global_fx_params,
                config.bpm,
            ),
        ],
        pick: NativeMenuPick::None,
    }
}

pub(super) fn instrument_item(
    config: &NativeMenuConfig,
    index: usize,
    label: &str,
) -> NativeMenuItem {
    let kind = instrument_kind(config, index);
    let route = config
        .instrument_routes
        .get(index)
        .copied()
        .unwrap_or_default();
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
    let mut item = instrument_group(InstrumentMenuConfig {
        index,
        label: instrument_item_label(config, index, label),
        name: config
            .instrument_names
            .get(index)
            .map(String::as_str)
            .unwrap_or(kind.wire_name()),
        kind,
        auto_name: config
            .instrument_auto_names
            .get(index)
            .copied()
            .unwrap_or(true),
        note_behavior: config
            .instrument_note_behaviors
            .get(index)
            .copied()
            .unwrap_or(NoteBehavior::Oneshot),
        route,
        volume: config.instrument_volumes.get(index).copied().unwrap_or(100),
        pan_pos: config
            .instrument_pan_positions
            .get(index)
            .copied()
            .unwrap_or(16),
        sample_slot,
        sample_paths: config
            .instrument_sample_paths
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        sample_availability: config
            .instrument_sample_availability
            .get(index)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        synth_config: config.instrument_synth_configs.get(index),
        fm_config: config.instrument_fm_configs.get(index),
        pluck_config: config.instrument_pluck_configs.get(index),
        drum_config: config.instrument_drum_configs.get(index),
        drum_voice: config
            .instrument_drum_selected_voices
            .get(index)
            .copied()
            .unwrap_or(0),
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
        sample_follow_transpose: config
            .instrument_sample_follow_transpose
            .get(index)
            .copied()
            .unwrap_or(false),
        sample_follow_pitch: config
            .instrument_sample_follow_pitch
            .get(index)
            .copied()
            .unwrap_or(false),
        tile_stamp: config
            .instrument_tile_stamps
            .get(index)
            .copied()
            .unwrap_or_default(),
        sample_playback: config
            .instrument_sample_playback
            .get(index)
            .copied()
            .unwrap_or_default(),
        midi_drum: config
            .instrument_midi_drums
            .get(index)
            .copied()
            .unwrap_or_default(),
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
    });
    item.breadcrumb = Some(instrument_breadcrumb_label(config, index, label));
    item
}

pub(super) fn instrument_item_label(
    config: &NativeMenuConfig,
    index: usize,
    base_label: &str,
) -> String {
    let short = instrument_breadcrumb_label(config, index, base_label);
    let suffix = match instrument_kind(config, index) {
        InstrumentType::Midi => format!(
            "ch{}",
            config
                .instrument_midi_channels
                .get(index)
                .copied()
                .unwrap_or(1)
        ),
        InstrumentType::MidiDrum => format!(
            "ch{}",
            config
                .instrument_midi_drums
                .get(index)
                .copied()
                .unwrap_or_default()
                .channel
        ),
        InstrumentType::None => return short,
        _ => compact_route_value(
            config
                .instrument_routes
                .get(index)
                .copied()
                .unwrap_or_default()
                .wire_name(),
        ),
    };
    format!("{short} {suffix}")
}

fn instrument_breadcrumb_label(
    config: &NativeMenuConfig,
    index: usize,
    base_label: &str,
) -> String {
    let prefix = base_label.split_whitespace().next().unwrap_or(base_label);
    let kind =
        super::format_values::instrument_kind_label(instrument_kind(config, index).wire_name());
    format!("{prefix} {kind}")
}

fn instrument_kind(config: &NativeMenuConfig, index: usize) -> InstrumentType {
    config
        .instrument_types
        .get(index)
        .copied()
        .unwrap_or(InstrumentType::Synth)
}
