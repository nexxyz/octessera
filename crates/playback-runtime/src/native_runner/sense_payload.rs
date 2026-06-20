use super::*;

pub(super) fn sense_part_payload(part: &NativeSensePart, probability_map: &[String]) -> Value {
    json!({
        "scanMode": part.scan_mode.clone(),
        "scanAxis": part.scan_axis.clone(),
        "scanUnit": part.scan_unit.clone(),
        "scanDirection": part.scan_direction.clone(),
        "scanSections": part.scan_sections,
        "eventEnabled": part.event_enabled,
        "triggerProbabilityMode": part.trigger_probability_mode.clone(),
        "triggerProbabilityLowPct": part.trigger_probability_low_pct,
        "triggerProbabilityHighPct": part.trigger_probability_high_pct,
        "stateNotesEnabled": part.state_notes_enabled,
        "triggerProbabilityMap": probability_map,
        "mapping": {
            "scanned": { "slot": slot_payload(part.scanned_slot), "action": part.scanned_action.clone() },
            "scanned_empty": { "slot": slot_payload(part.scanned_empty_slot), "action": part.scanned_empty_action.clone() },
            "activate": { "slot": slot_payload(part.activate_slot), "action": part.activate_action.clone() },
            "stable": { "slot": slot_payload(part.stable_slot), "action": part.stable_action.clone() },
            "deactivate": { "slot": slot_payload(part.deactivate_slot), "action": part.deactivate_action.clone() }
        },
        "pitch": {
            "lowestNote": part.lowest_note,
            "highestNote": part.highest_note,
            "startingNote": part.starting_note,
            "scale": part.scale.clone(),
            "root": part.root.clone(),
            "outOfRange": part.out_of_range.clone()
        },
        "x": {
            "from": part.x_from,
            "to": part.x_to,
            "pitch": {
                "enabled": part.x_pitch_enabled,
                "steps": part.x_pitch_steps,
                "restartEachSection": part.x_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.x_velocity),
            "filterCutoff": value_lane_payload(&part.x_filter_cutoff),
            "filterResonance": value_lane_payload(&part.x_filter_resonance)
        },
        "y": {
            "from": part.y_from,
            "to": part.y_to,
            "pitch": {
                "enabled": part.y_pitch_enabled,
                "steps": part.y_pitch_steps,
                "restartEachSection": part.y_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.y_velocity),
            "filterCutoff": value_lane_payload(&part.y_filter_cutoff),
            "filterResonance": value_lane_payload(&part.y_filter_resonance)
        }
    })
}

pub(super) fn value_lane_payload(lane: &NativeValueLane) -> Value {
    json!({
        "enabled": lane.enabled,
        "from": lane.from,
        "to": lane.to,
        "gridOffset": lane.grid_offset,
        "curve": lane.curve
    })
}
