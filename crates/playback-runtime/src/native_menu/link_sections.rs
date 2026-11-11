use super::{
    action_item, bool_item, enum_item, enum_item_from_strings, group, number_item,
    parameter_picker_group_numeric, selected_index, slot_option_selected, wire_enum_item,
    NativeLinkArpConfig, NativeLinkLayerConfig, NativeLinkLfoConfig, NativeMenuAction,
    NativeMenuConfig, NativeMenuItem,
};
use crate::native_runner::{
    ArpMode, ArpSource, ScanAxis, ScanDirection, ScanMode, TriggerProbabilityMode,
};
use platform_core::TriggerAction;

pub(super) fn arp_group(prefix: &str, arp: &NativeLinkArpConfig) -> NativeMenuItem {
    group(
        "Arp",
        vec![
            wire_enum_item(
                "Mode",
                format!("{prefix}.mode"),
                ArpMode::ALL,
                ArpMode::wire_name,
                arp.mode,
            ),
            wire_enum_item(
                "Source",
                format!("{prefix}.source"),
                ArpSource::ALL,
                ArpSource::wire_name,
                arp.source,
            ),
            number_item(
                "Step",
                format!("{prefix}.stepIntervalSteps"),
                i32::from(arp.step_interval_steps),
                1,
                16,
                1,
            ),
            number_item(
                "Length",
                format!("{prefix}.noteLengthMs"),
                i32::from(arp.note_length_ms),
                10,
                2000,
                10,
            ),
            number_item(
                "Gate",
                format!("{prefix}.gatePct"),
                i32::from(arp.gate_pct),
                1,
                100,
                1,
            ),
            number_item(
                "Octaves",
                format!("{prefix}.octaveSpread"),
                i32::from(arp.octave_spread),
                0,
                3,
                1,
            ),
        ],
    )
}

pub(super) fn link_lfo_group(
    label: String,
    prefix: &str,
    lfo: &NativeLinkLfoConfig,
) -> NativeMenuItem {
    group(
        label,
        vec![
            bool_item("Enabled", format!("{prefix}.enabled"), lfo.enabled),
            parameter_picker_group_numeric(
                "Target",
                format!("{prefix}.target"),
                lfo.target.as_ref(),
            ),
            enum_item(
                "Period",
                format!("{prefix}.period"),
                crate::timing_units::LFO_PERIOD_OPTIONS.to_vec(),
                crate::timing_units::LFO_PERIOD_OPTIONS
                    .iter()
                    .position(|period| *period == lfo.period)
                    .unwrap_or_else(|| crate::timing_units::note_unit_selection_index(&lfo.period)),
            ),
            number_item(
                "Depth",
                format!("{prefix}.depthPct"),
                i32::from(lfo.depth_pct),
                0,
                100,
                1,
            ),
        ],
    )
}

pub(super) fn global_link_lfos_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "LFOs",
        config
            .link_lfos
            .iter()
            .enumerate()
            .map(|(index, lfo)| {
                link_lfo_group(
                    format!("LFO{}", index + 1),
                    &format!("linkLfos.{index}"),
                    lfo,
                )
            })
            .collect(),
    )
}

