use super::{
    merge_preserved_aux_payloads, patch_payload_from_payload, validate_config_payload, Value,
};

pub(super) const CONFIG_KIND: &str = "octessera.config";
pub(super) const PATCH_KIND: &str = "octessera.patch";
pub(super) const CONFIG_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedConfigPayload {
    pub(super) payload: Value,
    pub(super) apply_payload: Value,
    pub(super) source_revision: Option<u64>,
}

pub(super) fn prepare_config_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision) = parse_envelope(input, CONFIG_KIND)?;
    let mut input = input;
    migrate_legacy_runtime_root(&mut input);
    let mut apply_payload = input.clone();
    set_config_envelope(
        &mut apply_payload,
        source_revision.or_else(|| revision_of(current)),
    );
    let mut payload = merge_values(current, &input);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    validate_config_payload(&payload)?;
    Ok(PreparedConfigPayload {
        payload,
        apply_payload,
        source_revision,
    })
}

pub(super) fn prepare_patch_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision) = parse_envelope(input, PATCH_KIND)?;
    let mut patch = patch_payload_from_payload(input);
    merge_preserved_aux_payloads(&mut patch, current, true);
    let apply_payload = patch.clone();
    let mut payload = merge_values(current, &patch);
    discard_incompatible_layer_state(&mut payload, &patch, current);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    validate_config_payload(&payload)?;
    Ok(PreparedConfigPayload {
        payload,
        apply_payload,
        source_revision,
    })
}

pub(super) fn prepare_device_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision) = parse_envelope(input, CONFIG_KIND)?;
    let mut device = super::device_config_payload_from_payload(input);
    merge_preserved_aux_payloads(&mut device, current, false);
    let apply_payload = device.clone();
    let mut payload = merge_values(current, &device);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    validate_config_payload(&payload)?;
    Ok(PreparedConfigPayload {
        payload,
        apply_payload,
        source_revision,
    })
}

fn discard_incompatible_layer_state(payload: &mut Value, patch: &Value, current: &Value) {
    let Some(patch_layers) = patch
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get("layers"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some(current_layers) = current
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get("layers"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some(candidate_layers) = payload
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("layers"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for (index, patch_layer) in patch_layers.iter().enumerate() {
        let Some(patch_worlds) = patch_layer.get("worlds") else {
            continue;
        };
        let Some(next_behavior) = patch_worlds.get("behaviorId").and_then(Value::as_str) else {
            continue;
        };
        let Some(current_behavior) = current_layers
            .get(index)
            .and_then(|layer| layer.get("worlds"))
            .and_then(|worlds| worlds.get("behaviorId"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if next_behavior == current_behavior || patch_worlds.get("savedState").is_some() {
            continue;
        }
        if let Some(worlds) = candidate_layers
            .get_mut(index)
            .and_then(|layer| layer.get_mut("worlds"))
            .and_then(Value::as_object_mut)
        {
            worlds.remove("savedState");
            worlds.remove("behaviorState");
            if patch_worlds.get("behaviorConfig").is_none() {
                worlds.remove("behaviorConfig");
            }
        }
    }
}

fn parse_envelope(input: Value, expected_kind: &str) -> Result<(Value, Option<u64>), String> {
    let object = input
        .as_object()
        .ok_or_else(|| "configuration payload must be an object".to_string())?;
    let has_kind = object.contains_key("kind");
    let has_schema = object.contains_key("schemaVersion");
    if has_kind != has_schema {
        return Err("configuration envelope must include kind and schemaVersion".into());
    }
    if has_kind {
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "configuration envelope kind must be a string".to_string())?;
        if kind != expected_kind {
            return Err(format!("unsupported configuration envelope kind `{kind}`"));
        }
        let version = object
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| "configuration schemaVersion must be an integer".to_string())?;
        if version != CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "unsupported configuration schema version {version}"
            ));
        }
    }
    let revision = object
        .get("revision")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| "configuration revision must be an unsigned integer".to_string())
        })
        .transpose()?;
    if let Some(runtime) = object.get("runtimeConfig") {
        if !runtime.is_object() {
            return Err("runtimeConfig must be an object".into());
        }
    }
    Ok((input, revision))
}

fn set_config_envelope(payload: &mut Value, revision: Option<u64>) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object.insert("kind".into(), Value::String(CONFIG_KIND.into()));
    object.insert(
        "schemaVersion".into(),
        Value::Number(CONFIG_SCHEMA_VERSION.into()),
    );
    if let Some(revision) = revision {
        object.insert("revision".into(), Value::Number(revision.into()));
    }
}

fn revision_of(payload: &Value) -> Option<u64> {
    payload.get("revision").and_then(Value::as_u64)
}

fn migrate_legacy_runtime_root(payload: &mut Value) {
    if payload.get("runtimeConfig").is_some() || payload.get("kind").is_some() {
        return;
    }
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let mut runtime = object.clone();
    runtime.remove("mappingConfig");
    runtime.remove("system");
    object.insert("runtimeConfig".into(), Value::Object(runtime));
}

fn merge_values(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            let mut merged = base.clone();
            for (key, value) in overlay {
                let next = merged
                    .get(key)
                    .map(|current| merge_values(current, value))
                    .unwrap_or_else(|| value.clone());
                merged.insert(key.clone(), next);
            }
            Value::Object(merged)
        }
        (Value::Array(base), Value::Array(overlay)) => {
            let mut merged = base.clone();
            for (index, value) in overlay.iter().enumerate() {
                if let Some(current) = merged.get(index) {
                    merged[index] = merge_values(current, value);
                } else {
                    merged.push(value.clone());
                }
            }
            Value::Array(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}
