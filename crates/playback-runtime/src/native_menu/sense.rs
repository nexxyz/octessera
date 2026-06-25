use super::sense_axis::{axis_group, AxisMenuConfig};
use super::{
    action_item, axis_binding_label, bool_item, enum_item, enum_item_from_strings, group,
    number_item, parameter_picker_group, selected_index, slot_option_selected, NativeMenuAction,
    NativeMenuConfig, NativeMenuItem, NativeSensePartConfig, NativeValueLaneConfig,
};
pub(super) fn default_sense_part_config() -> NativeSensePartConfig {
    NativeSensePartConfig {
        scan_mode: "none".into(),
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

pub(super) fn l2_part_group(
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
        vec!["none", "scanning"],
        selected_index(&["none", "scanning"], &sense.scan_mode),
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
                        "Prob Low",
                        format!("{prefix}.triggerProbabilityLowPct"),
                        i32::from(sense.trigger_probability_low_pct),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Prob High",
                        format!("{prefix}.triggerProbabilityHighPct"),
                        i32::from(sense.trigger_probability_high_pct),
                        0,
                        100,
                        1,
                    ),
                    action_item(
                        "Map Prob Grid",
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
                        "Low Note",
                        format!("{prefix}.pitch.lowestNote"),
                        i32::from(sense.lowest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "High Note",
                        format!("{prefix}.pitch.highestNote"),
                        i32::from(sense.highest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "Start Note",
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

pub(super) fn l2_root_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        super::system::aux_mappings_group(config),
        bool_item(
            "Events when paused",
            "inputEventsWhilePaused",
            config.input_events_while_paused,
        ),
    ]
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
