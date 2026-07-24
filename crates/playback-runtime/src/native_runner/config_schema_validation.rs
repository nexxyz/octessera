use super::{Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION};
use serde_json::Map;

#[path = "config_schema_validation_nested.rs"]
mod nested;

pub(super) fn validate_config_payload(payload: &Value) -> Result<(), String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "configuration payload must be an object".to_string())?;
    if object.get("kind").and_then(Value::as_str) != Some(CONFIG_KIND)
        || object.get("schemaVersion").and_then(Value::as_u64) != Some(CONFIG_SCHEMA_VERSION)
    {
        return Err("prepared configuration has an invalid envelope".into());
    }
    nested::validate_payload(object)
}

pub(super) fn object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    object
        .get(key)
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| format!("{path}.{key} must be an object"))
        })
        .transpose()
}

pub(super) fn array_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
    max_len: usize,
) -> Result<Option<&'a [Value]>, String> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("{path}.{key} must be an array"))?;
    if values.len() > max_len {
        return Err(format!("{path}.{key} has too many entries"));
    }
    Ok(Some(values))
}

pub(super) fn object_value<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{path} must be an object"))
}

pub(super) fn unsigned_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    min: u64,
    max: u64,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        let value = value
            .as_u64()
            .ok_or_else(|| format!("{path}.{key} must be an unsigned integer"))?;
        if value < min || value > max {
            return Err(format!("{path}.{key} is outside the supported range"));
        }
    }
    Ok(())
}

pub(super) fn signed_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    min: i64,
    max: i64,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        signed_value(value, &format!("{path}.{key}"), min, max)?;
    }
    Ok(())
}

pub(super) fn signed_value(value: &Value, path: &str, min: i64, max: i64) -> Result<(), String> {
    let value = value
        .as_i64()
        .ok_or_else(|| format!("{path} must be a signed integer"))?;
    if value < min || value > max {
        return Err(format!("{path} is outside the supported range"));
    }
    Ok(())
}

pub(super) fn number_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    min: f64,
    max: f64,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        number_value(value, &format!("{path}.{key}"), min, max)?;
    }
    Ok(())
}

pub(super) fn number_value(value: &Value, path: &str, min: f64, max: f64) -> Result<(), String> {
    let value = value
        .as_f64()
        .ok_or_else(|| format!("{path} must be a number"))?;
    if !value.is_finite() || value < min || value > max {
        return Err(format!("{path} is outside the supported range"));
    }
    Ok(())
}

pub(super) fn bool_field(object: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        if !value.is_boolean() {
            return Err(format!("{path}.{key} must be a boolean"));
        }
    }
    Ok(())
}

pub(super) fn string_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        if !value.is_string() {
            return Err(format!("{path}.{key} must be a string"));
        }
    }
    Ok(())
}

pub(super) fn enum_field(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    options: &[&str],
) -> Result<(), String> {
    if let Some(value) = object.get(key) {
        enum_value(value, &format!("{path}.{key}"), options)?;
    }
    Ok(())
}

pub(super) fn enum_value(value: &Value, path: &str, options: &[&str]) -> Result<(), String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("{path} must be a string enum"))?;
    if !options.contains(&value) {
        return Err(format!("{path} has unknown value `{value}`"));
    }
    Ok(())
}
