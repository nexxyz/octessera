use super::binding_tree::binding_tree_from_menu_item;
use super::voice::{instrument_group, InstrumentMenuConfig};
use super::{NativeMenuConfig, NativeMenuItem};

pub(super) fn instrument_binding_groups(
    config: &NativeMenuConfig,
    target: &str,
) -> Vec<NativeMenuItem> {
    config
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
                sample_paths: config
                    .instrument_sample_paths
                    .get(index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
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
        .collect()
}
