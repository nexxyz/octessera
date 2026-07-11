use super::*;

pub(super) fn pulses_layer_payload(layer: &NativePulsesLayer, probability_map: &[String]) -> Value {
    json!({
        "scanMode": layer.scan_mode.clone(),
        "scanAxis": layer.scan_axis.clone(),
        "scanUnit": layer.scan_unit.clone(),
        "scanDirection": layer.scan_direction.clone(),
        "scanSections": layer.scan_sections,
        "eventEnabled": layer.event_enabled,
        "triggerProbabilityMode": layer.trigger_probability_mode.clone(),
        "triggerProbabilityLowPct": layer.trigger_probability_low_pct,
        "triggerProbabilityHighPct": layer.trigger_probability_high_pct,
        "stateNotesEnabled": layer.state_notes_enabled,
        "triggerProbabilityMap": probability_map,
        "mapping": {
            "scanned": { "slot": slot_payload(layer.scanned_slot), "action": layer.scanned_action.clone() },
            "scanned_empty": { "slot": slot_payload(layer.scanned_empty_slot), "action": layer.scanned_empty_action.clone() },
            "activate": { "slot": slot_payload(layer.activate_slot), "action": layer.activate_action.clone() },
            "stable": { "slot": slot_payload(layer.stable_slot), "action": layer.stable_action.clone() },
            "deactivate": { "slot": slot_payload(layer.deactivate_slot), "action": layer.deactivate_action.clone() }
        },
        "pitch": {
            "lowestNote": layer.lowest_note,
            "highestNote": layer.highest_note,
            "startingNote": layer.starting_note,
            "scale": layer.scale.clone(),
            "root": layer.root.clone(),
            "outOfRange": layer.out_of_range.clone()
        },
        "x": {
            "from": layer.x_from,
            "to": layer.x_to,
            "pitch": {
                "enabled": layer.x_pitch_enabled,
                "steps": layer.x_pitch_steps,
                "restartEachSection": layer.x_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&layer.x_velocity),
            "filterCutoff": value_lane_payload(&layer.x_filter_cutoff),
            "filterResonance": value_lane_payload(&layer.x_filter_resonance)
        },
        "y": {
            "from": layer.y_from,
            "to": layer.y_to,
            "pitch": {
                "enabled": layer.y_pitch_enabled,
                "steps": layer.y_pitch_steps,
                "restartEachSection": layer.y_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&layer.y_velocity),
            "filterCutoff": value_lane_payload(&layer.y_filter_cutoff),
            "filterResonance": value_lane_payload(&layer.y_filter_resonance)
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
