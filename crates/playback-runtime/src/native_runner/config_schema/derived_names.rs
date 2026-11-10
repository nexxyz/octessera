use super::super::{derive_bus_name_from_slots, derive_instrument_name, InstrumentType, Value};
use serde_json::Map;

pub(super) fn canonicalize_merged_payload_names(payload: &mut Value, source: &Value) {
    let source_runtime = runtime_object(source);
    let Some(runtime) = runtime_object_mut(payload) else {
        return;
    };
    canonicalize_merged_instrument_names(
        runtime.get_mut("instruments").and_then(Value::as_array_mut),
        source_runtime
            .and_then(|runtime| runtime.get("instruments"))
            .and_then(Value::as_array),
    );
    canonicalize_merged_bus_names(
        runtime
            .get_mut("mixer")
            .and_then(Value::as_object_mut)
            .and_then(|mixer| mixer.get_mut("buses"))
            .and_then(Value::as_array_mut),
        source_runtime
            .and_then(|runtime| runtime.get("mixer"))
            .and_then(|mixer| mixer.get("buses"))
            .and_then(Value::as_array),
    );
}

fn canonicalize_merged_instrument_names(
    instruments: Option<&mut Vec<Value>>,
    source_instruments: Option<&Vec<Value>>,
) {
    let Some(instruments) = instruments else {
        return;
    };
    for (index, instrument) in instruments.iter_mut().enumerate() {
        let Some(instrument) = instrument.as_object_mut() else {
            continue;
        };
        let Some(source) = source_instruments
            .and_then(|items| items.get(index))
            .and_then(Value::as_object)
        else {
            continue;
        };
        let auto_name = effective_auto_name(instrument, Some(source));
        if !auto_name {
            continue;
        }
        let kind = normalized_instrument_kind(instrument.get("type"));
        if !source.contains_key("name") {
            instrument.insert(
                "name".into(),
                Value::String(derive_instrument_name(index, kind)),
            );
        }
    }
}

fn canonicalize_merged_bus_names(
    buses: Option<&mut Vec<Value>>,
    source_buses: Option<&Vec<Value>>,
) {
    let Some(buses) = buses else {
        return;
    };
    for (index, bus) in buses.iter_mut().enumerate() {
        let Some(bus) = bus.as_object_mut() else {
            continue;
        };
        let Some(source) = source_buses
            .and_then(|items| items.get(index))
            .and_then(Value::as_object)
        else {
            continue;
        };
        if !effective_auto_name(bus, Some(source)) {
            continue;
        }
        let slots = normalized_merged_bus_slots(bus);
        if !source.contains_key("name") {
            bus.insert("name".into(), Value::String(canonical_bus_name(&slots)));
        }
    }
}

fn runtime_object(value: &Value) -> Option<&Map<String, Value>> {
    value
        .get("runtimeConfig")
        .and_then(Value::as_object)
        .or_else(|| value.as_object())
}

fn runtime_object_mut(value: &mut Value) -> Option<&mut Map<String, Value>> {
    if value.get("runtimeConfig").is_some() {
        value
            .get_mut("runtimeConfig")
            .and_then(Value::as_object_mut)
    } else {
        value.as_object_mut()
    }
}

fn effective_auto_name(value: &Map<String, Value>, current: Option<&Map<String, Value>>) -> bool {
    value
        .get("autoName")
        .and_then(Value::as_bool)
        .or_else(|| {
            current
                .and_then(|current| current.get("autoName"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(true)
}

fn normalized_instrument_kind(value: Option<&Value>) -> InstrumentType {
    value
        .and_then(Value::as_str)
        .and_then(InstrumentType::from_wire_name)
        .unwrap_or(InstrumentType::None)
}

fn normalized_merged_bus_slots(value: &Map<String, Value>) -> [String; 3] {
    [
        normalized_slot_type(value.get("slot1")),
        normalized_slot_type(value.get("slot2")),
        normalized_slot_type(value.get("slot3")),
    ]
}

fn normalized_slot_type(value: Option<&Value>) -> String {
    value
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .filter(|kind| crate::native_menu::is_valid_fx_bus_slot_type(kind))
        .unwrap_or("none")
        .to_string()
}

fn canonical_bus_name(slots: &[String; 3]) -> String {
    derive_bus_name_from_slots([&slots[0], &slots[1], &slots[2]])
}
