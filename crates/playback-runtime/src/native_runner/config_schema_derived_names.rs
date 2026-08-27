use super::super::{derive_bus_name_from_slots, derive_instrument_name, Value};
use serde_json::Map;

pub(super) fn canonicalize_partial_payload_names(payload: &mut Value, current: &Value) {
    let current_runtime = runtime_object(current);
    let Some(runtime) = runtime_object_mut(payload) else {
        return;
    };
    canonicalize_partial_instrument_names(
        runtime.get_mut("instruments").and_then(Value::as_array_mut),
        current_runtime
            .and_then(|runtime| runtime.get("instruments"))
            .and_then(Value::as_array),
    );
    canonicalize_partial_bus_names(
        runtime
            .get_mut("mixer")
            .and_then(Value::as_object_mut)
            .and_then(|mixer| mixer.get_mut("buses"))
            .and_then(Value::as_array_mut),
        current_runtime
            .and_then(|runtime| runtime.get("mixer"))
            .and_then(|mixer| mixer.get("buses"))
            .and_then(Value::as_array),
    );
}

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

fn canonicalize_partial_instrument_names(
    instruments: Option<&mut Vec<Value>>,
    current_instruments: Option<&Vec<Value>>,
) {
    let Some(instruments) = instruments else {
        return;
    };
    for (index, instrument) in instruments.iter_mut().enumerate() {
        let Some(instrument) = instrument.as_object_mut() else {
            continue;
        };
        let current = current_instruments.and_then(|items| items.get(index));
        let current = current.and_then(Value::as_object);
        let auto_name = effective_auto_name(instrument, current);
        if !auto_name {
            continue;
        }
        let kind = effective_instrument_kind(instrument, current);
        let name = instrument.get("name").and_then(Value::as_str);
        if !instrument.contains_key("name") || is_legacy_instrument_name(name, kind) {
            instrument.insert(
                "name".into(),
                Value::String(derive_instrument_name(index, kind)),
            );
        }
    }
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
        let name = source.get("name").and_then(Value::as_str);
        if is_legacy_instrument_name(name, kind) {
            instrument.insert(
                "name".into(),
                Value::String(derive_instrument_name(index, kind)),
            );
        }
    }
}

fn canonicalize_partial_bus_names(
    buses: Option<&mut Vec<Value>>,
    current_buses: Option<&Vec<Value>>,
) {
    let Some(buses) = buses else {
        return;
    };
    for (index, bus) in buses.iter_mut().enumerate() {
        let Some(bus) = bus.as_object_mut() else {
            continue;
        };
        let current = current_buses
            .and_then(|items| items.get(index))
            .and_then(Value::as_object);
        if !effective_auto_name(bus, current) {
            continue;
        }
        let slots = normalized_partial_bus_slots(bus, current);
        let name = bus.get("name").and_then(Value::as_str);
        if !bus.contains_key("name") || name == Some(legacy_bus_name(&slots).as_str()) {
            bus.insert("name".into(), Value::String(canonical_bus_name(&slots)));
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
        let name = source.get("name").and_then(Value::as_str);
        if name == Some(legacy_bus_name(&slots).as_str()) {
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

fn effective_instrument_kind<'a>(
    value: &'a Map<String, Value>,
    current: Option<&'a Map<String, Value>>,
) -> &'a str {
    value
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| is_valid_instrument_kind(kind))
        .or_else(|| {
            current
                .and_then(|current| current.get("type"))
                .and_then(Value::as_str)
        })
        .unwrap_or("none")
}

fn normalized_instrument_kind(value: Option<&Value>) -> &str {
    value
        .and_then(Value::as_str)
        .filter(|kind| is_valid_instrument_kind(kind))
        .unwrap_or("none")
}

fn is_valid_instrument_kind(kind: &str) -> bool {
    matches!(kind, "none" | "synth" | "sampler" | "midi")
}

fn is_legacy_instrument_name(name: Option<&str>, kind: &str) -> bool {
    matches!(
        (kind, name),
        ("none", Some("none"))
            | ("synth", Some("synth"))
            | ("sampler", Some("sampler"))
            | ("midi", Some("midi"))
    )
}

fn normalized_partial_bus_slots(
    value: &Map<String, Value>,
    current: Option<&Map<String, Value>>,
) -> [String; 3] {
    let current_slots = current
        .map(normalized_bus_slots)
        .unwrap_or_else(|| ["none".into(), "none".into(), "none".into()]);
    [
        normalized_slot_type(value.get("slot1"), &current_slots[0]),
        normalized_slot_type(value.get("slot2"), &current_slots[1]),
        if value.get("slot3").is_some() {
            normalized_slot_type(value.get("slot3"), &current_slots[2])
        } else {
            "none".into()
        },
    ]
}

fn normalized_merged_bus_slots(value: &Map<String, Value>) -> [String; 3] {
    [
        normalized_slot_type(value.get("slot1"), "none"),
        normalized_slot_type(value.get("slot2"), "none"),
        normalized_slot_type(value.get("slot3"), "none"),
    ]
}

fn normalized_bus_slots(value: &Map<String, Value>) -> [String; 3] {
    normalized_merged_bus_slots(value)
}

fn normalized_slot_type(value: Option<&Value>, current: &str) -> String {
    value
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .map(|kind| {
            if crate::native_menu::is_valid_fx_bus_slot_type(kind) {
                kind.to_string()
            } else {
                "none".into()
            }
        })
        .unwrap_or_else(|| current.into())
}

fn canonical_bus_name(slots: &[String; 3]) -> String {
    derive_bus_name_from_slots([&slots[0], &slots[1], &slots[2]])
}

fn legacy_bus_name(slots: &[String; 3]) -> String {
    let slots = slots
        .iter()
        .filter(|slot| slot.as_str() != "none")
        .map(String::as_str)
        .collect::<Vec<_>>();
    match slots.as_slice() {
        [] => "(none)".into(),
        [slot] => (*slot).into(),
        slots => slots.join("+"),
    }
}
