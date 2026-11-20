use super::super::play_fx_config::{is_play_fx_target, play_fx_types, PlayFxConfig};
use super::super::{derive_bus_name, Value, PAN_POSITION_COUNT};
use super::value::{apply_fx_param_binding_value, apply_fx_slot_type_value, apply_u8_value};
use crate::native_menu::{is_valid_fx_bus_slot_type, is_valid_global_fx_slot_type};

pub(super) fn apply_fx_bus_binding_value(
    bus: &mut super::super::NativeFxBus,
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
                .and_then(super::super::NativeFxBus::slot_index)
                .and_then(|index| bus.slot_mut(index))
            else {
                return false;
            };
            if field == "type" {
                apply_fx_slot_type_value(slot_type, params, value, is_valid_fx_bus_slot_type)
            } else if let Some(param) = field.strip_prefix("params.") {
                apply_fx_param_binding_value(slot_type, params, param, value)
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
        "type" => apply_fx_slot_type_value(slot, slot_params, value, is_valid_global_fx_slot_type),
        field if field.starts_with("params.") => {
            apply_fx_param_binding_value(slot, slot_params, &field[7..], value)
        }
        _ => false,
    }
}

pub(super) fn apply_play_fx_binding_value(
    selected: &mut PlayFxConfig,
    field: &str,
    value: Value,
) -> bool {
    match field {
        "type" => {
            let Some(value) = value
                .as_str()
                .filter(|value| play_fx_types().any(|known| known == *value))
            else {
                return false;
            };
            if selected.fx_type == value {
                return false;
            }
            selected.fx_type = value.into();
            selected.params.clear();
            true
        }
        "target" => {
            let Some(value) = value.as_str().filter(|value| is_play_fx_target(value)) else {
                return false;
            };
            if selected.target_key == value {
                return false;
            }
            selected.target_key = value.into();
            true
        }
        field => {
            let (Some(key), Some(value)) = (field.strip_prefix("params."), value.as_f64()) else {
                return false;
            };
            let value = value.round() as i64;
            selected.params.insert(key.into(), value) != Some(value)
        }
    }
}
