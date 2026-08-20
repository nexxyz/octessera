use super::canonical::{
    array_field, bool_field, enum_field, number_value, object_field, object_value, signed_field,
    string_field, unsigned_field, unsigned_value_range,
};
use super::Value;
use platform_core::INSTRUMENT_COUNT;
use serde_json::Map;

pub(super) fn validate_bindings(runtime: &Map<String, Value>) -> Result<(), String> {
    for key in ["auxBindings", "shiftAuxBindings"] {
        let Some(bindings) = object_field(runtime, key, "runtimeConfig")? else {
            continue;
        };
        for (slot, value) in bindings {
            validate_aux_binding(value, &format!("runtimeConfig.{key}.{slot}"))?;
        }
    }
    Ok(())
}

fn validate_aux_binding(value: &Value, path: &str) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let binding = object_value(value, path)?;
    if let Some(turn_key) = binding.get("turnKey") {
        if let Some(turn_key) = turn_key.as_str() {
            if turn_key.starts_with("layers.") && turn_key.contains(".linkLfo.") {
                return Err(format!("{path}.turnKey uses a legacy per-layer LFO key"));
            }
            if !super::super::supported_aux_turn_key(turn_key) {
                return Err(format!("{path}.turnKey is unsupported"));
            }
        } else if !turn_key.is_null() {
            return Err(format!("{path}.turnKey must be a string or null"));
        }
    }
    if let Some(action) = binding.get("pressAction") {
        if action.is_null() {
            return Ok(());
        }
        let action_path = format!("{path}.pressAction");
        let action = object_value(action, &action_path)?;
        let kind = action
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{action_path}.kind must be a string"))?;
        match kind {
            "behavior_action" => string_field(action, "actionType", &action_path)?,
            "platform_effect" => string_field(action, "action", &action_path)?,
            "instrument_clone" | "instrument_reset" => unsigned_field(
                action,
                "slot",
                &action_path,
                0,
                (INSTRUMENT_COUNT - 1) as u64,
            )?,
            "reset_behavior" => {}
            _ => return Err(format!("{action_path}.kind has unknown value `{kind}`")),
        }
    }
    Ok(())
}

pub(super) fn validate_binding_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        validate_binding_value(value, &format!("{path}.{key}"))?;
    }
    Ok(())
}

pub(super) fn validate_binding_value(value: &Value, path: &str) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let binding = object_value(value, path)?;
    let key = binding
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{path}.key must be a string"))?;
    if !super::super::supported_param_binding_key(key) {
        return Err(format!("{path}.key is unsupported"));
    }
    enum_field(binding, "kind", path, &["number", "enum", "bool"])?;
    if let Some(label) = binding.get("label") {
        if !label.is_null() && !label.is_string() {
            return Err(format!("{path}.label must be a string or null"));
        }
    }
    for key in ["min", "max", "step", "userMin", "userMax"] {
        if let Some(value) = binding.get(key) {
            number_value(value, &format!("{path}.{key}"), f64::MIN, f64::MAX)?;
        }
    }
    if let Some(options) = array_field(binding, "options", path, usize::MAX)? {
        for (index, value) in options.iter().enumerate() {
            if !value.is_string() {
                return Err(format!("{path}.options[{index}] must be a string"));
            }
        }
    }
    bool_field(binding, "invert", path)
}

pub(super) fn validate_mapping(mapping: &Map<String, Value>, path: &str) -> Result<(), String> {
    for key in [
        "scanned",
        "scanned_empty",
        "activate",
        "stable",
        "deactivate",
    ] {
        let Some(event) = object_field(mapping, key, path)? else {
            continue;
        };
        let event_path = format!("{path}.{key}");
        if let Some(slot) = event.get("slot") {
            if slot.as_str() != Some("none")
                && slot
                    .as_u64()
                    .is_none_or(|value| value >= INSTRUMENT_COUNT as u64)
            {
                return Err(format!("{event_path}.slot is outside the supported range"));
            }
        }
        enum_field(
            event,
            "action",
            &event_path,
            &["none", "note_on", "note_off"],
        )?;
        unsigned_field(event, "delaySteps", &event_path, 0, 16)?;
        unsigned_field(event, "retriggerCount", &event_path, 0, 8)?;
    }
    Ok(())
}

pub(super) fn validate_mapping_config(value: &Value) -> Result<(), String> {
    let object = object_value(value, "mappingConfig")?;
    for key in ["baseMidiNote", "startingMidiNote", "maxMidiNote"] {
        unsigned_field(object, key, "mappingConfig", 0, 127)?;
    }
    let base = object
        .get("baseMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let starting = object
        .get("startingMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(base);
    let max = object
        .get("maxMidiNote")
        .and_then(Value::as_u64)
        .unwrap_or(127);
    if starting < base || starting > max || max < base {
        return Err("mappingConfig MIDI note range is invalid".into());
    }
    enum_field(object, "rangeMode", "mappingConfig", &["clamp", "wrap"])?;
    let scale = object
        .get("scale")
        .and_then(Value::as_array)
        .ok_or_else(|| "mappingConfig.scale must be a non-empty array".to_string())?;
    if scale.is_empty() {
        return Err("mappingConfig.scale must not be empty".into());
    }
    for (index, value) in scale.iter().enumerate() {
        unsigned_value_range(value, &format!("mappingConfig.scale[{index}]"), 0, 11)?;
    }
    for key in ["rowStepDegrees", "columnStepDegrees"] {
        signed_field(object, key, "mappingConfig", -16, 16)?;
    }
    for key in [
        "activate",
        "deactivate",
        "stable",
        "scanned",
        "scanned_empty",
    ] {
        let target = object_field(object, key, "mappingConfig")?
            .ok_or_else(|| format!("mappingConfig.{key} must be an object"))?;
        let path = format!("mappingConfig.{key}");
        enum_field(target, "action", &path, &["none", "note_on", "note_off"])?;
        unsigned_field(target, "channel", &path, 0, 15)?;
        unsigned_field(target, "velocity", &path, 1, 127)?;
        unsigned_field(target, "durationMs", &path, 1, 8000)?;
    }
    Ok(())
}
