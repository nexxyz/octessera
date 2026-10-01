use super::config_document_presence::{require_aux_banks, require_full_runtime_presence};
use super::{
    device_config_payload_from_payload, local_patch_payload_for_save,
    portable_patch_payload_for_save, portable_patch_projection, prepare_config_payload,
    validate_config_payload, validate_local_patch_sample_paths, validate_patch_fields,
    validate_portable_patch_sample_paths, ConfigDto, Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION,
    GLOBAL_LFO_COUNT, GRID_HEIGHT, GRID_WIDTH, PATCH_KIND,
};
use serde_json::{json, Map};
use std::collections::BTreeSet;

const SYSTEM_DOCUMENT_KIND: &str = "octessera.system";
const SYSTEM_DOCUMENT_VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct SystemPatchDocuments {
    pub system: Value,
    pub patch: Value,
}

pub fn split_system_patch_documents(full_config: &Value) -> Result<SystemPatchDocuments, String> {
    let normalized = normalize_full_config(full_config)?;
    let patch = portable_patch_payload_for_save(&normalized)?;
    split_normalized_documents(&normalized, patch)
}

pub fn split_local_system_patch_documents(
    full_config: &Value,
) -> Result<SystemPatchDocuments, String> {
    let normalized = normalize_full_config_structure(full_config)?;
    let patch = portable_patch_projection(&normalized)?;
    require_local_patch_fixed_arrays(&patch["runtimeConfig"])?;
    validate_local_patch_sample_paths(&patch)?;
    let patch = local_patch_payload_for_save(&normalized)?;
    split_normalized_documents(&normalized, patch)
}

fn split_normalized_documents(
    normalized: &Value,
    patch: Value,
) -> Result<SystemPatchDocuments, String> {
    let device = device_config_payload_from_payload(normalized.clone())?;
    let system = json!({
        "kind": SYSTEM_DOCUMENT_KIND,
        "schemaVersion": SYSTEM_DOCUMENT_VERSION,
        "runtimeConfig": device["runtimeConfig"],
    });
    require_aux_banks(&system["runtimeConfig"], "system.runtimeConfig")?;
    require_aux_banks(&patch["runtimeConfig"], "patch.runtimeConfig")?;
    Ok(SystemPatchDocuments { system, patch })
}

pub fn compose_system_patch_documents(system: &Value, patch: &Value) -> Result<Value, String> {
    let full = compose_system_patch_documents_structure(system, patch)?;
    validate_portable_patch_sample_paths(patch, None)?;
    Ok(full)
}

pub fn compose_local_system_patch_documents(
    system: &Value,
    patch: &Value,
) -> Result<Value, String> {
    let patch_runtime = document_runtime(patch, PATCH_KIND, CONFIG_SCHEMA_VERSION, true)?;
    require_local_patch_fixed_arrays(patch_runtime)?;
    let full = compose_system_patch_documents_structure(system, patch)?;
    validate_local_patch_sample_paths(patch)?;
    Ok(full)
}

fn require_local_patch_fixed_arrays(runtime: &Value) -> Result<(), String> {
    let layers = require_fixed_array(
        runtime.get("layers"),
        "patch.runtimeConfig.layers",
        platform_core::LAYER_COUNT,
    )?;
    let instruments = require_fixed_array(
        runtime.get("instruments"),
        "patch.runtimeConfig.instruments",
        platform_core::INSTRUMENT_COUNT,
    )?;
    require_fixed_array(
        runtime.get("linkLfos"),
        "patch.runtimeConfig.linkLfos",
        GLOBAL_LFO_COUNT,
    )?;

    for (index, instrument) in instruments.iter().enumerate() {
        let sample = instrument
            .get("sample")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!("patch.runtimeConfig.instruments[{index}].sample is required")
            })?;
        require_fixed_array(
            sample.get("slots"),
            &format!("patch.runtimeConfig.instruments[{index}].sample.slots"),
            platform_core::SAMPLE_SLOT_COUNT,
        )?;
    }

    let mixer = runtime
        .get("mixer")
        .and_then(Value::as_object)
        .ok_or_else(|| "patch.runtimeConfig.mixer is required".to_string())?;
    require_fixed_array(
        mixer.get("buses"),
        "patch.runtimeConfig.mixer.buses",
        platform_core::BUS_COUNT,
    )?;
    let master = mixer
        .get("master")
        .and_then(Value::as_object)
        .ok_or_else(|| "patch.runtimeConfig.mixer.master is required".to_string())?;
    require_fixed_array(
        master.get("slots"),
        "patch.runtimeConfig.mixer.master.slots",
        platform_core::GLOBAL_FX_SLOT_COUNT,
    )?;

    for (index, layer) in layers.iter().enumerate() {
        let link = layer
            .get("link")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("patch.runtimeConfig.layers[{index}].link is required"))?;
        require_fixed_array(
            link.get("triggerProbabilityMap"),
            &format!("patch.runtimeConfig.layers[{index}].link.triggerProbabilityMap"),
            GRID_WIDTH * GRID_HEIGHT,
        )?;
    }
    Ok(())
}

