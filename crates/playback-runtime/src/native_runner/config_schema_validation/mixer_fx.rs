use super::canonical::{
    array_field, bool_field, enum_value, number_value, object_field, object_value, string_field,
    unsigned_field,
};
use super::Value;
use crate::native_runner::fx_param_specs::{fx_param_spec, FxParam};
use platform_core::{BUS_COUNT, INSTRUMENT_COUNT, PAN_POSITION_COUNT};
use serde_json::Map;

pub(super) fn validate_mixer(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(mixer) = object_field(runtime, "mixer", "runtimeConfig")? else {
        return Ok(());
    };
    let path = "runtimeConfig.mixer";
    if let Some(buses) = array_field(mixer, "buses", path, BUS_COUNT)? {
        for (index, value) in buses.iter().enumerate() {
            let bus_path = format!("{path}.buses[{index}]");
            let bus = object_value(value, &bus_path)?;
            string_field(bus, "name", &bus_path)?;
            bool_field(bus, "autoName", &bus_path)?;
            unsigned_field(bus, "panPos", &bus_path, 0, (PAN_POSITION_COUNT - 1) as u64)?;
            unsigned_field(bus, "volumePct", &bus_path, 0, 100)?;
            for key in ["slot1", "slot2", "slot3"] {
                validate_fx_slot(bus, key, &bus_path, false)?;
            }
        }
    }
    if let Some(master) = object_field(mixer, "master", path)? {
        if let Some(slots) = array_field(
            master,
            "slots",
            &format!("{path}.master"),
            platform_core::GLOBAL_FX_SLOT_COUNT,
        )? {
            for (index, value) in slots.iter().enumerate() {
                let slot_path = format!("{path}.master.slots[{index}]");
                let slot = object_value(value, &slot_path)?;
                validate_fx_slot_value(slot, &slot_path, true)?;
            }
        }
    }
    Ok(())
}

fn validate_fx_slot(
    bus: &Map<String, Value>,
    key: &str,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let Some(value) = bus.get(key) else {
        return Ok(());
    };
    let slot_path = format!("{path}.{key}");
    let slot = object_value(value, &slot_path)?;
    validate_fx_slot_value(slot, &slot_path, global)
}

fn validate_fx_slot_value(
    slot: &Map<String, Value>,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let slot_type = slot.get("type").and_then(Value::as_str).unwrap_or("");
    let valid = if global {
        crate::native_menu::is_valid_global_fx_slot_type(slot_type)
    } else {
        crate::native_menu::is_valid_fx_bus_slot_type(slot_type)
    };
    if !valid {
        return Err(format!("{path}.type has unknown FX slot `{slot_type}`"));
    }
    if let Some(params) = object_field(slot, "params", path)? {
        for (key, value) in params {
            let value_path = format!("{path}.params.{key}");
            if key == "source" {
                if slot_type != "duck" || !valid_duck_source(value.as_str()) {
                    return Err(format!("{value_path} has an invalid source"));
                }
            } else if key == "sourceTap" {
                if slot_type != "duck" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, &["pre", "post"])?;
            } else if key == "timeMode" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, &["ms", "note"])?;
            } else if key == "timeNote" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, crate::timing_units::NOTE_UNIT_OPTIONS)?;
            } else if let Some((min, max)) = fx_param_range(slot_type, key) {
                number_value(value, &value_path, min, max)?;
            } else {
                return Err(format!("{value_path} is not valid for {slot_type}"));
            }
        }
    }
    Ok(())
}

pub fn is_valid_fx_string_param(fx_type: &str, key: &str, value: &str) -> bool {
    match (fx_type, key) {
        ("duck", "source") => valid_duck_source(Some(value)),
        ("duck", "sourceTap") => matches!(value, "pre" | "post"),
        _ => false,
    }
}

fn valid_duck_source(source: Option<&str>) -> bool {
    let Some(source) = source else {
        return false;
    };
    if source.len() < 2 {
        return false;
    }
    let (prefix, index) = source.split_at(1);
    (prefix == "I"
        && index
            .parse::<usize>()
            .ok()
            .is_some_and(|value| (1..=INSTRUMENT_COUNT).contains(&value)))
        || (prefix == "B"
            && index
                .parse::<usize>()
                .ok()
                .is_some_and(|value| (1..=BUS_COUNT).contains(&value)))
}

fn fx_param_range(slot_type: &str, key: &str) -> Option<(f64, f64)> {
    fx_param_spec(slot_type, key).map(FxParam::saved_range)
}
