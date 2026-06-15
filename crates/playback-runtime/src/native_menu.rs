use crate::protocol::SyncSource;
use bindings::{axis_binding_label, dance_fx_targets, parameter_picker_group, xy_pad_items};
use options::{duck_source_options, FX_BUS_SLOT_OPTIONS, GLOBAL_FX_SLOT_OPTIONS};
use platform_core::{BUS_COUNT as FX_BUS_COUNT, GLOBAL_FX_SLOT_COUNT};
use system::{aux_mappings_group, system_group};

mod bindings;
mod format;
mod help;
mod model;
mod model_helpers;
mod options;
mod system;
mod types;

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

fn dance_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = vec![
        NativeMenuItem {
            label: "Dance Page".into(),
            key: Some("danceMode".into()),
            value: NativeMenuValue::Enum {
                options: vec![
                    "none".into(),
                    "mix".into(),
                    "pan".into(),
                    "fx".into(),
                    "trigger-gate".into(),
                    "xy".into(),
                ],
                selected: ["none", "mix", "pan", "fx", "trigger-gate", "xy"]
                    .iter()
                    .position(|mode| *mode == config.dance_mode)
                    .unwrap_or(0),
            },
            children: vec![],
        },
        number_item("BPM", "transport.bpm", i32::from(config.bpm), 40, 240, 1),
    ];
    match config.dance_mode.as_str() {
        "fx" => children.extend(dance_fx_page_items(config)),
        "trigger-gate" => children.push(group("Mode Grid", vec![])),
        "xy" => children.extend(xy_pad_items(config)),
        _ => {}
    }
    group("L4: Dance", children)
}

