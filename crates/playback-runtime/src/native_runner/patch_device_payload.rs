use super::{
    validate_patch_fields, validate_patch_sample_paths, DeviceRuntimeConfigDto, RuntimeConfigDto,
    Value,
};
use serde_json::json;

pub(super) fn portable_patch_projection(payload: &Value) -> Result<Value, String> {
    let runtime = payload
        .get("runtimeConfig")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    let mut patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": patch_runtime_config(runtime)?,
    });
    if let Some(mapping_config) = payload.get("mappingConfig") {
        patch["mappingConfig"] = mapping_config.clone();
    }
    Ok(canonicalize_json(patch))
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn portable_patch_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let patch = portable_patch_projection(payload)?;
    validate_patch_sample_paths(&patch)?;
    serde_json::to_vec(&patch)
        .map_err(|error| format!("portable patch cannot be serialized: {error}"))
}

pub(super) fn portable_patch_payload_for_save(payload: &Value) -> Result<Value, String> {
    let patch = portable_patch_projection(payload)?;
    validate_patch_sample_paths(&patch)?;
    Ok(patch)
}

pub(super) fn local_patch_payload_for_save(payload: &Value) -> Result<Value, String> {
    let patch = portable_patch_projection(payload)?;
    validate_patch_fields(&patch, &patch)?;
    validate_patch_sample_paths(&patch)?;
    Ok(patch)
}

pub(super) fn patch_payload_from_payload(payload: Value) -> Result<Value, String> {
    portable_patch_projection(&payload)
}

pub(super) fn device_config_payload_from_payload(payload: Value) -> Result<Value, String> {
    let runtime = payload.get("runtimeConfig").cloned().unwrap_or(payload);
    Ok(json!({ "runtimeConfig": device_runtime_config(runtime)? }))
}

pub(super) fn patch_runtime_config(runtime: Value) -> Result<Value, String> {
    RuntimeConfigDto::from_value(&runtime)?.portable_value()
}

pub(super) fn device_runtime_config(runtime: Value) -> Result<Value, String> {
    let typed = RuntimeConfigDto::from_value(&runtime)?;
    DeviceRuntimeConfigDto::from_runtime(&typed).to_value()
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize_json(value)))
                    .collect(),
            )
        }
        value => value,
    }
}
