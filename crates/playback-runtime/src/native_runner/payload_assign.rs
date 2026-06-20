use super::{parse_slot_index, NativeValueLane, Value, INSTRUMENT_COUNT};

pub(super) fn assign_string(payload: &Value, key: &str, target: &mut String) {
    if let Some(value) = payload.get(key).and_then(Value::as_str) {
        *target = value.into();
    }
}

pub(super) fn assign_u8(payload: &Value, key: &str, target: &mut u8, max: u8) {
    if let Some(value) = payload.get(key).and_then(Value::as_u64) {
        *target = (value as u8).min(max);
    }
}

pub(super) fn assign_bool(payload: &Value, key: &str, target: &mut bool) {
    if let Some(value) = payload.get(key).and_then(Value::as_bool) {
        *target = value;
    }
}

pub(super) fn assign_i32(payload: &Value, key: &str, target: &mut i32, min: i32, max: i32) {
    if let Some(value) = payload.get(key).and_then(Value::as_i64) {
        *target = (value as i32).clamp(min, max);
    }
}

pub(super) fn assign_mapping(payload: &Value, key: &str, slot: &mut usize, action: &mut String) {
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

pub(super) fn apply_value_lane_payload(target: &mut NativeValueLane, payload: &Value) {
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
