use crate::native_menu::NativeMenuAction;

use super::{
    parse_slot_index, supported_aux_turn_key, NativeAuxBinding, NativeSensePart, NativeValueLane,
    Value, INSTRUMENT_COUNT,
};

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

fn apply_value_lane_payload(target: &mut NativeValueLane, payload: &Value) {
    assign_bool(payload, "enabled", &mut target.enabled);
    assign_u8(payload, "from", &mut target.from, 127);
    assign_u8(payload, "to", &mut target.to, 127);
    assign_i32(payload, "gridOffset", &mut target.grid_offset, -7, 7);
    if let Some(curve) = payload.get("curve").and_then(Value::as_str) {
        if matches!(curve, "linear" | "curve") {
            target.curve = curve.into();
        }
    }
}

fn assign_string(payload: &Value, key: &str, target: &mut String) {
    if let Some(value) = payload.get(key).and_then(Value::as_str) {
        *target = value.into();
    }
}

fn assign_u8(payload: &Value, key: &str, target: &mut u8, max: u8) {
    if let Some(value) = payload.get(key).and_then(Value::as_u64) {
        *target = (value as u8).min(max);
    }
}

fn assign_bool(payload: &Value, key: &str, target: &mut bool) {
    if let Some(value) = payload.get(key).and_then(Value::as_bool) {
        *target = value;
    }
}

fn assign_i32(payload: &Value, key: &str, target: &mut i32, min: i32, max: i32) {
    if let Some(value) = payload.get(key).and_then(Value::as_i64) {
        *target = (value as i32).clamp(min, max);
    }
}

fn assign_mapping(payload: &Value, key: &str, slot: &mut usize, action: &mut String) {
    let Some(mapping) = payload.get(key) else {
        return;
    };
    if let Some(value) = mapping.get("slot") {
        if value.as_str() == Some("none") {
            *slot = usize::MAX;
        } else if let Some(parsed) = value
            .as_str()
            .and_then(parse_slot_index)
            .or_else(|| value.as_u64().map(|value| value as usize))
        {
            *slot = parsed.min(INSTRUMENT_COUNT - 1);
        }
    }
    if let Some(value) = mapping.get("action").and_then(Value::as_str) {
        *action = value.into();
    }
}

pub(super) fn apply_aux_bindings_payload(
    bindings: &mut [Option<NativeAuxBinding>],
    payload: &Value,
) {
    for (index, binding) in bindings.iter_mut().enumerate() {
        let key = format!("aux{}", index + 1);
        let Some(value) = payload.get(&key) else {
            continue;
        };
        if value.is_null() {
            *binding = None;
            continue;
        }
        let turn_key = value
            .get("turnKey")
            .and_then(Value::as_str)
            .filter(|key| supported_aux_turn_key(key))
            .map(str::to_string);
        let press_action = value.get("pressAction").and_then(parse_aux_press_action);
        *binding = if turn_key.is_some() || press_action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action,
            })
        } else {
            None
        };
    }
}

fn parse_aux_press_action(value: &Value) -> Option<NativeMenuAction> {
    match value.get("kind").and_then(Value::as_str)? {
        "behavior_action" => value
            .get("actionType")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::BehaviorAction(action.into())),
        "platform_effect" => value
            .get("action")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::PlatformEffect(action.into())),
        "instrument_clone" => value.get("slot").and_then(Value::as_u64).map(|slot| {
            NativeMenuAction::CloneInstrument {
                index: slot as usize,
            }
        }),
        "instrument_reset" => value.get("slot").and_then(Value::as_u64).map(|slot| {
            NativeMenuAction::ResetInstrument {
                index: slot as usize,
            }
        }),
        "reset_behavior" => Some(NativeMenuAction::ResetBehavior),
        _ => None,
    }
}
