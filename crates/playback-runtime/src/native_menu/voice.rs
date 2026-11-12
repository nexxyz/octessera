use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, pick_list,
    selected_index, signal_item, text_item, NativeMenuAction, NativeMenuItem,
    NativeSampleAvailability, NativeSampleBrowserConfig,
};

mod drum;
mod fm;
mod midi;
mod midi_drum;
mod mixer;
mod pluck;
mod sampler;
mod synth;
mod tile_stamp_items;

use crate::instrument_presets::instrument_presets;
use crate::native_menu::{FactoryLoadKind, PlatformAction};
use crate::native_runner::InstrumentType;
use drum::drum_group;
use fm::fm_group;
use midi::midi_group;
pub(super) use midi_drum::gm_percussion_name;
use midi_drum::midi_drum_group;
use mixer::mixer_group;
use platform_core::NoteBehavior;
use pluck::pluck_group;
use realtime_engine::synth::MixerRoute;
use realtime_engine::synth::SignalSource;
use sampler::sampler_group;
use synth::synth_group;

pub(super) struct InstrumentMenuConfig<'a> {
    pub(super) index: usize,
    pub(super) label: String,
    pub(super) name: &'a str,
    pub(super) kind: InstrumentType,
    pub(super) auto_name: bool,
    pub(super) note_behavior: NoteBehavior,
    pub(super) route: MixerRoute,
    pub(super) volume: u8,
    pub(super) pan_pos: u8,
    pub(super) sample_slot: usize,
    pub(super) sample_paths: &'a [Option<String>],
    pub(super) sample_availability: &'a [NativeSampleAvailability],
    pub(super) synth_config: Option<&'a serde_json::Value>,
    pub(super) fm_config: Option<&'a serde_json::Value>,
    pub(super) pluck_config: Option<&'a serde_json::Value>,
    pub(super) drum_config: Option<&'a serde_json::Value>,
    pub(super) drum_voice: usize,
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
    pub(super) sample_follow_transpose: bool,
    pub(super) sample_follow_pitch: bool,
    pub(super) tile_stamp: super::NativeMenuTileStamp,
    pub(super) sample_playback: super::NativeSamplePlayback,
    pub(super) midi_drum: super::NativeMenuMidiDrum,
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

pub(super) fn factory_load_group<'c>(
    kind: FactoryLoadKind,
    index: usize,
    label: &str,
    choices: impl IntoIterator<Item = (&'c str, &'c str)>,
) -> NativeMenuItem {
    group(
        label,
        vec![pick_list(group(
            "Load",
            choices
                .into_iter()
                .map(|(id, label)| {
                    action_item(
                        label,
                        format!("{}.{index}.{id}", kind.wire_name()),
                        NativeMenuAction::PlatformEffect(PlatformAction::FactoryInstrumentLoad {
                            kind,
                            slot: index,
                            id: id.into(),
                        }),
                    )
                })
                .collect(),
        ))],
    )
}

/// The factory presets of an instrument family as menu `(id, label)` choices.
pub(super) fn factory_preset_choices(
    family: &str,
) -> impl Iterator<Item = (&'static str, &'static str)> {
    instrument_presets(family)
        .iter()
        .map(|preset| (preset.id.as_str(), preset.label.as_str()))
}

pub(super) fn instrument_group(config: InstrumentMenuConfig<'_>) -> NativeMenuItem {
    let prefix = format!("instruments.{}", config.index);
    let type_selected = match config.kind {
        InstrumentType::None => 0,
        InstrumentType::Synth => 1,
        InstrumentType::Sampler => 2,
        InstrumentType::Midi => 3,
        InstrumentType::Fm => 4,
        InstrumentType::Pluck => 5,
        InstrumentType::Drum => 6,
        InstrumentType::MidiDrum => 7,
    };
    let mut children = vec![enum_item(
        "Type",
        format!("{prefix}.type"),
        vec![
            "none",
            "synth",
            "sampler",
            "midi",
            "fm",
            "pluck",
            "drum",
            "midi_drum",
        ],
        type_selected,
    )];
    if config.kind == InstrumentType::None {
        children.push(bool_item(
            "Auto Label",
            format!("{prefix}.autoName"),
            config.auto_name,
        ));
        children.push(text_item("Name", format!("{prefix}.name"), config.name, 32));
        return group(config.label.clone(), children);
    }
    if !matches!(config.kind, InstrumentType::Drum | InstrumentType::MidiDrum) {
        children.push(enum_item(
            "Note Mode",
            format!("{prefix}.noteBehavior"),
            vec!["oneshot", "hold"],
            selected_index(&["oneshot", "hold"], config.note_behavior.wire_name()),
        ));
    }
    if config.kind == InstrumentType::Synth {
        children.push(synth_group(&config, &prefix));
    }
    if config.kind == InstrumentType::Fm {
        children.push(fm_group(&config, &prefix));
    }
    if config.kind == InstrumentType::Pluck {
        children.push(pluck_group(&config, &prefix));
    }
    if config.kind == InstrumentType::Drum {
        children.push(drum_group(&config, &prefix));
    }
    if config.kind == InstrumentType::Sampler {
        children.push(sampler_group(&config, &prefix));
    }
    if config.kind == InstrumentType::Midi {
        children.push(midi_group(&config, &prefix));
    }
    if config.kind == InstrumentType::MidiDrum {
        children.push(midi_drum_group(&config, &prefix));
    }
    if matches!(
        config.kind,
        InstrumentType::Synth
            | InstrumentType::Sampler
            | InstrumentType::Fm
            | InstrumentType::Pluck
            | InstrumentType::Drum
    ) {
        children.push(mixer_group(&config, &prefix));
        children.push(signal_item(
            format!("signal.instrument.{}", config.index),
            SignalSource::Instrument(config.index),
        ));
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
