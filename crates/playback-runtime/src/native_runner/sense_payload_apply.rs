use super::payload_assign::{
    apply_value_lane_payload, assign_bool, assign_i32, assign_mapping, assign_string, assign_u8,
};
use super::{NativeSensePart, Value};

pub(super) fn apply_sense_payload(part: &mut NativeSensePart, payload: &Value) {
    assign_string(payload, "scanMode", &mut part.scan_mode);
    assign_string(payload, "scanAxis", &mut part.scan_axis);
    assign_string(payload, "scanUnit", &mut part.scan_unit);
    assign_string(payload, "scanDirection", &mut part.scan_direction);
    assign_u8(payload, "scanSections", &mut part.scan_sections, 8);
    if let Some(enabled) = payload.get("eventEnabled").and_then(Value::as_bool) {
        part.event_enabled = enabled;
    }
    if let Some(enabled) = payload.get("stateNotesEnabled").and_then(Value::as_bool) {
        part.state_notes_enabled = enabled;
    }
    assign_string(
        payload,
        "triggerProbabilityMode",
        &mut part.trigger_probability_mode,
    );
    assign_u8(
        payload,
        "triggerProbabilityLowPct",
        &mut part.trigger_probability_low_pct,
        100,
    );
    assign_u8(
        payload,
        "triggerProbabilityHighPct",
        &mut part.trigger_probability_high_pct,
        100,
    );
    if let Some(mapping) = payload.get("mapping") {
        assign_mapping(
            mapping,
            "scanned",
            &mut part.scanned_slot,
            &mut part.scanned_action,
        );
        assign_mapping(
            mapping,
            "scanned_empty",
            &mut part.scanned_empty_slot,
            &mut part.scanned_empty_action,
        );
        assign_mapping(
            mapping,
            "activate",
            &mut part.activate_slot,
            &mut part.activate_action,
        );
        assign_mapping(
            mapping,
            "stable",
            &mut part.stable_slot,
            &mut part.stable_action,
        );
        assign_mapping(
            mapping,
            "deactivate",
            &mut part.deactivate_slot,
            &mut part.deactivate_action,
        );
    }
    if let Some(pitch) = payload.get("pitch") {
        assign_u8(pitch, "lowestNote", &mut part.lowest_note, 127);
        assign_u8(pitch, "highestNote", &mut part.highest_note, 127);
        assign_u8(pitch, "startingNote", &mut part.starting_note, 127);
        assign_string(pitch, "scale", &mut part.scale);
        assign_string(pitch, "root", &mut part.root);
        assign_string(pitch, "outOfRange", &mut part.out_of_range);
    }
    if let Some(x) = payload.get("x") {
        assign_u8(x, "from", &mut part.x_from, 7);
        assign_u8(x, "to", &mut part.x_to, 7);
        if let Some(pitch) = x.get("pitch") {
            assign_bool(pitch, "enabled", &mut part.x_pitch_enabled);
            assign_i32(pitch, "steps", &mut part.x_pitch_steps, -16, 16);
            assign_bool(
                pitch,
                "restartEachSection",
                &mut part.x_pitch_restart_each_section,
            );
        }
        if let Some(lane) = x.get("velocity") {
            apply_value_lane_payload(&mut part.x_velocity, lane);
        }
        if let Some(lane) = x.get("filterCutoff") {
            apply_value_lane_payload(&mut part.x_filter_cutoff, lane);
        }
        if let Some(lane) = x.get("filterResonance") {
            apply_value_lane_payload(&mut part.x_filter_resonance, lane);
        }
    }
    if let Some(y) = payload.get("y") {
        assign_u8(y, "from", &mut part.y_from, 7);
        assign_u8(y, "to", &mut part.y_to, 7);
        if let Some(pitch) = y.get("pitch") {
            assign_bool(pitch, "enabled", &mut part.y_pitch_enabled);
            assign_i32(pitch, "steps", &mut part.y_pitch_steps, -16, 16);
            assign_bool(
                pitch,
                "restartEachSection",
                &mut part.y_pitch_restart_each_section,
            );
        }
        if let Some(lane) = y.get("velocity") {
            apply_value_lane_payload(&mut part.y_velocity, lane);
        }
        if let Some(lane) = y.get("filterCutoff") {
            apply_value_lane_payload(&mut part.y_filter_cutoff, lane);
        }
        if let Some(lane) = y.get("filterResonance") {
            apply_value_lane_payload(&mut part.y_filter_resonance, lane);
        }
    }
}
