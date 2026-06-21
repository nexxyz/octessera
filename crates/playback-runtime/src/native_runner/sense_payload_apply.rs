use super::payload_assign::{
    apply_value_lane_payload, assign_bool, assign_i32, assign_mapping, assign_string, assign_u8,
};
use super::{NativeSensePart, Value};

pub(super) fn apply_sense_payload(part: &mut NativeSensePart, payload: &Value) {
    apply_scan_and_trigger_payload(part, payload);
    apply_mapping_payload(part, payload);
    apply_pitch_payload(part, payload);
    apply_axis_payload(part, payload, "x");
    apply_axis_payload(part, payload, "y");
}

fn apply_scan_and_trigger_payload(part: &mut NativeSensePart, payload: &Value) {
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
}

fn apply_mapping_payload(part: &mut NativeSensePart, payload: &Value) {
    let Some(mapping) = payload.get("mapping") else {
        return;
    };
    assign_mapping(mapping, "scanned", &mut part.scanned_slot, &mut part.scanned_action);
    assign_mapping(
        mapping,
        "scanned_empty",
        &mut part.scanned_empty_slot,
        &mut part.scanned_empty_action,
    );
    assign_mapping(mapping, "activate", &mut part.activate_slot, &mut part.activate_action);
    assign_mapping(mapping, "stable", &mut part.stable_slot, &mut part.stable_action);
    assign_mapping(
        mapping,
        "deactivate",
        &mut part.deactivate_slot,
        &mut part.deactivate_action,
    );
}

fn apply_pitch_payload(part: &mut NativeSensePart, payload: &Value) {
    let Some(pitch) = payload.get("pitch") else {
        return;
    };
    assign_u8(pitch, "lowestNote", &mut part.lowest_note, 127);
    assign_u8(pitch, "highestNote", &mut part.highest_note, 127);
    assign_u8(pitch, "startingNote", &mut part.starting_note, 127);
    assign_string(pitch, "scale", &mut part.scale);
    assign_string(pitch, "root", &mut part.root);
    assign_string(pitch, "outOfRange", &mut part.out_of_range);
}

fn apply_axis_payload(part: &mut NativeSensePart, payload: &Value, axis: &str) {
    let Some(axis_payload) = payload.get(axis) else {
        return;
    };
    let (from, to, pitch_enabled, pitch_steps, pitch_restart_each_section, velocity, filter_cutoff, filter_resonance) =
        if axis == "x" {
            (
                &mut part.x_from,
                &mut part.x_to,
                &mut part.x_pitch_enabled,
                &mut part.x_pitch_steps,
                &mut part.x_pitch_restart_each_section,
                &mut part.x_velocity,
                &mut part.x_filter_cutoff,
                &mut part.x_filter_resonance,
            )
        } else {
            (
                &mut part.y_from,
                &mut part.y_to,
                &mut part.y_pitch_enabled,
                &mut part.y_pitch_steps,
                &mut part.y_pitch_restart_each_section,
                &mut part.y_velocity,
                &mut part.y_filter_cutoff,
                &mut part.y_filter_resonance,
            )
        };
    assign_u8(axis_payload, "from", from, 7);
    assign_u8(axis_payload, "to", to, 7);
    if let Some(pitch) = axis_payload.get("pitch") {
        assign_bool(pitch, "enabled", pitch_enabled);
        assign_i32(pitch, "steps", pitch_steps, -16, 16);
        assign_bool(pitch, "restartEachSection", pitch_restart_each_section);
    }
    if let Some(lane) = axis_payload.get("velocity") {
        apply_value_lane_payload(velocity, lane);
    }
    if let Some(lane) = axis_payload.get("filterCutoff") {
        apply_value_lane_payload(filter_cutoff, lane);
    }
    if let Some(lane) = axis_payload.get("filterResonance") {
        apply_value_lane_payload(filter_resonance, lane);
    }
}