fn require_fixed_array<'a>(
    value: Option<&'a Value>,
    path: &str,
    expected_len: usize,
) -> Result<&'a [Value], String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{path} must contain exactly {expected_len} entries"))?;
    if values.len() != expected_len {
        return Err(format!(
            "{path} must contain exactly {expected_len} entries"
        ));
    }
    Ok(values)
}

fn compose_system_patch_documents_structure(
    system: &Value,
    patch: &Value,
) -> Result<Value, String> {
    let system_runtime =
        document_runtime(system, SYSTEM_DOCUMENT_KIND, SYSTEM_DOCUMENT_VERSION, false)?;
    let patch_runtime = document_runtime(patch, PATCH_KIND, CONFIG_SCHEMA_VERSION, true)?;
    require_aux_banks(system_runtime, "system.runtimeConfig")?;
    require_aux_banks(patch_runtime, "patch.runtimeConfig")?;
    validate_patch_structure(patch)?;
    validate_aux_claim_conflicts(patch_runtime, system_runtime)?;

    let runtime_config = compose_runtime_config(patch_runtime, system_runtime)?;
    require_full_runtime_presence(&runtime_config, "composed runtimeConfig")?;

    let canonical_patch = portable_patch_projection(patch)?;
    if canonical_patch != *patch {
        return Err(
            "patch document contains data not preserved by the native patch projection".into(),
        );
    }
    let projected_system =
        device_config_payload_from_payload(json!({ "runtimeConfig": system_runtime }))?;
    validate_projection_preserves_data(
        system_runtime,
        &projected_system["runtimeConfig"],
        "system.runtimeConfig",
    )?;

    let play_mode = runtime_config
        .get("playMode")
        .and_then(Value::as_str)
        .ok_or_else(|| "patch document is missing runtimeConfig.playMode".to_string())?;
    let mut full = json!({
        "kind": CONFIG_KIND,
        "schemaVersion": CONFIG_SCHEMA_VERSION,
        "runtimeConfig": runtime_config,
        "system": { "playMode": play_mode },
    });
    if let Some(mapping_config) = patch.get("mappingConfig") {
        full["mappingConfig"] = mapping_config.clone();
    }
    validate_config_payload(&full)?;
    validate_patch_fields(&full, &full)?;
    Ok(full)
}

fn normalize_full_config(full_config: &Value) -> Result<Value, String> {
    let normalized = normalize_full_config_structure(full_config)?;
    validate_portable_patch_sample_paths(&portable_patch_projection(&normalized)?, None)?;
    Ok(normalized)
}

