use super::Value;

pub(super) fn validate_portable_patch_fields(
    payload: &Value,
    current: &Value,
) -> Result<(), String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "portable patch must be an object".to_string())?;
    for (key, value) in object {
        match key.as_str() {
            "kind" | "schemaVersion" | "revision" => {}
            "runtimeConfig" => {
                let template = current
                    .get("runtimeConfig")
                    .ok_or_else(|| "current configuration is missing runtimeConfig".to_string())?;
                validate_value(value, template, "$.runtimeConfig")?;
            }
            "mappingConfig" => {
                let template = current
                    .get("mappingConfig")
                    .ok_or_else(|| "current configuration is missing mappingConfig".to_string())?;
                validate_value(value, template, "$.mappingConfig")?;
            }
            "system" => {
                validate_object_keys(value, "$.system", SYSTEM_FIELDS)?;
            }
            _ => return Err(format!("$.{key} is unknown in a v2 portable patch")),
        }
    }
    validate_portable_patch_sample_paths(payload, Some(current))?;
    Ok(())
}

pub(super) fn validate_portable_patch_sample_paths(
    payload: &Value,
    _current: Option<&Value>,
) -> Result<(), String> {
    let Some(runtime) = payload.get("runtimeConfig") else {
        return Ok(());
    };
    let Some(instruments) = runtime.get("instruments").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, instrument) in instruments.iter().enumerate() {
        let Some(instrument) = instrument.as_object() else {
            continue;
        };
        let Some(slots) = instrument
            .get("sample")
            .and_then(|sample| sample.get("slots"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for (slot_index, slot) in slots.iter().enumerate() {
            let Some(value) = slot.get("path") else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let path =
                format!("$.runtimeConfig.instruments[{index}].sample.slots[{slot_index}].path");
            let value = value.as_str().ok_or_else(|| {
                format!("{path} must be a canonical default-library WAV sample ID")
            })?;
            validate_default_sample_id(value, path.as_str())?;
        }
    }
    Ok(())
}

fn validate_default_sample_id(value: &str, path: &str) -> Result<(), String> {
    if !value.starts_with("samples/") || value.contains('\\') {
        return Err(format!(
            "{path} must be a canonical default-library WAV sample ID"
        ));
    }
    let relative = &value["samples/".len()..];
    if relative.is_empty()
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !relative.to_ascii_lowercase().ends_with(".wav")
        || !canonical_manifest_contains(relative)
    {
        return Err(format!(
            "{path} must be a canonical default-library WAV sample ID"
        ));
    }
    Ok(())
}

fn canonical_manifest_contains(relative: &str) -> bool {
    include_str!("../../../../samples/ATTRIBUTIONS.tsv")
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').next())
        .any(|path| path == relative)
}

fn validate_value(value: &Value, template: &Value, path: &str) -> Result<(), String> {
    if is_opaque_path(path) || is_dynamic_params_path(path) {
        return Ok(());
    }
    if is_param_binding_path(path) {
        return validate_object_keys(value, path, PARAM_BINDING_FIELDS);
    }
    if is_aux_binding_path(path) {
        validate_object_keys(value, path, AUX_BINDING_FIELDS)?;
        if let Some(action) = value.get("pressAction") {
            if !action.is_null() {
                validate_object_keys(action, &format!("{path}.pressAction"), PRESS_ACTION_FIELDS)?;
            }
        }
        return Ok(());
    }
    if is_sample_assignments_path(path) {
        return validate_sample_assignments(value, path);
    }
    if is_sparks_assignments_path(path) {
        return validate_sparks_assignments(value, path);
    }
    match (value, template) {
        (Value::Object(object), Value::Object(template)) => {
            for (key, value) in object {
                let next_path = format!("{path}.{key}");
                let Some(template_value) = template.get(key) else {
                    return Err(format!("{next_path} is unknown in a v2 portable patch"));
                };
                validate_value(value, template_value, &next_path)?;
            }
        }
        (Value::Array(values), Value::Array(template)) => {
            for (index, value) in values.iter().enumerate() {
                let next_path = format!("{path}[{index}]");
                let Some(template_value) = template.get(index).or_else(|| template.first()) else {
                    return Err(format!("{next_path} has no canonical schema entry"));
                };
                validate_value(value, template_value, &next_path)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_object_keys(value: &Value, path: &str, allowed: &[&str]) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{path}.{key} is unknown in a v2 portable patch"));
        }
    }
    Ok(())
}

fn validate_sample_assignment(value: &Value, path: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    validate_object_keys(value, path, SAMPLE_ASSIGNMENT_FIELDS)?;
    for key in ["sampleSlot", "x", "y"] {
        if let Some(value) = object.get(key) {
            if value.as_u64().is_none() {
                return Err(format!("{path}.{key} must be an unsigned integer"));
            }
        }
    }
    if let Some(level) = object.get("level") {
        if !level.is_null() {
            let level = level
                .as_str()
                .ok_or_else(|| format!("{path}.level must be null or a string enum"))?;
            if !SAMPLE_ASSIGNMENT_LEVELS.contains(&level) {
                return Err(format!("{path}.level has unknown value `{level}`"));
            }
        }
    }
    Ok(())
}

fn validate_sample_assignments(value: &Value, path: &str) -> Result<(), String> {
    let assignments = value
        .as_array()
        .ok_or_else(|| format!("{path} must be an array"))?;
    for (index, value) in assignments.iter().enumerate() {
        validate_sample_assignment(value, &format!("{path}[{index}]"))?;
    }
    Ok(())
}

fn validate_sparks_assignments(value: &Value, path: &str) -> Result<(), String> {
    let assignments = value
        .as_array()
        .ok_or_else(|| format!("{path} must be an array"))?;
    for (index, value) in assignments.iter().enumerate() {
        let path = format!("{path}[{index}]");
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path} must be an object"))?;
        validate_object_keys(value, &path, SPARKS_ASSIGNMENT_FIELDS)?;
        for key in ["x", "y"] {
            if let Some(value) = object.get(key) {
                if value.as_u64().is_none() {
                    return Err(format!("{path}.{key} must be an unsigned integer"));
                }
            }
        }
        if let Some(config) = object.get("config") {
            validate_sparks_assignment_config(config, &format!("{path}.config"))?;
        }
    }
    Ok(())
}

fn validate_sparks_assignment_config(value: &Value, path: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))?;
    validate_object_keys(value, path, SPARKS_CONFIG_FIELDS)?;
    for key in ["fxType", "targetKey"] {
        if let Some(value) = object.get(key) {
            if !value.is_string() {
                return Err(format!("{path}.{key} must be a string"));
            }
        }
    }
    if let Some(params) = object.get("params") {
        if !params.is_object() {
            return Err(format!("{path}.params must be an object"));
        }
    }
    Ok(())
}

