use super::payload_assign::{
    apply_value_lane_payload, assign_bool, assign_i32, assign_mapping, assign_string, assign_u8,
};
use super::{LinkEventTiming, NativePulsesLayer, Value};

pub(super) fn apply_pulses_payload(layer: &mut NativePulsesLayer, payload: &Value) {
    apply_scan_and_trigger_payload(layer, payload);
    apply_mapping_payload(layer, payload);
    apply_pitch_payload(layer, payload);
    apply_axis_payload(layer, payload, "x");
    apply_axis_payload(layer, payload, "y");
}

fn apply_scan_and_trigger_payload(layer: &mut NativePulsesLayer, payload: &Value) {
    assign_scan_mode(payload, &mut layer.scan_mode);
    assign_string(payload, "scanAxis", &mut layer.scan_axis);
    assign_string(payload, "scanUnit", &mut layer.scan_unit);
    assign_string(payload, "scanDirection", &mut layer.scan_direction);
    assign_u8(payload, "scanSections", &mut layer.scan_sections, 8);
    if let Some(enabled) = payload.get("eventEnabled").and_then(Value::as_bool) {
        layer.event_enabled = enabled;
    }
    if let Some(enabled) = payload.get("stateNotesEnabled").and_then(Value::as_bool) {
        layer.state_notes_enabled = enabled;
    }
    assign_string(
        payload,
        "triggerProbabilityMode",
        &mut layer.trigger_probability_mode,
    );
    assign_u8(
        payload,
        "triggerProbabilityLowPct",
        &mut layer.trigger_probability_low_pct,
        100,
    );
    assign_u8(
        payload,
        "triggerProbabilityHighPct",
        &mut layer.trigger_probability_high_pct,
        100,
    );
}

fn assign_scan_mode(payload: &Value, target: &mut String) {
    if let Some(value) = payload.get("scanMode").and_then(Value::as_str) {
        *target = if value == "immediate" { "none" } else { value }.into();
    }
}

fn apply_mapping_payload(layer: &mut NativePulsesLayer, payload: &Value) {
    let Some(mapping) = payload.get("mapping") else {
        return;
    };
    assign_mapping(
        mapping,
        "scanned",
        &mut layer.scanned_slot,
        &mut layer.scanned_action,
    );
    assign_timing(mapping, "scanned", &mut layer.scanned_timing);
    assign_mapping(
        mapping,
        "scanned_empty",
        &mut layer.scanned_empty_slot,
        &mut layer.scanned_empty_action,
    );
    assign_timing(mapping, "scanned_empty", &mut layer.scanned_empty_timing);
    assign_mapping(
        mapping,
        "activate",
        &mut layer.activate_slot,
        &mut layer.activate_action,
    );
    assign_timing(mapping, "activate", &mut layer.activate_timing);
    assign_mapping(
        mapping,
        "stable",
        &mut layer.stable_slot,
        &mut layer.stable_action,
    );
    assign_timing(mapping, "stable", &mut layer.stable_timing);
    assign_mapping(
        mapping,
        "deactivate",
        &mut layer.deactivate_slot,
        &mut layer.deactivate_action,
    );
    assign_timing(mapping, "deactivate", &mut layer.deactivate_timing);
}

fn assign_timing(mapping: &Value, key: &str, timing: &mut LinkEventTiming) {
    let Some(payload) = mapping.get(key) else {
        return;
    };
    if let Some(delay) = payload.get("delaySteps").and_then(Value::as_u64) {
        timing.delay_steps = (delay as u8).min(16);
    }
    if let Some(retrigger) = payload.get("retriggerCount").and_then(Value::as_u64) {
        timing.retrigger_count = (retrigger as u8).min(8);
    }
}

fn apply_pitch_payload(layer: &mut NativePulsesLayer, payload: &Value) {
    let Some(pitch) = payload.get("pitch") else {
        return;
    };
    assign_u8(pitch, "lowestNote", &mut layer.lowest_note, 127);
    assign_u8(pitch, "highestNote", &mut layer.highest_note, 127);
    assign_u8(pitch, "startingNote", &mut layer.starting_note, 127);
    assign_string(pitch, "scale", &mut layer.scale);
    assign_string(pitch, "root", &mut layer.root);
    assign_string(pitch, "outOfRange", &mut layer.out_of_range);
}

fn apply_axis_payload(layer: &mut NativePulsesLayer, payload: &Value, axis: &str) {
    let Some(axis_payload) = payload.get(axis) else {
        return;
    };
    let (
        from,
        to,
        pitch_enabled,
        pitch_steps,
        pitch_restart_each_section,
        velocity,
        filter_cutoff,
        filter_resonance,
    ) = if axis == "x" {
        (
            &mut layer.x_from,
            &mut layer.x_to,
            &mut layer.x_pitch_enabled,
            &mut layer.x_pitch_steps,
            &mut layer.x_pitch_restart_each_section,
            &mut layer.x_velocity,
            &mut layer.x_filter_cutoff,
            &mut layer.x_filter_resonance,
        )
    } else {
        (
            &mut layer.y_from,
            &mut layer.y_to,
            &mut layer.y_pitch_enabled,
            &mut layer.y_pitch_steps,
            &mut layer.y_pitch_restart_each_section,
            &mut layer.y_velocity,
            &mut layer.y_filter_cutoff,
            &mut layer.y_filter_resonance,
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