pub(super) fn scanning_group(
    prefix: &str,
    sense: &NativeLinkLayerConfig,
    instrument_options: &[String],
) -> NativeMenuItem {
    let mut children = vec![wire_enum_item(
        "Scan Mode",
        format!("{prefix}.scanMode"),
        ScanMode::ALL,
        ScanMode::wire_name,
        sense.scan_mode,
    )];
    if sense.scan_mode == ScanMode::Scanning {
        children.extend(vec![
            wire_enum_item(
                "Scan Axis",
                format!("{prefix}.scanAxis"),
                ScanAxis::ALL,
                ScanAxis::wire_name,
                sense.scan_axis,
            ),
            enum_item(
                "Scan Unit",
                format!("{prefix}.scanUnit"),
                crate::timing_units::NOTE_UNIT_OPTIONS.to_vec(),
                crate::timing_units::note_unit_selection_index(&sense.scan_unit),
            ),
            wire_enum_item(
                "Scan Dir",
                format!("{prefix}.scanDirection"),
                ScanDirection::ALL,
                ScanDirection::wire_name,
                sense.scan_direction,
            ),
            enum_item(
                "Sections",
                format!("{prefix}.scanSections"),
                vec!["1", "2", "4", "8"],
                selected_index(&["1", "2", "4", "8"], &sense.scan_sections.to_string()),
            ),
            enum_item_from_strings(
                "Instr.",
                format!("{prefix}.mapping.scanned.slot"),
                instrument_options.to_vec(),
                slot_option_selected(sense.scanned_slot, instrument_options.len()),
            ),
            wire_enum_item(
                "Action",
                format!("{prefix}.mapping.scanned.action"),
                TriggerAction::ALL,
                TriggerAction::wire_name,
                sense.scanned_action,
            ),
            timing_item(
                "Scan Delay",
                format!("{prefix}.mapping.scanned.delaySteps"),
                sense.scanned_timing.delay_steps,
                16,
            ),
            timing_item(
                "Scan Retrig",
                format!("{prefix}.mapping.scanned.retriggerCount"),
                sense.scanned_timing.retrigger_count,
                8,
            ),
            enum_item_from_strings(
                "Empty Inst",
                format!("{prefix}.mapping.scanned_empty.slot"),
                instrument_options.to_vec(),
                slot_option_selected(sense.scanned_empty_slot, instrument_options.len()),
            ),
            wire_enum_item(
                "Empty Act",
                format!("{prefix}.mapping.scanned_empty.action"),
                TriggerAction::ALL,
                TriggerAction::wire_name,
                sense.scanned_empty_action,
            ),
            timing_item(
                "Empty Delay",
                format!("{prefix}.mapping.scanned_empty.delaySteps"),
                sense.scanned_empty_timing.delay_steps,
                16,
            ),
            timing_item(
                "Empty Retrig",
                format!("{prefix}.mapping.scanned_empty.retriggerCount"),
                sense.scanned_empty_timing.retrigger_count,
                8,
            ),
        ]);
    }
    group("Scanning", children)
}

pub(super) fn events_group(
    prefix: &str,
    sense: &NativeLinkLayerConfig,
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
        sense.activate_action,
        sense.activate_timing,
        instrument_options,
    ));
    children.extend(event_mapping_item(
        prefix,
        "Stable",
        "stable",
        sense.stable_slot,
        sense.stable_action,
        sense.stable_timing,
        instrument_options,
    ));
    children.extend(event_mapping_item(
        prefix,
        "Deactivate",
        "deactivate",
        sense.deactivate_slot,
        sense.deactivate_action,
        sense.deactivate_timing,
        instrument_options,
    ));
    group("Events", children)
}

pub(super) fn trigger_probability_group(
    index: usize,
    prefix: &str,
    sense: &NativeLinkLayerConfig,
) -> NativeMenuItem {
    group(
        "Trigger Prob.",
        vec![
            wire_enum_item(
                "Mode",
                format!("{prefix}.triggerProbabilityMode"),
                TriggerProbabilityMode::ALL,
                TriggerProbabilityMode::wire_name,
                sense.trigger_probability_mode,
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
            bool_item(
                "Prob Note Off",
                format!("{prefix}.triggerProbabilityNoteOffs"),
                sense.trigger_probability_note_offs,
            ),
            action_item(
                "Map Prob Grid",
                format!("{prefix}.triggerProbability.map"),
                NativeMenuAction::PlatformEffect(format!("trigger.probability.assign:{index}")),
            ),
        ],
    )
}

fn event_mapping_item(
    prefix: &str,
    label: &str,
    key: &str,
    slot: usize,
    action: TriggerAction,
    timing: super::LinkEventTimingConfig,
    instrument_options: &[String],
) -> Vec<NativeMenuItem> {
    let short_label = match label {
        "Activate" => "On",
        "Stable" => "Hold",
        "Deactivate" => "Off",
        _ => label,
    };
    vec![
        enum_item_from_strings(
            format!("{short_label} Inst"),
            format!("{prefix}.mapping.{key}.slot"),
            instrument_options.to_vec(),
            slot_option_selected(slot, instrument_options.len()),
        ),
        wire_enum_item(
            format!("{short_label} Trig"),
            format!("{prefix}.mapping.{key}.action"),
            TriggerAction::ALL,
            TriggerAction::wire_name,
            action,
        ),
        timing_item(
            format!("{short_label} Delay"),
            format!("{prefix}.mapping.{key}.delaySteps"),
            timing.delay_steps,
            16,
        ),
        timing_item(
            format!("{short_label} Retrig"),
            format!("{prefix}.mapping.{key}.retriggerCount"),
            timing.retrigger_count,
            8,
        ),
    ]
}

fn timing_item(label: impl Into<String>, key: String, value: u8, max: i32) -> NativeMenuItem {
    number_item(label, key, i32::from(value), 0, max, 1)
}