fn is_opaque_path(path: &str) -> bool {
    [
        ".behaviorConfig",
        ".behaviorConfigHistory",
        ".savedState",
        ".behaviorState",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn is_dynamic_params_path(path: &str) -> bool {
    path.ends_with(".params")
}

fn is_param_binding_path(path: &str) -> bool {
    path.ends_with(".target")
        || path.ends_with(".xy.x")
        || path.ends_with(".xy.y")
        || path.contains(".paramMods.x[")
        || path.contains(".paramMods.y[")
}

fn is_aux_binding_path(path: &str) -> bool {
    (path.contains(".auxBindings.") || path.contains(".shiftAuxBindings."))
        && !path.ends_with(".pressAction")
}

fn is_sample_assignments_path(path: &str) -> bool {
    path.ends_with(".sample.assignments")
}

fn is_sparks_assignments_path(path: &str) -> bool {
    path.ends_with(".sparksFx.assignments")
}

const PARAM_BINDING_FIELDS: &[&str] = &[
    "key", "label", "kind", "min", "max", "step", "userMin", "userMax", "options", "invert",
];
const AUX_BINDING_FIELDS: &[&str] = &["turnKey", "pressAction"];
const PRESS_ACTION_FIELDS: &[&str] = &["kind", "actionType", "action", "slot"];
const SYSTEM_FIELDS: &[&str] = &["sparksMode"];
const SAMPLE_ASSIGNMENT_FIELDS: &[&str] = &["level", "sampleSlot", "x", "y"];
const SAMPLE_ASSIGNMENT_LEVELS: &[&str] = &["high", "medium", "low"];
const SPARKS_ASSIGNMENT_FIELDS: &[&str] = &["config", "x", "y"];
const SPARKS_CONFIG_FIELDS: &[&str] = &["fxType", "params", "targetKey"];
