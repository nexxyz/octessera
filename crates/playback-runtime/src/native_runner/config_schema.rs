use super::{
    merge_preserved_aux_payloads, migrate_legacy_modulation, patch_payload_from_payload,
    validate_canonical_lfo_bank_shape, validate_config_payload, Value,
};

pub(super) const CONFIG_KIND: &str = "octessera.config";
pub(super) const PATCH_KIND: &str = "octessera.patch";
pub(super) const CONFIG_SCHEMA_VERSION: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvelopeVersion {
    Unversioned,
    V1,
    V2,
}

impl EnvelopeVersion {
    fn is_legacy(self) -> bool {
        matches!(self, Self::Unversioned | Self::V1)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PreparedConfigPayload {
    pub(super) payload: Value,
    pub(super) apply_payload: Value,
    pub(super) source_revision: Option<u64>,
    pub(super) migration_report: Option<String>,
}

pub(super) fn prepare_config_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision, version) = parse_envelope(input, CONFIG_KIND)?;
    let mut input = input;
    validate_canonical_lfo_bank_shape(&input)?;
    if version == EnvelopeVersion::V2 && !has_global_lfo_bank(&input) {
        return Err("runtimeConfig.linkLfos must be supplied in a v2 full config".into());
    }
    if version.is_legacy() {
        migrate_legacy_runtime_root(&mut input);
    }
    let migration_report = if version.is_legacy() {
        migrate_legacy_modulation(&mut input, current)?
    } else {
        None
    };
    let mut payload = merge_values(current, &input);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    if !version.is_legacy() {
        validate_config_payload(&payload)?;
    }
    Ok(PreparedConfigPayload {
        apply_payload: if version.is_legacy() {
            input
        } else {
            payload.clone()
        },
        payload,
        source_revision,
        migration_report,
    })
}

pub(super) fn prepare_patch_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision, version) = parse_envelope(input, PATCH_KIND)?;
    let mut input = input;
    validate_canonical_lfo_bank_shape(&input)?;
    if version.is_legacy() {
        migrate_legacy_runtime_root(&mut input);
    }
    let migration_report = if version.is_legacy() {
        migrate_legacy_modulation(&mut input, current)?
    } else {
        None
    };
    let mut patch = patch_payload_from_payload(input);
    merge_preserved_aux_payloads(&mut patch, current, true);
    let mut payload = merge_values(current, &patch);
    discard_incompatible_layer_state(&mut payload, &patch, current);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    if !version.is_legacy() {
        validate_config_payload(&payload)?;
    }
    Ok(PreparedConfigPayload {
        apply_payload: if version.is_legacy() {
            patch
        } else {
            payload.clone()
        },
        payload,
        source_revision,
        migration_report,
    })
}

pub(super) fn prepare_device_payload(
    input: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let (input, source_revision, version) = parse_envelope(input, CONFIG_KIND)?;
    let mut input = input;
    validate_canonical_lfo_bank_shape(&input)?;
    if version.is_legacy() {
        migrate_legacy_runtime_root(&mut input);
    }
    let migration_report = if version.is_legacy() {
        migrate_legacy_modulation(&mut input, current)?
    } else {
        None
    };
    let mut device = super::device_config_payload_from_payload(input);
    merge_preserved_aux_payloads(&mut device, current, false);
    let mut payload = merge_values(current, &device);
    set_config_envelope(
        &mut payload,
        source_revision.or_else(|| revision_of(current)),
    );
    if !version.is_legacy() {
        validate_config_payload(&payload)?;
    }
    Ok(PreparedConfigPayload {
        apply_payload: if version.is_legacy() {
            device
        } else {
            payload.clone()
        },
        payload,
        source_revision,
        migration_report,
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
        let Some(current_behavior) = current_layers
            .get(index)
            .and_then(|layer| layer.get("worlds"))
            .and_then(|worlds| worlds.get("behaviorId"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let behavior_changed = patch_worlds
            .get("behaviorId")
            .and_then(Value::as_str)
            .is_some_and(|next_behavior| next_behavior != current_behavior);
        let config_changed = patch_worlds
            .get("behaviorConfig")
            .zip(
                current_layers
                    .get(index)
                    .and_then(|layer| layer.get("worlds"))
                    .and_then(|worlds| worlds.get("behaviorConfig")),
            )
            .is_some_and(|(patch_config, current_config)| {
                merge_values(current_config, patch_config) != *current_config
            })
            || patch_worlds
                .get("behaviorConfig")
                .is_some_and(|patch_config| {
                    current_layers
                        .get(index)
                        .and_then(|layer| layer.get("worlds"))
                        .and_then(|worlds| worlds.get("behaviorConfig"))
                        .is_none()
                        && !patch_config
                            .as_object()
                            .is_some_and(|object| object.is_empty())
                });
        if let Some(worlds) = candidate_layers
            .get_mut(index)
            .and_then(|layer| layer.get_mut("worlds"))
            .and_then(Value::as_object_mut)
        {
            if behavior_changed && patch_worlds.get("behaviorConfig").is_none() {
                worlds.remove("behaviorConfig");
            }
            if patch_worlds.get("savedState").is_none() && (behavior_changed || config_changed) {
                worlds.remove("savedState");
                worlds.remove("behaviorState");
            }
        }
    }
}

fn parse_envelope(
    input: Value,
    expected_kind: &str,
) -> Result<(Value, Option<u64>, EnvelopeVersion), String> {
    let object = input
        .as_object()
        .ok_or_else(|| "configuration payload must be an object".to_string())?;
    let has_kind = object.contains_key("kind");
    let has_schema = object.contains_key("schemaVersion");
    if has_kind != has_schema {
        return Err("configuration envelope must include kind and schemaVersion".into());
    }
    let version = if has_kind {
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
        if version != CONFIG_SCHEMA_VERSION && version != 1 {
            return Err(format!(
                "unsupported configuration schema version {version}"
            ));
        }
        if version == CONFIG_SCHEMA_VERSION {
            EnvelopeVersion::V2
        } else {
            EnvelopeVersion::V1
        }
    } else {
        EnvelopeVersion::Unversioned
    };
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
    Ok((input, revision, version))
}

fn has_global_lfo_bank(payload: &Value) -> bool {
    payload
        .get("runtimeConfig")
        .unwrap_or(payload)
        .get("linkLfos")
        .is_some()
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
    if payload.get("runtimeConfig").is_some() {
        return;
    }
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let mut runtime = object.clone();
    runtime.remove("kind");
    runtime.remove("schemaVersion");
    runtime.remove("revision");
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
