use super::{supported_param_binding_key, Value};
use serde_json::json;

pub(super) fn patch_payload_from_payload(payload: Value) -> Value {
    let runtime = payload
        .get("runtimeConfig")
        .cloned()
        .unwrap_or(payload.clone());
    let mut patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": patch_runtime_config(runtime),
    });
    if let Some(mapping_config) = payload.get("mappingConfig") {
        patch["mappingConfig"] = mapping_config.clone();
    }
    patch
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn device_config_payload_from_payload(payload: Value) -> Value {
    let runtime = payload.get("runtimeConfig").cloned().unwrap_or(payload);
    json!({ "runtimeConfig": device_runtime_config(runtime) })
}

pub(super) fn patch_runtime_config(mut runtime: Value) -> Value {
    let Some(object) = runtime.as_object_mut() else {
        return runtime;
    };
    for key in DEVICE_RUNTIME_KEYS {
        object.remove(*key);
    }
    if let Some(sound) = object.get_mut("sound").and_then(Value::as_object_mut) {
        sound.remove("audioOutputBufferFrames");
    }
    split_aux_payloads(object, true);
    runtime
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn device_runtime_config(runtime: Value) -> Value {
    let mut device = serde_json::Map::new();
    let Some(object) = runtime.as_object() else {
        return Value::Object(device);
    };
    for key in DEVICE_RUNTIME_KEYS {
        if let Some(value) = object.get(*key) {
            device.insert((*key).into(), value.clone());
        }
    }
    if let Some(value) = object
        .get("sound")
        .and_then(|sound| sound.get("audioOutputBufferFrames"))
    {
        device.insert("sound".into(), json!({ "audioOutputBufferFrames": value }));
    }
    if let Some(aux) = object.get("auxBindings") {
        device.insert("auxBindings".into(), split_aux_payload(aux, false));
    }
    if let Some(aux) = object.get("shiftAuxBindings") {
        device.insert("shiftAuxBindings".into(), split_aux_payload(aux, false));
    }
    Value::Object(device)
}

pub(super) fn merge_preserved_aux_payloads(payload: &mut Value, preserved: &Value, musical: bool) {
    let runtime = if payload.get("runtimeConfig").is_some() {
        payload.get_mut("runtimeConfig").expect("runtimeConfig")
    } else {
        payload
    };
    let preserved_runtime = preserved.get("runtimeConfig").unwrap_or(preserved);
    let Some(runtime_object) = runtime.as_object_mut() else {
        return;
    };
    for key in ["auxBindings", "shiftAuxBindings"] {
        let incoming = runtime_object
            .get(key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let preserved_aux = preserved_runtime
            .get(key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        runtime_object.insert(
            key.into(),
            merge_aux_payload(&incoming, &preserved_aux, musical),
        );
    }
}

fn split_aux_payloads(object: &mut serde_json::Map<String, Value>, musical: bool) {
    for key in ["auxBindings", "shiftAuxBindings"] {
        if let Some(value) = object.get_mut(key) {
            *value = split_aux_payload(value, musical);
        }
    }
}

fn split_aux_payload(payload: &Value, musical: bool) -> Value {
    let mut out = serde_json::Map::new();
    let Some(object) = payload.as_object() else {
        return Value::Object(out);
    };
    for (key, binding) in object {
        let Some(binding_object) = binding.as_object() else {
            out.insert(key.clone(), Value::Null);
            continue;
        };
        let mut next = serde_json::Map::new();
        if let Some(turn_key) = binding_object.get("turnKey").and_then(Value::as_str) {
            if is_musical_aux_turn_key(turn_key) == musical {
                next.insert("turnKey".into(), json!(turn_key));
            }
        }
        if let Some(action) = binding_object.get("pressAction") {
            if is_musical_aux_press_action(action) == musical {
                next.insert("pressAction".into(), action.clone());
            }
        }
        out.insert(
            key.clone(),
            if next.is_empty() {
                Value::Null
            } else {
                Value::Object(next)
            },
        );
    }
    Value::Object(out)
}

fn merge_aux_payload(incoming: &Value, preserved: &Value, incoming_musical: bool) -> Value {
    let mut out = serde_json::Map::new();
    let keys = incoming
        .as_object()
        .into_iter()
        .flat_map(|object| object.keys())
        .chain(
            preserved
                .as_object()
                .into_iter()
                .flat_map(|object| object.keys()),
        );
    for key in keys {
        let incoming_binding = incoming.get(key).unwrap_or(&Value::Null);
        let preserved_binding = preserved.get(key).unwrap_or(&Value::Null);
        let mut next = serde_json::Map::new();
        merge_aux_side(&mut next, incoming_binding, "turnKey", incoming_musical);
        merge_aux_side(&mut next, preserved_binding, "turnKey", !incoming_musical);
        merge_aux_side(&mut next, incoming_binding, "pressAction", incoming_musical);
        merge_aux_side(
            &mut next,
            preserved_binding,
            "pressAction",
            !incoming_musical,
        );
        out.insert(
            key.clone(),
            if next.is_empty() {
                Value::Null
            } else {
                Value::Object(next)
            },
        );
    }
    Value::Object(out)
}

fn merge_aux_side(
    out: &mut serde_json::Map<String, Value>,
    binding: &Value,
    side: &str,
    musical: bool,
) {
    let Some(value) = binding.get(side) else {
        return;
    };
    let is_musical = if side == "turnKey" {
        value.as_str().map(is_musical_aux_turn_key).unwrap_or(false)
    } else {
        is_musical_aux_press_action(value)
    };
    if is_musical == musical {
        out.entry(side).or_insert_with(|| value.clone());
    }
}

fn is_musical_aux_turn_key(key: &str) -> bool {
    supported_param_binding_key(key)
        || key.starts_with("layers.")
        || key.starts_with("mixer.")
        || key.starts_with("transport.")
        || key.starts_with("sparks.")
}

fn is_musical_aux_press_action(value: &Value) -> bool {
    if value.get("kind").and_then(Value::as_str) == Some("platform_effect") {
        return value
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(is_musical_platform_effect_action);
    }
    matches!(
        value.get("kind").and_then(Value::as_str),
        Some(
            "behavior_action"
                | "behaviorAction"
                | "instrument_clone"
                | "instrument_reset"
                | "reset_behavior"
        )
    ) || value.get("actionType").is_some()
}

fn is_musical_platform_effect_action(action: &str) -> bool {
    action == "sparks.fx.map"
        || action.starts_with("sample.assign:")
        || action.starts_with("trigger.probability.assign:")
        || action.starts_with("synth.preset:")
}

const DEVICE_RUNTIME_KEYS: &[&str] = &[
    "masterVolume",
    "sampleFavouriteDirs",
    "hdmi",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "midi",
    "usb",
    "recording",
];
