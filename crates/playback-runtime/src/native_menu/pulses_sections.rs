use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    slot_option_selected, NativeMenuAction, NativeMenuItem, NativePulsesLayerConfig,
};

pub(super) fn scanning_group(
    prefix: &str,
    sense: &NativePulsesLayerConfig,
    instrument_options: &[String],
) -> NativeMenuItem {
    let mut children = vec![enum_item(
        "Scan Mode",
        format!("{prefix}.scanMode"),
        vec!["none", "scanning"],
        selected_index(&["none", "scanning"], &sense.scan_mode),
    )];
    if sense.scan_mode == "scanning" {
        children.extend(vec![
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
                instrument_options.to_vec(),
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
                instrument_options.to_vec(),
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
    group("Scanning", children)
}

pub(super) fn events_group(
    prefix: &str,
    sense: &NativePulsesLayerConfig,
    instrument_options: &[String],
) -> NativeMenuItem {
    let mut children = vec![
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
    ];
    children.extend(event_mapping_item(
        prefix,
        "Activate",
        "activate",
        sense.activate_slot,
        &sense.activate_action,
        instrument_options,
    ));
    children.extend(event_mapping_item(
        prefix,
        "Stable",
        "stable",
        sense.stable_slot,
        &sense.stable_action,
        instrument_options,
    ));
    children.extend(event_mapping_item(
        prefix,
        "Deactivate",
        "deactivate",
        sense.deactivate_slot,
        &sense.deactivate_action,
        instrument_options,
    ));
    group("Events", children)
}

pub(super) fn trigger_probability_group(
    index: usize,
    prefix: &str,
    sense: &NativePulsesLayerConfig,
) -> NativeMenuItem {
    group(
        "Trigger Prob.",
        vec![
            enum_item(
                "Mode",
                format!("{prefix}.triggerProbabilityMode"),
                vec!["zero", "custom", "full"],
                selected_index(&["zero", "custom", "full"], &sense.trigger_probability_mode),
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
                NativeMenuAction::PlatformEffect(format!("trigger.probability.assign:{index}")),
            ),
        ],
    )
}

pub(super) fn note_mapping_group(prefix: &str, sense: &NativePulsesLayerConfig) -> NativeMenuItem {
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
    )
}

fn event_mapping_item(
    prefix: &str,
    label: &str,
    key: &str,
    slot: usize,
    action: &str,
    instrument_options: &[String],
) -> Vec<NativeMenuItem> {
    vec![
        enum_item_from_strings(
            format!("{label} Instrument"),
            format!("{prefix}.mapping.{key}.slot"),
            instrument_options.to_vec(),
            slot_option_selected(slot, instrument_options.len()),
        ),
        enum_item(
            format!("{label} Action"),
            format!("{prefix}.mapping.{key}.action"),
            vec!["none", "note_on", "note_off"],
            selected_index(&["none", "note_on", "note_off"], action),
        ),
    ]
}