fn normalize_full_config_structure(full_config: &Value) -> Result<Value, String> {
    let runtime = full_config
        .get("runtimeConfig")
        .ok_or_else(|| "full config is missing runtimeConfig".to_string())?;
    require_full_runtime_presence(runtime, "full config.runtimeConfig")?;
    let config = ConfigDto::decode(full_config)?;
    if !config.extensions().is_empty() {
        return Err("full config contains unknown top-level fields".into());
    }
    validate_config_payload(full_config)?;
    validate_patch_fields(full_config, full_config)?;
    validate_full_structure(full_config)?;

    let typed_runtime = config.typed_runtime_config_value()?;
    validate_projection_preserves_data(config.runtime_config(), &typed_runtime, "runtimeConfig")?;
    let normalized = prepare_config_payload(full_config.clone(), full_config)?.payload;
    let normalized_runtime = normalized["runtimeConfig"].clone();
    let play_mode = normalized_runtime
        .get("playMode")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "full config is missing runtimeConfig.playMode".to_string())?;
    validate_full_system(config.system(), play_mode.as_str())?;
    let mut normalized = full_config.clone();
    normalized["runtimeConfig"] = normalized_runtime;
    normalized["system"] = json!({ "playMode": play_mode });
    if let Some(object) = normalized.as_object_mut() {
        object.remove("revision");
    }
    validate_config_payload(&normalized)?;
    validate_patch_fields(&normalized, &normalized)?;
    Ok(normalized)
}

fn document_runtime<'a>(
    document: &'a Value,
    expected_kind: &str,
    expected_version: u64,
    allow_mapping_config: bool,
) -> Result<&'a Value, String> {
    let object = document
        .as_object()
        .ok_or_else(|| format!("{expected_kind} document must be an object"))?;
    for key in object.keys() {
        let allowed = matches!(key.as_str(), "kind" | "schemaVersion" | "runtimeConfig")
            || (allow_mapping_config && key == "mappingConfig");
        if !allowed {
            return Err(format!(
                "{expected_kind} document has unsupported field `{key}`"
            ));
        }
    }
    if object.get("kind").and_then(Value::as_str) != Some(expected_kind) {
        return Err(format!("expected document kind `{expected_kind}`"));
    }
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(expected_version) {
        return Err(format!(
            "{expected_kind} document schemaVersion must be {expected_version}"
        ));
    }
    object
        .get("runtimeConfig")
        .filter(|runtime| runtime.is_object())
        .ok_or_else(|| format!("{expected_kind} document is missing runtimeConfig object"))
}

fn validate_full_structure(full: &Value) -> Result<(), String> {
    if !full.get("mappingConfig").is_some_and(Value::is_object) {
        return Err("full config mappingConfig must be an object".into());
    }
    Ok(())
}

fn validate_patch_structure(document: &Value) -> Result<(), String> {
    if !document.get("mappingConfig").is_some_and(Value::is_object) {
        return Err("patch document mappingConfig must be an object".into());
    }
    Ok(())
}

fn validate_full_system(system: Option<&Value>, runtime_play_mode: &str) -> Result<(), String> {
    let Some(system) = system else {
        return Ok(());
    };
    let object = system
        .as_object()
        .ok_or_else(|| "full config system must be an object".to_string())?;
    if object.keys().any(|key| key != "playMode") {
        return Err("full config system contains unsupported fields".into());
    }
    if let Some(system_play_mode) = object.get("playMode") {
        if system_play_mode.as_str() != Some(runtime_play_mode) {
            return Err("full config system.playMode conflicts with runtimeConfig.playMode".into());
        }
    }
    Ok(())
}

fn validate_projection_preserves_data(
    source: &Value,
    projection: &Value,
    path: &str,
) -> Result<(), String> {
    match (source, projection) {
        (Value::Object(source), Value::Object(projection)) => {
            for (key, value) in source {
                let Some(projected) = projection.get(key) else {
                    return Err(format!(
                        "{path}.{key} is not preserved by the native config DTO"
                    ));
                };
                validate_projection_preserves_data(value, projected, &format!("{path}.{key}"))?;
            }
            for (key, value) in projection {
                if !source.contains_key(key) && !value.is_null() {
                    return Err(format!(
                        "{path}.{key} is defaulted by the native config DTO"
                    ));
                }
            }
        }
        (Value::Array(source), Value::Array(projection)) => {
            if source.len() != projection.len() {
                return Err(format!(
                    "{path} array length changes in the native config DTO"
                ));
            }
            for (index, (value, projected)) in source.iter().zip(projection).enumerate() {
                validate_projection_preserves_data(value, projected, &format!("{path}[{index}]"))?;
            }
        }
        _ if source != projection => {
            return Err(format!("{path} is normalized by the native config DTO"));
        }
        _ => {}
    }
    Ok(())
}