fn dance_fx_page_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let fx_types = vec!["none", "stutter", "freeze", "filter_sweep", "pitch_shift"];
    let targets = dance_fx_targets();
    let mut children = vec![
        enum_item(
            "FX Type",
            "dance.fx.type",
            fx_types.clone(),
            selected_index(&fx_types, &config.dance_fx_type),
        ),
        enum_item_from_strings(
            "Target",
            "dance.fx.target",
            targets.clone(),
            targets
                .iter()
                .position(|target| target == &config.dance_fx_target)
                .unwrap_or(0),
        ),
    ];
    match config.dance_fx_type.as_str() {
        "stutter" => {
            children.push(number_item(
                "Rate Hz",
                "dance.fx.params.rateHz",
                number_param(&config.dance_fx_params, "rateHz", 8),
                1,
                32,
                1,
            ));
            children.push(number_item(
                "Depth",
                "dance.fx.params.depthPct",
                number_param(&config.dance_fx_params, "depthPct", 100),
                0,
                100,
                1,
            ));
        }
        "freeze" => {
            children.push(number_item(
                "Release Ms",
                "dance.fx.params.releaseMs",
                number_param(&config.dance_fx_params, "releaseMs", 500),
                10,
                5000,
                10,
            ));
            children.push(number_item(
                "Mix",
                "dance.fx.params.mixPct",
                number_param(&config.dance_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        "filter_sweep" => {
            children.push(number_item(
                "Cutoff",
                "dance.fx.params.cutoffPct",
                number_param(&config.dance_fx_params, "cutoffPct", 50),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Res",
                "dance.fx.params.resonancePct",
                number_param(&config.dance_fx_params, "resonancePct", 0),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Sweep In",
                "dance.fx.params.sweepInMs",
                number_param(&config.dance_fx_params, "sweepInMs", 120),
                10,
                3000,
                10,
            ));
            children.push(number_item(
                "Sweep Out",
                "dance.fx.params.sweepOutMs",
                number_param(&config.dance_fx_params, "sweepOutMs", 180),
                10,
                3000,
                10,
            ));
        }
        "pitch_shift" => {
            children.push(number_item(
                "Semitones",
                "dance.fx.params.semitones",
                number_param(&config.dance_fx_params, "semitones", 0),
                -24,
                24,
                1,
            ));
            children.push(number_item(
                "Cents",
                "dance.fx.params.cents",
                number_param(&config.dance_fx_params, "cents", 0),
                -100,
                100,
                1,
            ));
            children.push(number_item(
                "Mix",
                "dance.fx.params.mixPct",
                number_param(&config.dance_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        _ => {}
    }
    children.push(action_item(
        "Map to Grid",
        "dance.fx.map",
        NativeMenuAction::PlatformEffect("dance.fx.map".into()),
    ));
    children
}

fn number_param(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: i32,
) -> i32 {
    params
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .map(|value| value as i32)
        .unwrap_or(default)
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

fn default_sense_part_config() -> NativeSensePartConfig {
    NativeSensePartConfig {
        scan_mode: "immediate".into(),
        scan_axis: "rows".into(),
        scan_unit: "1/8".into(),
        scan_direction: "forward".into(),
        scan_sections: 1,
        scanned_slot: 0,
        scanned_action: "note_on".into(),
        scanned_empty_slot: usize::MAX,
        scanned_empty_action: "none".into(),
        event_enabled: true,
        activate_slot: 0,
        activate_action: "note_on".into(),
        stable_slot: 0,
        stable_action: "note_on".into(),
        deactivate_slot: 0,
        deactivate_action: "note_on".into(),
        trigger_probability_mode: "full".into(),
        trigger_probability_low_pct: 0,
        trigger_probability_high_pct: 100,
        state_notes_enabled: true,
        lowest_note: 24,
        highest_note: 84,
        starting_note: 60,
        scale: "chromatic".into(),
        root: "C".into(),
        out_of_range: "wrap".into(),
        x_pitch_enabled: true,
        x_pitch_steps: 1,
        x_pitch_restart_each_section: false,
        y_pitch_enabled: true,
        y_pitch_steps: 3,
        y_pitch_restart_each_section: false,
        x_from: 0,
        x_to: 7,
        x_velocity: value_lane_config(1, 127),
        x_filter_cutoff: value_lane_config(20, 127),
        x_filter_resonance: value_lane_config(10, 90),
        y_from: 0,
        y_to: 7,
        y_velocity: value_lane_config(1, 127),
        y_filter_cutoff: value_lane_config(20, 127),
        y_filter_resonance: value_lane_config(10, 90),
    }
}

fn value_lane_config(from: u8, to: u8) -> NativeValueLaneConfig {
    NativeValueLaneConfig {
        enabled: false,
        from,
        to,
        grid_offset: 0,
        curve: "linear".into(),
    }
}

fn l2_part_group(
    index: usize,
    label: String,
    instrument_options: &[String],
    sense: Option<&NativeSensePartConfig>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("parts.{index}.l2");
    let instrument_options = if instrument_options.is_empty() {
        vec!["none".to_string()]
    } else {
        let mut options = vec!["none".to_string()];
        options.extend(instrument_options.iter().cloned());
        options
    };
    let default_sense = default_sense_part_config();
    let sense = sense.unwrap_or(&default_sense);
    let mut scanning_children = vec![enum_item(
        "Scan Mode",
        format!("{prefix}.scanMode"),
        vec!["immediate", "scanning"],
        selected_index(&["immediate", "scanning"], &sense.scan_mode),
    )];
    if sense.scan_mode == "scanning" {
        scanning_children.extend(vec![
            enum_item(
                "Scan Axis",
                format!("{prefix}.scanAxis"),
                vec!["rows", "columns"],
                selected_index(&["rows", "columns"], &sense.scan_axis),
            ),
            enum_item(
                "Scan Unit",
                format!("{prefix}.scanUnit"),
                vec!["1/16", "1/8", "1/4", "1/2", "1/1"],
                selected_index(&["1/16", "1/8", "1/4", "1/2", "1/1"], &sense.scan_unit),
            ),
            enum_item(
                "Scan Direction",
                format!("{prefix}.scanDirection"),
                vec!["forward", "reverse"],
                selected_index(&["forward", "reverse"], &sense.scan_direction),
            ),
            enum_item(
                "Sections",
                format!("{prefix}.scanSections"),
                vec!["1", "2", "4", "8"],
                selected_index(&["1", "2", "4", "8"], &sense.scan_sections.to_string()),
            ),
            enum_item_from_strings(
                "Instrument",
                format!("{prefix}.mapping.scanned.slot"),
                instrument_options.clone(),
                slot_option_selected(sense.scanned_slot, instrument_options.len()),
            ),
            enum_item(
                "Action",
                format!("{prefix}.mapping.scanned.action"),
                vec!["none", "note_on", "note_off"],
                selected_index(&["none", "note_on", "note_off"], &sense.scanned_action),
            ),
            enum_item_from_strings(
                "Empty Instrument",
                format!("{prefix}.mapping.scanned_empty.slot"),
                instrument_options.clone(),
                slot_option_selected(sense.scanned_empty_slot, instrument_options.len()),
            ),
            enum_item(
                "Empty Action",
                format!("{prefix}.mapping.scanned_empty.action"),
                vec!["none", "note_on", "note_off"],
                selected_index(
                    &["none", "note_on", "note_off"],
                    &sense.scanned_empty_action,
                ),
            ),
        ]);
    }
    group(
        label,
        vec![
            group("Scanning", scanning_children),
            group(
                "Events",
                vec![
                    bool_item(
                        "Event Triggers",
                        format!("{prefix}.eventEnabled"),
                        sense.event_enabled,
                    ),
                    bool_item(
                        "State Notes",
                        format!("{prefix}.stateNotesEnabled"),
                        sense.state_notes_enabled,
                    ),
                    enum_item_from_strings(
                        "Activate Instrument",
                        format!("{prefix}.mapping.activate.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.activate_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Activate Action",
                        format!("{prefix}.mapping.activate.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.activate_action),
                    ),
                    enum_item_from_strings(
                        "Stable Instrument",
                        format!("{prefix}.mapping.stable.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.stable_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Stable Action",
                        format!("{prefix}.mapping.stable.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.stable_action),
                    ),
                    enum_item_from_strings(
                        "Deactivate Instrument",
                        format!("{prefix}.mapping.deactivate.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.deactivate_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Deactivate Action",
                        format!("{prefix}.mapping.deactivate.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.deactivate_action),
                    ),
                ],
            ),
            group(
                "Trigger Prob.",
                vec![
                    enum_item(
                        "Mode",
                        format!("{prefix}.triggerProbabilityMode"),
                        vec!["zero", "custom", "full"],
                        selected_index(
                            &["zero", "custom", "full"],
                            &sense.trigger_probability_mode,
                        ),
                    ),
                    number_item(
                        "Low Prob",
                        format!("{prefix}.triggerProbabilityLowPct"),
                        i32::from(sense.trigger_probability_low_pct),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "High Prob",
                        format!("{prefix}.triggerProbabilityHighPct"),
                        i32::from(sense.trigger_probability_high_pct),
                        0,
                        100,
                        1,
                    ),
                    action_item(
                        "Map Probability Grid",
                        format!("{prefix}.triggerProbability.map"),
                        NativeMenuAction::PlatformEffect(format!(
                            "trigger.probability.assign:{index}"
                        )),
                    ),
                ],
            ),
            group(
                "Mappings",
                vec![
                    param_mod_axis_group(index, "X Axis", "x", config),
                    param_mod_axis_group(index, "Y Axis", "y", config),
                ],
            ),
            group(
                "Note Mapping",
                vec![
                    number_item(
                        "Lowest Note",
                        format!("{prefix}.pitch.lowestNote"),
                        i32::from(sense.lowest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "Highest Note",
                        format!("{prefix}.pitch.highestNote"),
                        i32::from(sense.highest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "Starting Note",
                        format!("{prefix}.pitch.startingNote"),
                        i32::from(sense.starting_note),
                        0,
                        127,
                        1,
                    ),
                    enum_item(
                        "Scale",
                        format!("{prefix}.pitch.scale"),
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
                        selected_index(
                            &[
                                "chromatic",
                                "major",
                                "natural_minor",
                                "dorian",
                                "mixolydian",
                                "major_pentatonic",
                                "minor_pentatonic",
                                "harmonic_minor",
                            ],
                            &sense.scale,
                        ),
                    ),
                    enum_item(
                        "Root",
                        format!("{prefix}.pitch.root"),
                        vec![
                            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                        ],
                        selected_index(
                            &[
                                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                            ],
                            &sense.root,
                        ),
                    ),
                    enum_item(
                        "Out of Range",
                        format!("{prefix}.pitch.outOfRange"),
                        vec!["clamp", "wrap"],
                        selected_index(&["clamp", "wrap"], &sense.out_of_range),
                    ),
                ],
            ),
            axis_group(
                &format!("{prefix}.x"),
                "X Axis",
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.x_pitch_enabled,
                    pitch_steps: sense.x_pitch_steps,
                    restart_each_section: sense.x_pitch_restart_each_section,
                    velocity: &sense.x_velocity,
                    filter_cutoff: &sense.x_filter_cutoff,
                    filter_resonance: &sense.x_filter_resonance,
                },
            ),
            axis_group(
                &format!("{prefix}.y"),
                "Y Axis",
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.y_pitch_enabled,
                    pitch_steps: sense.y_pitch_steps,
                    restart_each_section: sense.y_pitch_restart_each_section,
                    velocity: &sense.y_velocity,
                    filter_cutoff: &sense.y_filter_cutoff,
                    filter_resonance: &sense.y_filter_resonance,
                },
            ),
        ],
    )
}

fn param_mod_axis_group(
    part_index: usize,
    label: &str,
    axis: &str,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("parts.{part_index}.paramMods.{axis}");
    let bindings = config
        .param_mods
        .get(part_index)
        .cloned()
        .unwrap_or_default();
    let (slot1, slot2) = if axis == "x" {
        (bindings.x[0].as_ref(), bindings.x[1].as_ref())
    } else {
        (bindings.y[0].as_ref(), bindings.y[1].as_ref())
    };
    group(
        label,
        vec![
            parameter_picker_group(
                axis_binding_label("Slot 1", slot1),
                format!("param:{part_index}:{axis}:0"),
                slot1,
                config,
            ),
            bool_item(
                "Slot 1 Invert",
                format!("{prefix}.0.invert"),
                slot1.map(|binding| binding.invert).unwrap_or(false),
            ),
            parameter_picker_group(
                axis_binding_label("Slot 2", slot2),
                format!("param:{part_index}:{axis}:1"),
                slot2,
                config,
            ),
            bool_item(
                "Slot 2 Invert",
                format!("{prefix}.1.invert"),
                slot2.map(|binding| binding.invert).unwrap_or(false),
            ),
        ],
    )
}

struct AxisMenuConfig<'a> {
    offset_limit: i32,
    pitch_enabled: bool,
    pitch_steps: i32,
    restart_each_section: bool,
    velocity: &'a NativeValueLaneConfig,
    filter_cutoff: &'a NativeValueLaneConfig,
    filter_resonance: &'a NativeValueLaneConfig,
}

fn axis_group(prefix: &str, label: &str, config: AxisMenuConfig<'_>) -> NativeMenuItem {
    let mut pitch_children = vec![bool_item(
        "Enabled",
        format!("{prefix}.pitch.enabled"),
        config.pitch_enabled,
    )];
    if config.pitch_enabled {
        pitch_children.extend(vec![
            number_item(
                "Steps",
                format!("{prefix}.pitch.steps"),
                config.pitch_steps,
                -16,
                16,
                1,
            ),
            bool_item(
                "Restart Section",
                format!("{prefix}.pitch.restartEachSection"),
                config.restart_each_section,
            ),
        ]);
    }
    group(
        label,
        vec![
            group("Pitch Steps", pitch_children),
            lane_group(
                "Velocity",
                &format!("{prefix}.velocity"),
                config.velocity,
                config.offset_limit,
            ),
            lane_group(
                "Filter Cutoff",
                &format!("{prefix}.filterCutoff"),
                config.filter_cutoff,
                config.offset_limit,
            ),
            lane_group(
                "Filter Resonance",
                &format!("{prefix}.filterResonance"),
                config.filter_resonance,
                config.offset_limit,
            ),
        ],
    )
}

fn lane_group(
    label: &str,
    prefix: &str,
    lane: &NativeValueLaneConfig,
    offset_limit: i32,
) -> NativeMenuItem {
    let mut children = vec![bool_item(
        "Enabled",
        format!("{prefix}.enabled"),
        lane.enabled,
    )];
    if lane.enabled {
        children.extend(vec![
            number_item(
                "From",
                format!("{prefix}.from"),
                i32::from(lane.from),
                0,
                127,
                1,
            ),
            number_item("To", format!("{prefix}.to"), i32::from(lane.to), 0, 127, 1),
            number_item(
                "Grid Offset",
                format!("{prefix}.gridOffset"),
                lane.grid_offset,
                -offset_limit,
                offset_limit,
                1,
            ),
            enum_item(
                "Curve",
                format!("{prefix}.curve"),
                vec!["linear", "curve"],
                selected_index(&["linear", "curve"], &lane.curve),
            ),
        ]);
    }
    group(label, children)
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

struct InstrumentMenuConfig<'a> {
    index: usize,
    label: String,
    name: &'a str,
    kind: &'a str,
    auto_name: bool,
    note_behavior: &'a str,
    route: &'a str,
    volume: u8,
    pan_pos: u8,
    sample_slot: usize,
    synth_config: Option<&'a serde_json::Value>,
    synth_osc1_waveform: &'a str,
    synth_osc2_waveform: &'a str,
    synth_filter_type: &'a str,
    synth_filter_cutoff: u16,
    synth_gain_pct: u8,
    synth_filter_resonance: u8,
    sample_tune_semis: i8,
    sample_gain_pct: u8,
    sample_base_velocity: u8,
    sample_amp_velocity_sensitivity_pct: u8,
    sample_velocity_levels_enabled: bool,
    sample_velocity_high: u8,
    sample_velocity_medium: u8,
    sample_velocity_low: u8,
    sample_amp_env: Option<&'a serde_json::Value>,
    sample_filter: Option<&'a serde_json::Value>,
    sample_filter_env: Option<&'a serde_json::Value>,
    midi_enabled: bool,
    midi_channel: u8,
    midi_velocity: u8,
    midi_duration_ms: u16,
    sample_browser: Option<&'a NativeSampleBrowserConfig>,
}

fn instrument_group(config: InstrumentMenuConfig<'_>) -> NativeMenuItem {
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

fn synth_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    synth_config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.synth.{env_key}.attackMs"),
                synth_number(synth_config, &[env_key, "attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.synth.{env_key}.decayMs"),
                synth_number(synth_config, &[env_key, "decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.synth.{env_key}.sustainPct"),
                synth_number(synth_config, &[env_key, "sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.synth.{env_key}.releaseMs"),
                synth_number(synth_config, &[env_key, "releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}

fn synth_number(config: Option<&serde_json::Value>, path: &[&str], fallback: i32) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

fn sample_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.sample.{env_key}.attackMs"),
                sample_number(config, &["attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.sample.{env_key}.decayMs"),
                sample_number(config, &["decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.sample.{env_key}.sustainPct"),
                sample_number(config, &["sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.sample.{env_key}.releaseMs"),
                sample_number(config, &["releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}

fn sample_number(config: Option<&serde_json::Value>, path: &[&str], fallback: i32) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

fn sample_string(config: Option<&serde_json::Value>, path: &[&str], fallback: &str) -> String {
    let Some(mut current) = config else {
        return fallback.into();
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

fn synth_preset_items(index: usize) -> Vec<NativeMenuItem> {
    [
        ("init", "init"),
        ("soft_pad", "soft pad"),
        ("bright_pluck", "bright pluck"),
        ("bass_mono", "bass mono"),
        ("hollow_pwm", "hollow pwm"),
        ("lead", "lead"),
        ("bell", "bell"),
        ("perc_hit", "perc hit"),
    ]
    .iter()
    .map(|(id, label)| {
        action_item(
            *label,
            format!("synth.preset.{index}.{id}"),
            NativeMenuAction::PlatformEffect(format!("synth.preset:{index}:{id}")),
        )
    })
    .collect()
}

fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    sample_browser: Option<&NativeSampleBrowserConfig>,
) -> NativeMenuItem {
    let mut children = Vec::new();
    if let Some(browser) = sample_browser {
        if browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot {
            children.push(action_item(
                "..",
                format!("sample.up.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.up:{instrument_slot}:{sample_slot}"
                )),
            ));
            for entry in &browser.entries {
                let action = if entry.is_dir {
                    "sample.enter"
                } else {
                    "sample.pick"
                };
                children.push(action_item(
                    if entry.is_dir {
                        format!("[{}]", entry.name)
                    } else {
                        entry.name.clone()
                    },
                    format!("{action}.{instrument_slot}.{sample_slot}.{}", entry.path),
                    NativeMenuAction::PlatformEffect(format!(
                        "{action}:{instrument_slot}:{sample_slot}:{}",
                        entry.path
                    )),
                ));
            }
            if children.len() == 1 {
                children.push(action_item(
                    "(empty)",
                    format!("sample.open.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(format!(
                        "sample.open:{instrument_slot}:{sample_slot}:{}",
                        browser.dir
                    )),
                ));
            }
        }
    }
    NativeMenuItem {
        label: "Choose Sample".into(),
        key: Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
        value: NativeMenuValue::Group,
        children,
    }
}

fn fx_buses_group(config: &[NativeFxBusConfig]) -> NativeMenuItem {
    group(
        "FX Buses",
        (0..FX_BUS_COUNT)
            .map(|bus_index| {
                let prefix = format!("mixer.buses.{bus_index}");
                let bus = config
                    .get(bus_index)
                    .cloned()
                    .unwrap_or_else(default_fx_bus_config);
                group(
                    format!("B{}: {}", bus_index + 1, bus.name),
                    vec![
                        fx_slot_group(
                            "Slot 1",
                            &format!("{prefix}.slot1"),
                            &bus.slot1_type,
                            &bus.slot1_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        fx_slot_group(
                            "Slot 2",
                            &format!("{prefix}.slot2"),
                            &bus.slot2_type,
                            &bus.slot2_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        number_item(
                            "Pan Pos",
                            format!("{prefix}.panPos"),
                            i32::from(bus.pan_pos),
                            0,
                            32,
                            1,
                        ),
                        bool_item("Auto Name", format!("{prefix}.autoName"), bus.auto_name),
                        text_item("Name", format!("{prefix}.name"), bus.name.clone(), 32),
                    ],
                )
            })
            .collect(),
    )
}

fn global_fx_group(config: &[String], params: &[serde_json::Value]) -> NativeMenuItem {
    group(
        "Global FX",
        (0..GLOBAL_FX_SLOT_COUNT)
            .map(|slot_index| {
                let prefix = format!("mixer.master.slots.{slot_index}");
                let slot_type = config.get(slot_index).map(String::as_str).unwrap_or("none");
                let slot_params = params.get(slot_index).unwrap_or(&serde_json::Value::Null);
                group(
                    format!("Slot {}", slot_index + 1),
                    fx_slot_children(
                        &prefix,
                        slot_type,
                        slot_params,
                        GLOBAL_FX_SLOT_OPTIONS,
                        None,
                    ),
                )
            })
            .collect(),
    )
}

fn fx_slot_group(
    label: impl Into<String>,
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> NativeMenuItem {
    group(
        label,
        fx_slot_children(prefix, slot_type, params, options, bus_index),
    )
}

fn fx_slot_children(
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    let mut children = vec![enum_item(
        "Type",
        format!("{prefix}.type"),
        options.to_vec(),
        selected_index(options, slot_type),
    )];
    children.extend(fx_param_items(
        slot_type,
        &format!("{prefix}.params"),
        params,
        bus_index,
    ));
    children
}

fn fx_param_items(
    slot_type: &str,
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    match slot_type {
        "duck" => {
            let options = duck_source_options(bus_index.unwrap_or(usize::MAX));
            vec![
                enum_item_from_strings(
                    "Source",
                    format!("{prefix}.source"),
                    options.clone(),
                    options
                        .iter()
                        .position(|option| option == &fx_param_string(params, "source", "I1"))
                        .unwrap_or(0),
                ),
                fx_number_item(
                    "Threshold",
                    prefix,
                    params,
                    "threshold",
                    0,
                    100,
                    1,
                    100.0,
                    0.08,
                ),
                fx_number_item(
                    "Amount %",
                    prefix,
                    params,
                    "amountPct",
                    0,
                    100,
                    1,
                    1.0,
                    60.0,
                ),
                fx_number_item("Attack ms", prefix, params, "attackMs", 1, 500, 1, 1.0, 8.0),
                fx_number_item(
                    "Release ms",
                    prefix,
                    params,
                    "releaseMs",
                    1,
                    5000,
                    5,
                    1.0,
                    160.0,
                ),
            ]
        }
        "delay" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 35.0),
            fx_number_item("Time ms", prefix, params, "timeMs", 1, 2000, 5, 1.0, 250.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", 0, 98, 1, 100.0, 0.35,
            ),
        ],
        "tremolo" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 5, 4000, 5, 100.0, 4.0),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 60.0),
        ],
        "saturator" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 200, 1, 10.0, 1.8),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "distortion" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 500, 5, 10.0, 2.5),
            fx_number_item("Clip", prefix, params, "clip", 5, 200, 5, 100.0, 0.6),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "bitcrusher" => vec![
            fx_number_item("Bits", prefix, params, "bits", 1, 16, 1, 1.0, 6.0),
            fx_number_item("Rate Div", prefix, params, "rateDiv", 1, 128, 1, 1.0, 4.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vibrato" | "chorus" | "flanger" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.8),
            fx_number_item("Depth ms", prefix, params, "depthMs", 0, 400, 1, 10.0, 6.0),
            fx_number_item("Base ms", prefix, params, "baseMs", 1, 800, 1, 10.0, 8.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", -95, 95, 1, 100.0, 0.0,
            ),
        ],
        "filter_lfo" | "wah" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item(
                "Center Hz",
                prefix,
                params,
                "centerHz",
                40,
                12000,
                20,
                1.0,
                1600.0,
            ),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 70.0),
            fx_number_item("Q", prefix, params, "q", 25, 2000, 25, 100.0, 1.0),
        ],
        "reverb" => vec![
            fx_number_item("Decay", prefix, params, "decay", 0, 995, 5, 1000.0, 0.72),
            fx_number_item("Damp", prefix, params, "damp", 0, 98, 1, 100.0, 0.35),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 30.0),
        ],
        "auto_pan" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 100.0),
        ],
        "glitch" => vec![
            fx_number_item("Chance %", prefix, params, "chancePct", 0, 100, 1, 1.0, 8.0),
            fx_number_item("Slice ms", prefix, params, "sliceMs", 5, 500, 5, 1.0, 80.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "compressor" => vec![
            fx_number_item(
                "Threshold dB",
                prefix,
                params,
                "thresholdDb",
                -120,
                0,
                1,
                2.0,
                -24.0,
            ),
            fx_number_item("Ratio", prefix, params, "ratio", 2, 40, 1, 2.0, 4.0),
            fx_number_item(
                "Attack ms",
                prefix,
                params,
                "attackMs",
                1,
                200,
                1,
                1.0,
                10.0,
            ),
            fx_number_item(
                "Release ms",
                prefix,
                params,
                "releaseMs",
                5,
                2000,
                5,
                1.0,
                100.0,
            ),
            fx_number_item("Makeup dB", prefix, params, "makeupDb", 0, 48, 1, 2.0, 0.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "eq" => vec![
            fx_number_item(
                "Low Gain dB",
                prefix,
                params,
                "lowGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Gain dB",
                prefix,
                params,
                "midGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "High Gain dB",
                prefix,
                params,
                "highGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Freq Hz",
                prefix,
                params,
                "midFreqHz",
                40,
                8000,
                10,
                1.0,
                1000.0,
            ),
            fx_number_item("Mid Q", prefix, params, "midQ", 25, 2000, 25, 100.0, 1.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vinyl" => vec![
            fx_number_item(
                "Saturation %",
                prefix,
                params,
                "saturationPct",
                0,
                100,
                1,
                1.0,
                15.0,
            ),
            fx_number_item(
                "Crackle %",
                prefix,
                params,
                "cracklePct",
                0,
                100,
                1,
                1.0,
                8.0,
            ),
            fx_number_item(
                "Warp Depth %",
                prefix,
                params,
                "warpDepthPct",
                0,
                100,
                1,
                1.0,
                5.0,
            ),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        _ => vec![],
    }
}

#[expect(clippy::too_many_arguments, reason = "FX menu specs are data rows")]
fn fx_number_item(
    label: impl Into<String>,
    prefix: &str,
    params: &serde_json::Value,
    key: &str,
    min: i32,
    max: i32,
    step: i32,
    scale: f64,
    default: f64,
) -> NativeMenuItem {
    number_item(
        label,
        format!("{prefix}.{key}"),
        ((fx_param_number(params, key, default) * scale).round() as i32).clamp(min, max),
        min,
        max,
        step,
    )
}

fn fx_param_number(params: &serde_json::Value, key: &str, default: f64) -> f64 {
    params
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default)
}

fn fx_param_string(params: &serde_json::Value, key: &str, default: &str) -> String {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .into()
}

fn default_fx_bus_config() -> NativeFxBusConfig {
    NativeFxBusConfig {
        name: "(none)".into(),
        slot1_type: "none".into(),
        slot1_params: serde_json::json!({}),
        slot2_type: "none".into(),
        slot2_params: serde_json::json!({}),
        pan_pos: 16,
        auto_name: true,
    }
}

#[cfg(test)]
mod tests;
