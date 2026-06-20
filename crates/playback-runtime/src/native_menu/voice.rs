use super::sample_browser_menu::sample_browser_group;
use super::synth_preset_items::synth_preset_items;
use super::voice_config_read::{cutoff_hz_to_display, sample_number, sample_string, synth_number};
use super::voice_env_groups::{sample_env_group, synth_env_group};
use super::{
    action_item, bool_item, enum_item, group, number_item, selected_index, text_item,
    NativeMenuAction, NativeMenuItem, NativeSampleBrowserConfig,
};

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
    pub(super) midi_enabled: bool,
    pub(super) midi_channel: u8,
    pub(super) midi_velocity: u8,
    pub(super) midi_duration_ms: u16,
    pub(super) sample_browser: Option<&'a NativeSampleBrowserConfig>,
}

pub(super) fn instrument_group(config: InstrumentMenuConfig<'_>) -> NativeMenuItem {
    let InstrumentMenuConfig {
        index,
        label,
        name,
        kind,
        auto_name,
        note_behavior,
        route,
        volume,
        pan_pos,
        sample_slot,
        synth_config,
        synth_osc1_waveform,
        synth_osc2_waveform,
        synth_filter_type,
        synth_filter_cutoff,
        synth_gain_pct,
        synth_filter_resonance,
        sample_tune_semis,
        sample_gain_pct,
        sample_base_velocity,
        sample_amp_velocity_sensitivity_pct,
        sample_velocity_levels_enabled,
        sample_velocity_high,
        sample_velocity_medium,
        sample_velocity_low,
        sample_amp_env,
        sample_filter,
        sample_filter_env,
        midi_enabled,
        midi_channel,
        midi_velocity,
        midi_duration_ms,
        sample_browser,
    } = config;
    let prefix = format!("instruments.{index}");
    let type_selected = match kind {
        "none" => 0,
        "sampler" => 2,
        "midi" => 3,
        _ => 1,
    };
    let mut children = vec![
        enum_item(
            "Type",
            format!("{prefix}.type"),
            vec!["none", "synth", "sampler", "midi"],
            type_selected,
        ),
        enum_item(
            "Note Behavior",
            format!("{prefix}.noteBehavior"),
            vec!["oneshot", "hold"],
            selected_index(&["oneshot", "hold"], note_behavior),
        ),
    ];
    if kind == "synth" {
        children.push(group(
            "Synth",
            vec![
                group("Preset", vec![group("Load", synth_preset_items(index))]),
                group(
                    "Oscillator",
                    vec![
                        group(
                            "Osc 1",
                            vec![
                                enum_item(
                                    "Wave",
                                    format!("{prefix}.synth.osc1.waveform"),
                                    vec!["sine", "triangle", "saw", "square", "pulse"],
                                    selected_index(
                                        &["sine", "triangle", "saw", "square", "pulse"],
                                        synth_osc1_waveform,
                                    ),
                                ),
                                number_item(
                                    "Octave",
                                    format!("{prefix}.synth.osc1.octave"),
                                    synth_number(synth_config, &["osc1", "octave"], 0),
                                    -2,
                                    2,
                                    1,
                                ),
                                number_item(
                                    "Level",
                                    format!("{prefix}.synth.osc1.levelPct"),
                                    synth_number(synth_config, &["osc1", "levelPct"], 80),
                                    0,
                                    100,
                                    1,
                                ),
                                number_item(
                                    "Detune",
                                    format!("{prefix}.synth.osc1.detuneCents"),
                                    synth_number(synth_config, &["osc1", "detuneCents"], 0),
                                    -50,
                                    50,
                                    1,
                                ),
                                number_item(
                                    "Pulse Width",
                                    format!("{prefix}.synth.osc1.pulseWidthPct"),
                                    synth_number(synth_config, &["osc1", "pulseWidthPct"], 50),
                                    5,
                                    95,
                                    1,
                                ),
                            ],
                        ),
                        group(
                            "Osc 2",
                            vec![
                                enum_item(
                                    "Wave",
                                    format!("{prefix}.synth.osc2.waveform"),
                                    vec!["sine", "triangle", "saw", "square", "pulse"],
                                    selected_index(
                                        &["sine", "triangle", "saw", "square", "pulse"],
                                        synth_osc2_waveform,
                                    ),
                                ),
                                number_item(
                                    "Octave",
                                    format!("{prefix}.synth.osc2.octave"),
                                    synth_number(synth_config, &["osc2", "octave"], 0),
                                    -2,
                                    2,
                                    1,
                                ),
                                number_item(
                                    "Level",
                                    format!("{prefix}.synth.osc2.levelPct"),
                                    synth_number(synth_config, &["osc2", "levelPct"], 72),
                                    0,
                                    100,
                                    1,
                                ),
                                number_item(
                                    "Detune",
                                    format!("{prefix}.synth.osc2.detuneCents"),
                                    synth_number(synth_config, &["osc2", "detuneCents"], 0),
                                    -50,
                                    50,
                                    1,
                                ),
                                number_item(
                                    "Pulse Width",
                                    format!("{prefix}.synth.osc2.pulseWidthPct"),
                                    synth_number(synth_config, &["osc2", "pulseWidthPct"], 50),
                                    5,
                                    95,
                                    1,
                                ),
                            ],
                        ),
                    ],
                ),
                group(
                    "Filter",
                    vec![
                        enum_item(
                            "Type",
                            format!("{prefix}.synth.filter.type"),
                            vec!["lowpass", "highpass", "bandpass", "notch"],
                            selected_index(
                                &["lowpass", "highpass", "bandpass", "notch"],
                                synth_filter_type,
                            ),
                        ),
                        number_item(
                            "Cutoff",
                            format!("{prefix}.synth.filter.cutoffHz"),
                            cutoff_hz_to_display(i32::from(synth_filter_cutoff)),
                            0,
                            255,
                            1,
                        ),
                        number_item(
                            "Res",
                            format!("{prefix}.synth.filter.resonance"),
                            i32::from(synth_filter_resonance),
                            0,
                            255,
                            1,
                        ),
                        number_item(
                            "Env Amount",
                            format!("{prefix}.synth.filter.envAmountPct"),
                            synth_number(synth_config, &["filter", "envAmountPct"], 0),
                            -100,
                            100,
                            1,
                        ),
                        number_item(
                            "Key Tracking",
                            format!("{prefix}.synth.filter.keyTrackingPct"),
                            synth_number(synth_config, &["filter", "keyTrackingPct"], 0),
                            0,
                            100,
                            1,
                        ),
                    ],
                ),
                group(
                    "Volume",
                    vec![
                        number_item(
                            "Gain",
                            format!("{prefix}.synth.amp.gainPct"),
                            i32::from(synth_gain_pct),
                            0,
                            100,
                            1,
                        ),
                        number_item(
                            "Velocity Sens",
                            format!("{prefix}.synth.amp.velocitySensitivityPct"),
                            synth_number(synth_config, &["amp", "velocitySensitivityPct"], 100),
                            0,
                            100,
                            1,
                        ),
                    ],
                ),
                synth_env_group("Amp Env", &prefix, "ampEnv", synth_config),
                synth_env_group("Filter Env", &prefix, "filterEnv", synth_config),
            ],
        ));
    }
    if kind == "sampler" {
        let mut sampler_children = vec![
            enum_item(
                "Sample Slot",
                format!("{prefix}.sample.selectedSlot"),
                vec!["1", "2", "3", "4", "5", "6", "7", "8"],
                sample_slot.min(7),
            ),
            sample_browser_group(index, sample_slot.min(7), sample_browser),
            action_item(
                "Assign",
                format!("sample.assign.{index}.{}", sample_slot.min(7)),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.assign:{index}:{}",
                    sample_slot.min(7)
                )),
            ),
            number_item(
                "Tune",
                format!("{prefix}.sample.tuneSemis"),
                i32::from(sample_tune_semis),
                -24,
                24,
                1,
            ),
            number_item(
                "Gain",
                format!("{prefix}.sample.amp.gainPct"),
                i32::from(sample_gain_pct),
                0,
                100,
                1,
            ),
            number_item(
                "Base Velocity",
                format!("{prefix}.sample.baseVelocity"),
                i32::from(sample_base_velocity),
                1,
                127,
                1,
            ),
            bool_item(
                "Velocity Levels",
                format!("{prefix}.sample.velocityLevelsEnabled"),
                sample_velocity_levels_enabled,
            ),
            group(
                "Filter",
                vec![
                    enum_item(
                        "Type",
                        format!("{prefix}.sample.filter.type"),
                        vec!["lowpass", "highpass", "bandpass", "notch"],
                        selected_index(
                            &["lowpass", "highpass", "bandpass", "notch"],
                            sample_string(sample_filter, &["type"], "lowpass").as_str(),
                        ),
                    ),
                    number_item(
                        "Cutoff",
                        format!("{prefix}.sample.filter.cutoffHz"),
                        cutoff_hz_to_display(sample_number(sample_filter, &["cutoffHz"], 8000)),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Res",
                        format!("{prefix}.sample.filter.resonance"),
                        sample_number(sample_filter, &["resonance"], 20),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Env Amount",
                        format!("{prefix}.sample.filter.envAmountPct"),
                        sample_number(sample_filter, &["envAmountPct"], 0),
                        -100,
                        100,
                        1,
                    ),
                    number_item(
                        "Key Tracking",
                        format!("{prefix}.sample.filter.keyTrackingPct"),
                        sample_number(sample_filter, &["keyTrackingPct"], 0),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            number_item(
                "Velocity Sens",
                format!("{prefix}.sample.amp.velocitySensitivityPct"),
                i32::from(sample_amp_velocity_sensitivity_pct),
                0,
                100,
                1,
            ),
            sample_env_group("Amp Env", &prefix, "ampEnv", sample_amp_env),
            sample_env_group("Filter Env", &prefix, "filterEnv", sample_filter_env),
        ];
        if sample_velocity_levels_enabled {
            sampler_children.insert(
                7,
                group(
                    "Velocity Levels",
                    vec![
                        number_item(
                            "High",
                            format!("{prefix}.sample.velocityLevels.high"),
                            i32::from(sample_velocity_high),
                            1,
                            127,
                            1,
                        ),
                        number_item(
                            "Medium",
                            format!("{prefix}.sample.velocityLevels.medium"),
                            i32::from(sample_velocity_medium),
                            1,
                            127,
                            1,
                        ),
                        number_item(
                            "Low",
                            format!("{prefix}.sample.velocityLevels.low"),
                            i32::from(sample_velocity_low),
                            1,
                            127,
                            1,
                        ),
                    ],
                ),
            );
        }
        children.push(group("Sampler", sampler_children));
    }
    if kind == "midi" {
        children.push(group(
            "MIDI",
            vec![
                bool_item("Enabled", format!("{prefix}.midi.enabled"), midi_enabled),
                number_item(
                    "Channel",
                    format!("{prefix}.midi.channel"),
                    i32::from(midi_channel),
                    1,
                    16,
                    1,
                ),
                number_item(
                    "Velocity",
                    format!("{prefix}.midi.velocity"),
                    i32::from(midi_velocity),
                    1,
                    127,
                    1,
                ),
                number_item(
                    "Duration",
                    format!("{prefix}.midi.durationMs"),
                    i32::from(midi_duration_ms),
                    10,
                    2000,
                    10,
                ),
            ],
        ));
    }
    if kind == "synth" || kind == "sampler" {
        children.push(group(
            "Mixer",
            vec![
                enum_item(
                    "Route",
                    format!("{prefix}.mixer.route"),
                    vec!["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                    selected_index(
                        &["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                        route,
                    ),
                ),
                number_item(
                    "Volume",
                    format!("{prefix}.mixer.volume"),
                    i32::from(volume),
                    0,
                    100,
                    1,
                ),
                number_item(
                    "Pan Pos",
                    format!("{prefix}.mixer.panPos"),
                    i32::from(pan_pos),
                    0,
                    32,
                    1,
                ),
            ],
        ));
    }
    children.push(bool_item(
        "Auto Name",
        format!("{prefix}.autoName"),
        auto_name,
    ));
    children.push(text_item("Name", format!("{prefix}.name"), name, 32));
    children.push(action_item(
        "Clone",
        format!("instruments.{index}.clone"),
        NativeMenuAction::CloneInstrument { index },
    ));
    children.push(action_item(
        "Reset",
        format!("instruments.{index}.reset"),
        NativeMenuAction::ResetInstrument { index },
    ));
    group(label, children)
}