fn validate_aux_claim_conflicts(patch: &Value, system: &Value) -> Result<(), String> {
    for bank in ["auxBindings", "shiftAuxBindings"] {
        let Some(patch_slots) = patch.get(bank).and_then(Value::as_object) else {
            continue;
        };
        let Some(system_slots) = system.get(bank).and_then(Value::as_object) else {
            continue;
        };
        for (slot, patch_binding) in patch_slots {
            let Some(system_binding) = system_slots.get(slot) else {
                continue;
            };
            for side in ["turnKey", "pressAction"] {
                if has_aux_side(patch_binding, side) && has_aux_side(system_binding, side) {
                    return Err(format!(
                        "{bank}.{slot}.{side} is claimed by both System and Patch"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn has_aux_side(binding: &Value, side: &str) -> bool {
    binding.get(side).is_some_and(|value| !value.is_null())
}

fn compose_runtime_config(patch: &Value, system: &Value) -> Result<Value, String> {
    let mut runtime = patch
        .as_object()
        .ok_or_else(|| "patch runtimeConfig must be an object".to_string())?
        .clone();
    let system = system
        .as_object()
        .ok_or_else(|| "system runtimeConfig must be an object".to_string())?;
    for (key, value) in system {
        match key.as_str() {
            "auxBindings" | "shiftAuxBindings" => {
                let patch_bank = runtime.get(key).unwrap_or(&Value::Null);
                runtime.insert(key.clone(), merge_aux_bank(patch_bank, value, key)?);
            }
            "sound" => {
                let patch_sound = runtime.get(key).unwrap_or(&Value::Null);
                runtime.insert(key.clone(), merge_sound(patch_sound, value)?);
            }
            _ if runtime.contains_key(key) => {
                return Err(format!("runtimeConfig.{key} is claimed by both documents"));
            }
            _ => {
                runtime.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(Value::Object(runtime))
}

fn merge_aux_bank(patch: &Value, system: &Value, path: &str) -> Result<Value, String> {
    let patch = patch.as_object();
    let system = system.as_object();
    let slots = patch
        .into_iter()
        .flat_map(|slots| slots.keys().cloned())
        .chain(system.into_iter().flat_map(|slots| slots.keys().cloned()))
        .collect::<BTreeSet<_>>();
    let mut merged = Map::new();
    for slot in slots {
        let patch_binding = patch
            .and_then(|slots| slots.get(&slot))
            .unwrap_or(&Value::Null);
        let system_binding = system
            .and_then(|slots| slots.get(&slot))
            .unwrap_or(&Value::Null);
        let mut binding = Map::new();
        for side in ["turnKey", "pressAction"] {
            let patch_side = patch_binding.get(side).filter(|value| !value.is_null());
            let system_side = system_binding.get(side).filter(|value| !value.is_null());
            if patch_side.is_some() && system_side.is_some() {
                return Err(format!("{path}.{slot}.{side} is claimed by both documents"));
            }
            if let Some(value) = patch_side {
                binding.insert(side.into(), value.clone());
            } else if let Some(value) = system_side {
                binding.insert(side.into(), value.clone());
            }
        }
        merged.insert(
            slot,
            if binding.is_empty() {
                Value::Null
            } else {
                Value::Object(binding)
            },
        );
    }
    Ok(Value::Object(merged))
}

fn merge_sound(patch: &Value, system: &Value) -> Result<Value, String> {
    let mut merged = patch
        .as_object()
        .ok_or_else(|| "patch runtimeConfig.sound must be an object".to_string())?
        .clone();
    let system = system
        .as_object()
        .ok_or_else(|| "system runtimeConfig.sound must be an object".to_string())?;
    for (key, value) in system {
        if merged.contains_key(key) {
            return Err(format!(
                "runtimeConfig.sound.{key} is claimed by both documents"
            ));
        }
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

#[cfg(test)]
#[path = "config_documents_tests.rs"]
mod tests;
