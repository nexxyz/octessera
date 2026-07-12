use super::{
    action_item, bool_item, enum_item, group, number_item, selected_index, text_item,
    NativeMenuAction, NativeMenuItem, NativeSampleBrowserConfig,
};

mod midi;
mod mixer;
mod sampler;
mod synth;

use midi::midi_group;
use mixer::mixer_group;
use sampler::sampler_group;
use synth::synth_group;

pub(super) struct InstrumentMenuConfig<'a> {
    pub(super) index: usize,
    pub(super) label: String,
    pub(super) name: &'a str,
    pub(super) kind: &'a str,
    pub(super) auto_name: bool,
    pub(super) note_behavior: &'a str,
    pub(super) route: &'a str,
    pub(super) volume: u8,
    pub(super) pan_pos: u8,
    pub(super) sample_slot: usize,
    pub(super) sample_paths: &'a [Option<String>],
    pub(super) synth_config: Option<&'a serde_json::Value>,
    pub(super) synth_osc1_waveform: &'a str,
    pub(super) synth_osc2_waveform: &'a str,
    pub(super) synth_filter_type: &'a str,
    pub(super) synth_filter_cutoff: u16,
    pub(super) synth_gain_pct: u8,
    pub(super) synth_filter_resonance: u8,
    pub(super) sample_tune_semis: i8,
    pub(super) sample_gain_pct: u8,
    pub(super) sample_base_velocity: u8,
    pub(super) sample_amp_velocity_sensitivity_pct: u8,
    pub(super) sample_velocity_levels_enabled: bool,
    pub(super) sample_velocity_high: u8,
    pub(super) sample_velocity_medium: u8,
    pub(super) sample_velocity_low: u8,
    pub(super) sample_amp_env: Option<&'a serde_json::Value>,
    pub(super) sample_filter: Option<&'a serde_json::Value>,
    pub(super) sample_filter_env: Option<&'a serde_json::Value>,
    pub(super) sample_favourite_dirs: &'a [String],
    pub(super) sample_builtin_favourite_dirs: &'a [String],
    pub(super) midi_enabled: bool,
    pub(super) midi_channel: u8,
    pub(super) midi_velocity: u8,
    pub(super) midi_duration_ms: u16,
    pub(super) sample_browser: Option<&'a NativeSampleBrowserConfig>,
}

pub(super) fn instrument_group(config: InstrumentMenuConfig<'_>) -> NativeMenuItem {
    let prefix = format!("instruments.{}", config.index);
    let type_selected = match config.kind {
        "none" => 0,
        "sampler" => 2,
        "midi" => 3,
        _ => 1,
    };
    let mut children = vec![enum_item(
        "Type",
        format!("{prefix}.type"),
        vec!["none", "synth", "sampler", "midi"],
        type_selected,
    )];
    if config.kind == "none" {
        children.push(bool_item(
            "Auto Label",
            format!("{prefix}.autoName"),
            config.auto_name,
        ));
        children.push(text_item("Name", format!("{prefix}.name"), config.name, 32));
        return group(config.label.clone(), children);
    }
    children.push(enum_item(
        "Note Mode",
        format!("{prefix}.noteBehavior"),
        vec!["oneshot", "hold"],
        selected_index(&["oneshot", "hold"], config.note_behavior),
    ));
    if config.kind == "synth" {
        children.push(synth_group(&config, &prefix));
    }
    if config.kind == "sampler" {
        children.push(sampler_group(&config, &prefix));
    }
    if config.kind == "midi" {
        children.push(midi_group(&config, &prefix));
    }
    if matches!(config.kind, "synth" | "sampler") {
        children.push(mixer_group(&config, &prefix));
    }
    children.push(bool_item(
        "Auto Label",
        format!("{prefix}.autoName"),
        config.auto_name,
    ));
    children.push(text_item("Name", format!("{prefix}.name"), config.name, 32));
    children.push(group(
        "Slot Actions",
        vec![
            action_item(
                "Clone",
                format!("instruments.{}.clone", config.index),
                NativeMenuAction::CloneInstrument {
                    index: config.index,
                },
            ),
            action_item(
                "Reset",
                format!("instruments.{}.reset", config.index),
                NativeMenuAction::ResetInstrument {
                    index: config.index,
                },
            ),
        ],
    ));
    group(config.label.clone(), children)
}
