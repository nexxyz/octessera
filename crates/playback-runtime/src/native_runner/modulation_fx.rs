use super::modulation_value::{
    apply_fx_param_binding_value, apply_fx_slot_type_value, apply_u8_value,
};
use super::{derive_bus_name, fx_default_params, json, Value, PAN_POSITION_COUNT};

pub(super) fn apply_fx_bus_binding_value(
    bus: &mut super::NativeFxBus,
    slot: &str,
    field: &str,
    value: Value,
) -> bool {
    let changed = match (slot, field) {
        ("bus", "panPos") => apply_u8_value(&mut bus.pan_pos, value, PAN_POSITION_COUNT - 1),
        ("bus", "volume") => apply_u8_value(&mut bus.volume_pct, value, 100),
        (slot, field) => {
            let Some((slot_type, params)) = slot
                .strip_prefix("slot")
                .and_then(super::NativeFxBus::slot_index)
                .and_then(|index| bus.slot_mut(index))
            else {
                return false;
            };
            if field == "type" {
                apply_fx_slot_type_value(slot_type, params, value)
            } else if let Some(param) = field.strip_prefix("params.") {
                apply_fx_param_binding_value(params, param, value)
            } else {
                false
            }
        }
    };
    if changed {
        if bus.auto_name {
            bus.name = derive_bus_name(bus);
        }
        return true;
    }
    false
}

pub(super) fn apply_global_fx_binding_value(
    slots: &mut [String],
    params: &mut [Value],
    index: usize,
    field: &str,
    value: Value,
) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    let Some(slot_params) = params.get_mut(index) else {
        return false;
    };
    match field {
        "type" => apply_fx_slot_type_value(slot, slot_params, value),
        field if field.starts_with("params.") => {
            apply_fx_param_binding_value(slot_params, &field[7..], value)
        }
        _ => false,
    }
}

pub(super) fn apply_play_fx_binding_value(selected: &mut Value, field: &str, value: Value) -> bool {
    let mut object = selected.as_object().cloned().unwrap_or_default();
    let changed = match field {
        "type" => {
            let Some(value) = value.as_str() else {
                return false;
            };
            let changed = object
                .get("fxType")
                .and_then(Value::as_str)
                .unwrap_or("none")
                != value;
            if changed {
                object.insert("fxType".into(), json!(value));
                object.insert("params".into(), fx_default_params(value));
            }
            changed
        }
        "target" => {
            let Some(value) = value.as_str() else {
                return false;
            };
            let changed = object
                .get("targetKey")
                .and_then(Value::as_str)
                .unwrap_or("master")
                != value;
            if changed {
                object.insert("targetKey".into(), json!(value));
            }
            changed
        }
        field if field.starts_with("params.") => {
            let mut params = object
                .get("params")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let key = &field[7..];
            let changed = params.get(key) != Some(&value);
            if changed {
                params.insert(key.into(), value);
                object.insert("params".into(), Value::Object(params));
            }
            changed
        }
        _ => false,
    };
    if changed {
        *selected = Value::Object(object);
        return true;
    }
    false
}
